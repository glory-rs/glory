//! Structural content shared by handwritten configurations and XML lowering.
use super::*;
use glory_core::{KeyedEntry, KeyedList};
use std::hash::Hash;

pub trait Content: sealed::Content {
    fn validate(&self, scope: &Scope) -> Result<()>;
    #[cfg(target_arch = "wasm32")]
    fn install(
        self,
        app: &crate::template::App,
        scope: &Scope,
        parent: &crate::template::Children,
        instance: &crate::template::Instance,
    ) -> Result<()>;
}
impl<A: Actions> sealed::Content for View<A> {}
impl<A: Actions> Content for View<A> {
    fn validate(&self, s: &Scope) -> Result<()> {
        self.validate(s)
    }
    #[cfg(target_arch = "wasm32")]
    fn install(
        self,
        a: &crate::template::App,
        s: &Scope,
        p: &crate::template::Children,
        i: &crate::template::Instance,
    ) -> Result<()> {
        let child = s.child();
        let view = child.enter(|| self.mount(a, &child))?;
        view.place(Some(p), None)?;
        i.retain_child(child)
    }
}
impl<C: Component> sealed::Content for ComponentView<C> {}
impl<C: Component> Content for ComponentView<C> {
    fn validate(&self, _: &Scope) -> Result<()> {
        Ok(())
    }
    #[cfg(target_arch = "wasm32")]
    fn install(
        self,
        a: &crate::template::App,
        s: &Scope,
        p: &crate::template::Children,
        i: &crate::template::Instance,
    ) -> Result<()> {
        let child = s.child();
        let view = self.mount(a, &child)?;
        view.place(Some(p), None)?;
        i.retain_child(child)
    }
}
impl<V: Content> sealed::Content for Option<V> {}
impl<V: Content> Content for Option<V> {
    fn validate(&self, s: &Scope) -> Result<()> {
        self.as_ref().map_or(Ok(()), |v| v.validate(s))
    }
    #[cfg(target_arch = "wasm32")]
    fn install(
        self,
        a: &crate::template::App,
        s: &Scope,
        p: &crate::template::Children,
        i: &crate::template::Instance,
    ) -> Result<()> {
        self.map_or(Ok(()), |v| v.install(a, s, p, i))
    }
}
impl<V: Content> sealed::Content for Vec<V> {}
impl<V: Content> Content for Vec<V> {
    fn validate(&self, s: &Scope) -> Result<()> {
        for v in self {
            v.validate(s)?
        }
        Ok(())
    }
    #[cfg(target_arch = "wasm32")]
    fn install(
        self,
        a: &crate::template::App,
        s: &Scope,
        p: &crate::template::Children,
        i: &crate::template::Instance,
    ) -> Result<()> {
        for v in self {
            v.install(a, s, p, i)?
        }
        Ok(())
    }
}

/// Type-erased, single-use component children, independent of HTML node types.
pub struct ChildContent {
    validate: Box<dyn Fn(&Scope) -> Result<()>>,
    #[cfg(target_arch = "wasm32")]
    mount: Box<
        dyn FnOnce(
            &crate::template::App,
            &Scope,
            &crate::template::Children,
            &crate::template::Instance,
        ) -> Result<()>,
    >,
}
pub fn child_content<C: Content + 'static>(content: C) -> ChildContent {
    #[cfg(target_arch = "wasm32")]
    {
        let content = std::rc::Rc::new(std::cell::RefCell::new(Some(content)));
        let validation = content.clone();
        ChildContent {
            validate: Box::new(move |s| {
                validation
                    .borrow()
                    .as_ref()
                    .ok_or_else(|| error("children already consumed"))?
                    .validate(s)
            }),
            mount: Box::new(move |a, s, p, i| {
                content
                    .borrow_mut()
                    .take()
                    .ok_or_else(|| error("children already consumed"))?
                    .install(a, s, p, i)
            }),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        ChildContent {
            validate: Box::new(move |s| content.validate(s)),
        }
    }
}
impl sealed::Content for ChildContent {}
impl Content for ChildContent {
    fn validate(&self, s: &Scope) -> Result<()> {
        (self.validate)(s)
    }
    #[cfg(target_arch = "wasm32")]
    fn install(
        self,
        a: &crate::template::App,
        s: &Scope,
        p: &crate::template::Children,
        i: &crate::template::Instance,
    ) -> Result<()> {
        (self.mount)(a, s, p, i)
    }
}

pub struct Each<K: 'static, T: 'static, F> {
    source: KeyedList<K, T>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    factory: F,
}
pub fn each<K, T, F, V>(source: KeyedList<K, T>, factory: F) -> Each<K, T, F>
where
    K: Clone + Eq + Hash + 'static,
    T: 'static,
    F: FnMut(KeyedEntry<K, T>) -> V + 'static,
    V: IntoView,
{
    Each { source, factory }
}
impl<K, T, F> sealed::Content for Each<K, T, F> {}
impl<K, T, F, V> Content for Each<K, T, F>
where
    K: Clone + Eq + Hash + 'static,
    T: 'static,
    F: FnMut(KeyedEntry<K, T>) -> V + 'static,
    V: IntoView,
{
    fn validate(&self, s: &Scope) -> Result<()> {
        if self.source.runtime() != s.runtime() || !self.source.is_alive() {
            return Err(error("foreign or disposed list"));
        }
        Ok(())
    }
    #[cfg(target_arch = "wasm32")]
    fn install(
        self,
        a: &crate::template::App,
        s: &Scope,
        p: &crate::template::Children,
        i: &crate::template::Instance,
    ) -> Result<()> {
        use glory_ui::list::{Cache, RowHost};
        struct Row {
            view: Mounted,
            _scope: Scope,
        }
        struct Host<K: 'static, T: 'static, F> {
            source: KeyedList<K, T>,
            tokens: Vec<glory_core::RowToken<K>>,
            factory: F,
            app: crate::template::WeakApp,
            owner: glory_core::InstanceId,
            parent: crate::template::Children,
            fault: Option<Error>,
        }
        impl<K, T, F, V> RowHost for Host<K, T, F>
        where
            K: Clone + Eq + Hash + 'static,
            T: 'static,
            F: FnMut(KeyedEntry<K, T>) -> V + 'static,
            V: IntoView,
        {
            type Key = u64;
            type Row = Option<Row>;
            fn key(&mut self, index: usize) -> u64 {
                self.tokens[index].id()
            }
            fn mount(&mut self, index: usize, _: &u64) -> Option<Row> {
                if self.fault.is_some() {
                    return None;
                }
                let result = (|| {
                    let scope = self.owner.child_scope().map_err(error)?;
                    let entry = self.source.entry(&self.tokens[index]).map_err(error)?;
                    let view = scope
                        .enter(|| (self.factory)(entry))
                        .mount(&self.app.upgrade()?, &scope)?;
                    Ok(Row {
                        view,
                        _scope: scope,
                    })
                })();
                match result {
                    Ok(row) => Some(row),
                    Err(e) => {
                        self.fault = Some(e);
                        None
                    }
                }
            }
            fn update(&mut self, _: usize, _: &u64, _: &mut Option<Row>) {}
            fn has_content(&self, row: &Option<Row>) -> bool {
                row.is_some()
            }
            fn place(&mut self, row: &Option<Row>, before: Option<&Option<Row>>) {
                if self.fault.is_none()
                    && let Some(row) = row
                    && let Err(e) = row.view.place(
                        Some(&self.parent),
                        before.and_then(|r| r.as_ref().map(|r| &r.view)),
                    )
                {
                    self.fault = Some(e)
                }
            }
            fn dispose(&mut self, row: Option<Row>) {
                drop(row)
            }
        }
        let source = self.source;
        let mut host = Host {
            source,
            tokens: Vec::new(),
            factory: self.factory,
            app: a.downgrade(),
            owner: s.id(),
            parent: p.clone(),
            fault: None,
        };
        let mut cache = Cache::default();
        let mut revision = 0u64;
        let instance = i.clone();
        s.structural(move || {
            host.tokens = source.with_order(<[_]>::to_vec);
            revision = revision.checked_add(1).expect("list revision exhausted");
            if let Err(e) = cache.reconcile(0..host.tokens.len(), revision, &mut host) {
                host.fault = Some(error(e))
            }
            if let Some(e) = host.fault.take() {
                instance.fail(e)
            }
        });
        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub struct Show<C, F, G> {
    condition: C,
    yes: F,
    no: G,
}
pub fn show<C, F, G, V, W>(condition: C, yes: F, no: G) -> Show<C, F, G>
where
    C: FnMut() -> bool + 'static,
    F: FnMut() -> V + 'static,
    G: FnMut() -> W + 'static,
    V: IntoView,
    W: IntoView,
{
    Show { condition, yes, no }
}
impl<C, F, G> sealed::Content for Show<C, F, G> {}
impl<C, F, G, V, W> Content for Show<C, F, G>
where
    C: FnMut() -> bool + 'static,
    F: FnMut() -> V + 'static,
    G: FnMut() -> W + 'static,
    V: IntoView,
    W: IntoView,
{
    fn validate(&self, _: &Scope) -> Result<()> {
        Ok(())
    }
    #[cfg(target_arch = "wasm32")]
    fn install(
        mut self,
        a: &crate::template::App,
        s: &Scope,
        p: &crate::template::Children,
        i: &crate::template::Instance,
    ) -> Result<()> {
        let app = a.downgrade();
        let owner = s.id();
        let parent = p.clone();
        let instance = i.clone();
        let mut active = None;
        let mut current: Option<Scope> = None;
        s.structural(move || {
            let next = (self.condition)();
            if active == Some(next) {
                return;
            }
            drop(current.take());
            let result = (|| {
                let scope = owner.child_scope().map_err(error)?;
                let app = app.upgrade()?;
                let view = if next {
                    scope.enter(&mut self.yes).mount(&app, &scope)?
                } else {
                    scope.enter(&mut self.no).mount(&app, &scope)?
                };
                view.place(Some(&parent), None)?;
                current = Some(scope);
                active = Some(next);
                Ok(())
            })();
            if let Err(e) = result {
                instance.fail(e)
            }
        });
        Ok(())
    }
}

static RANGE: Template = Template::new(&[glory_html::view::outlet(Outlet::new("content"))]);
pub fn content<C: Content>(value: C) -> impl IntoView {
    view(&RANGE).child(Outlet::new("content"), value)
}
pub fn empty() -> impl IntoView {
    content(None::<View>)
}
pub struct Lazy<F>(F);
pub fn lazy<F, V>(factory: F) -> Lazy<F>
where
    F: FnOnce() -> V,
    V: IntoView,
{
    Lazy(factory)
}
impl<F> sealed::Content for Lazy<F> {}
impl<F, V> Content for Lazy<F>
where
    F: FnOnce() -> V,
    V: IntoView,
{
    fn validate(&self, _: &Scope) -> Result<()> {
        Ok(())
    }
    #[cfg(target_arch = "wasm32")]
    fn install(
        self,
        a: &crate::template::App,
        s: &Scope,
        p: &crate::template::Children,
        i: &crate::template::Instance,
    ) -> Result<()> {
        let child = s.child();
        let view = child.enter(self.0).mount(a, &child)?;
        view.place(Some(p), None)?;
        i.retain_child(child)
    }
}
impl<V: Content> sealed::View for Option<V> {}
impl<V: Content> IntoView for Option<V> {
    #[cfg(target_arch = "wasm32")]
    fn mount(self, a: &crate::template::App, s: &Scope) -> Result<Mounted> {
        content(self).mount(a, s)
    }
}
impl<V: Content> sealed::View for Vec<V> {}
impl<V: Content> IntoView for Vec<V> {
    #[cfg(target_arch = "wasm32")]
    fn mount(self, a: &crate::template::App, s: &Scope) -> Result<Mounted> {
        content(self).mount(a, s)
    }
}
