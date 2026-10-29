//! One content/controller implementation, with physical mounting supplied by adapters.
use super::mounting::ContentMount;
use super::*;

pub trait Content: sealed::Content {
    fn validate(&self, scope: &Scope) -> Result<()>;
    #[doc(hidden)]
    fn install(self, scope: &Scope, parent: &ContentMount) -> glory_ui::Result;
}
impl sealed::Content for glory_ui::Children {}
impl Content for glory_ui::Children {
    fn validate(&self, _: &Scope) -> Result<()> {
        Ok(())
    }
    fn install(self, _: &Scope, parent: &ContentMount) -> glory_ui::Result {
        for child in self.into_widgets() {
            parent.one(move || child)?;
        }
        Ok(())
    }
}
impl<A: Actions + 'static> sealed::Content for View<A> {}
impl<A: Actions + 'static> Content for View<A> {
    fn validate(&self, scope: &Scope) -> Result<()> {
        self.validate(scope)
    }
    fn install(self, _: &Scope, parent: &ContentMount) -> glory_ui::Result {
        parent.one(move || self)
    }
}
impl<C: Content> sealed::Content for Option<C> {}
impl<C: Content> Content for Option<C> {
    fn validate(&self, scope: &Scope) -> Result<()> {
        self.as_ref()
            .map_or(Ok(()), |content| content.validate(scope))
    }
    fn install(self, scope: &Scope, parent: &ContentMount) -> glory_ui::Result {
        self.map_or(Ok(()), |content| content.install(scope, parent))
    }
}
impl<C: Content> sealed::Content for Vec<C> {}
impl<C: Content> Content for Vec<C> {
    fn validate(&self, scope: &Scope) -> Result<()> {
        for content in self {
            content.validate(scope)?;
        }
        Ok(())
    }
    fn install(self, scope: &Scope, parent: &ContentMount) -> glory_ui::Result {
        for content in self {
            content.install(scope, parent)?;
        }
        Ok(())
    }
}
impl<S, F, W> sealed::Content for glory_ui::Each<S, F>
where
    S: glory_ui::each::source::Source,
    F: FnMut(S::Row) -> W + 'static,
    W: Widget,
{
}
impl<S, F, W> Content for glory_ui::Each<S, F>
where
    S: glory_ui::each::source::Source,
    F: FnMut(S::Row) -> W + 'static,
    W: Widget,
{
    fn validate(&self, scope: &Scope) -> Result<()> {
        glory_ui::Each::validate(self, scope).map_err(error)
    }
    fn install(self, scope: &Scope, parent: &ContentMount) -> glory_ui::Result {
        glory_ui::Each::install(self, scope, parent.clone())
    }
}
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
    V: Widget,
    W: Widget,
{
    Show { condition, yes, no }
}
impl<C, F, G> sealed::Content for Show<C, F, G> {}
impl<C, F, G, V, W> Content for Show<C, F, G>
where
    C: FnMut() -> bool + 'static,
    F: FnMut() -> V + 'static,
    G: FnMut() -> W + 'static,
    V: Widget,
    W: Widget,
{
    fn validate(&self, _: &Scope) -> Result<()> {
        Ok(())
    }
    fn install(mut self, scope: &Scope, parent: &ContentMount) -> glory_ui::Result {
        let parent = parent.clone();
        let mut branch = glory_ui::branch::Branch::default();
        scope.structural(move || {
            let result = branch.update((self.condition)(), |selected| {
                let child = if selected {
                    parent.mount(&mut self.yes)?
                } else {
                    parent.mount(&mut self.no)?
                };
                parent.attach(&child)?;
                Ok::<_, glory_ui::Error>(Some(child))
            });
            if let Err(error) = result {
                parent.report(error);
            }
        });
        Ok(())
    }
}
static RANGE: Template = Template::new(&[glory_html::view::outlet(Outlet::new("content"))]);
pub fn content<C: Content + 'static>(value: C) -> impl Widget {
    view(&RANGE).child(Outlet::new("content"), value)
}
pub fn empty() -> impl Widget {
    content(None::<View>)
}
pub struct Lazy<F>(F);
pub fn lazy<F, V>(factory: F) -> Lazy<F>
where
    F: FnOnce() -> V,
    V: Widget,
{
    Lazy(factory)
}
impl<F> sealed::Content for Lazy<F> {}
impl<F, V> Content for Lazy<F>
where
    F: FnOnce() -> V + 'static,
    V: Widget,
{
    fn validate(&self, _: &Scope) -> Result<()> {
        Ok(())
    }
    fn install(self, _: &Scope, parent: &ContentMount) -> glory_ui::Result {
        parent.one(self.0)
    }
}
