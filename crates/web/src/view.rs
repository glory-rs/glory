//! Declarative, short-lived view configuration. Browser mounting is wasm-only.
use glory_core::{Bond, Cage, RuntimeId, Scope, Vary};
pub use glory_html::view::{Click, Field, Outlet, Template};
use glory_html::{
    template::Kind,
    view::{Compiled, Source},
};
use std::fmt::Display;
mod content;
mod flags;
pub use content::{
    ChildContent, Content, Each, Show, child_content, content, each, empty, lazy, show,
};
pub use flags::BoolValue;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
impl From<glory_html::template::Error> for Error {
    fn from(e: glory_html::template::Error) -> Self {
        Self(e.to_string())
    }
}
pub type Result<T> = std::result::Result<T, Error>;
fn error(value: impl Display) -> Error {
    Error(value.to_string())
}

mod sealed {
    pub trait Actions {}
    pub trait Text {}
    pub trait View {}
    pub trait Content {}
    pub trait Flag {}
}
/// Sources are explicit: values initialize once, handles and Computed observe.
pub trait TextValue: sealed::Text {
    fn validate(&self, runtime: RuntimeId) -> Result<()>;
    #[cfg(target_arch = "wasm32")]
    fn install(self, instance: &crate::template::Instance, owner: &Scope, slot: u16) -> Result<()>;
}
macro_rules! scalar {
    ($($ty:ty),*)=>{$(impl sealed::Text for $ty {}
    impl TextValue for $ty {
        fn validate(&self,_runtime:RuntimeId)->Result<()>{Ok(())}
        #[cfg(target_arch="wasm32")]
        fn install(self,instance:&crate::template::Instance,_owner:&Scope,slot:u16)->Result<()>{instance.set(slot,self.to_string())}
    })*};
}
scalar!(
    bool, char, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64
);
impl sealed::Text for String {}
impl TextValue for String {
    fn validate(&self, _runtime: RuntimeId) -> Result<()> {
        if self.len() > 65536 {
            Err(error("field payload budget"))
        } else {
            Ok(())
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn install(
        self,
        instance: &crate::template::Instance,
        _owner: &Scope,
        slot: u16,
    ) -> Result<()> {
        instance.set(slot, self)
    }
}
impl sealed::Text for &str {}
impl TextValue for &str {
    fn validate(&self, _runtime: RuntimeId) -> Result<()> {
        if self.len() > 65536 {
            Err(error("field payload budget"))
        } else {
            Ok(())
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn install(
        self,
        instance: &crate::template::Instance,
        _owner: &Scope,
        slot: u16,
    ) -> Result<()> {
        instance.set(slot, self.to_owned())
    }
}
macro_rules! handle {
    ($ty:ident) => {
        impl<T: Display + 'static> sealed::Text for $ty<T> {}
        impl<T: Display + 'static> TextValue for $ty<T> {
            fn validate(&self, runtime: RuntimeId) -> Result<()> {
                if self.runtime() != runtime || !self.is_alive() {
                    Err(error("foreign or disposed reactive input"))
                } else {
                    Ok(())
                }
            }
            #[cfg(target_arch = "wasm32")]
            fn install(
                self,
                instance: &crate::template::Instance,
                owner: &Scope,
                slot: u16,
            ) -> Result<()> {
                instance.bind_values(owner, [slot], move || [self.with(ToString::to_string)])
            }
        }
    };
}
handle!(Cage);
handle!(Bond);
impl<T: Display + 'static> sealed::Text for Vary<T> {}
impl<T: Display + 'static> TextValue for Vary<T> {
    fn validate(&self, runtime: RuntimeId) -> Result<()> {
        match self {
            Self::Value(_) => Ok(()),
            Self::Cage(v) => v.validate(runtime),
            Self::Bond(v) => v.validate(runtime),
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn install(self, instance: &crate::template::Instance, owner: &Scope, slot: u16) -> Result<()> {
        match self {
            Self::Value(value) => instance.set(slot, value.to_string()),
            Self::Cage(v) => v.install(instance, owner, slot),
            Self::Bond(v) => v.install(instance, owner, slot),
        }
    }
}
pub struct Computed<F>(F);
pub fn computed<F, T>(read: F) -> Computed<F>
where
    F: FnMut() -> T,
    T: Display,
{
    Computed(read)
}
impl<F, T> sealed::Text for Computed<F>
where
    F: FnMut() -> T + 'static,
    T: Display,
{
}
impl<F, T> TextValue for Computed<F>
where
    F: FnMut() -> T + 'static,
    T: Display,
{
    fn validate(&self, _runtime: RuntimeId) -> Result<()> {
        Ok(())
    }
    #[cfg(target_arch = "wasm32")]
    fn install(
        mut self,
        instance: &crate::template::Instance,
        owner: &Scope,
        slot: u16,
    ) -> Result<()> {
        instance.bind_values(owner, [slot], move || [(self.0)().to_string()])
    }
}

pub struct View<A = ()> {
    template: &'static Template,
    actions: A,
}
pub fn view(template: &'static Template) -> View {
    View {
        template,
        actions: (),
    }
}
#[doc(hidden)]
pub struct Bind<V> {
    field: Field,
    value: V,
}
#[doc(hidden)]
pub struct On<F> {
    click: Click,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    callback: F,
}
#[doc(hidden)]
pub struct Child<C: Component> {
    outlet: Outlet,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    props: C::Props,
}
#[doc(hidden)]
pub struct ChildValue<C> {
    outlet: Outlet,
    content: C,
}
#[doc(hidden)]
pub struct Toggle<V> {
    field: Field,
    value: V,
}
impl<A> View<A> {
    pub fn class_when<V: BoolValue>(self, field: Field, value: V) -> View<(A, Toggle<V>)> {
        View {
            template: self.template,
            actions: (self.actions, Toggle { field, value }),
        }
    }
    pub fn child<C: Content>(self, outlet: Outlet, content: C) -> View<(A, ChildValue<C>)> {
        View {
            template: self.template,
            actions: (self.actions, ChildValue { outlet, content }),
        }
    }
    pub fn bind<V: TextValue>(self, field: Field, value: V) -> View<(A, Bind<V>)> {
        View {
            template: self.template,
            actions: (self.actions, Bind { field, value }),
        }
    }
    pub fn watch<F, T>(self, field: Field, read: F) -> View<(A, Bind<Computed<F>>)>
    where
        F: FnMut() -> T + 'static,
        T: Display,
    {
        self.bind(field, computed(read))
    }
    pub fn on_click<F: FnMut() + 'static>(self, click: Click, callback: F) -> View<(A, On<F>)> {
        View {
            template: self.template,
            actions: (self.actions, On { click, callback }),
        }
    }
    pub fn component<C: Component>(self, outlet: Outlet, props: C::Props) -> View<(A, Child<C>)> {
        View {
            template: self.template,
            actions: (self.actions, Child { outlet, props }),
        }
    }
    pub fn template(&self) -> &'static Template {
        self.template
    }
}
fn resolve(
    definition: &Compiled,
    name: &str,
    expected: fn(Kind) -> bool,
    seen: &mut Vec<u16>,
) -> Result<u16> {
    let (slot, kind) = definition
        .slot(name)
        .ok_or_else(|| error(format!("unknown slot: {name}")))?;
    if !expected(kind) {
        return Err(error(format!("wrong kind for slot: {name}")));
    }
    if seen.contains(&slot) {
        return Err(error(format!("duplicate configuration: {name}")));
    }
    seen.push(slot);
    Ok(slot)
}
/// Implemented only by built-in configuration actions; not a host extension API.
#[doc(hidden)]
pub trait Actions: sealed::Actions {
    fn validate(&self, definition: &Compiled, scope: &Scope, seen: &mut Vec<u16>) -> Result<()>;
    #[cfg(target_arch = "wasm32")]
    fn apply(
        self,
        definition: &Compiled,
        app: &crate::template::App,
        scope: &Scope,
        instance: &crate::template::Instance,
    ) -> Result<()>;
}
impl sealed::Actions for () {}
impl Actions for () {
    fn validate(&self, _: &Compiled, _: &Scope, _: &mut Vec<u16>) -> Result<()> {
        Ok(())
    }
    #[cfg(target_arch = "wasm32")]
    fn apply(
        self,
        _: &Compiled,
        _: &crate::template::App,
        _: &Scope,
        _: &crate::template::Instance,
    ) -> Result<()> {
        Ok(())
    }
}
impl<A: Actions, B: Actions> sealed::Actions for (A, B) {}
impl<A: Actions, B: Actions> Actions for (A, B) {
    fn validate(&self, d: &Compiled, s: &Scope, seen: &mut Vec<u16>) -> Result<()> {
        self.0.validate(d, s, seen)?;
        self.1.validate(d, s, seen)
    }
    #[cfg(target_arch = "wasm32")]
    fn apply(
        self,
        d: &Compiled,
        a: &crate::template::App,
        s: &Scope,
        i: &crate::template::Instance,
    ) -> Result<()> {
        self.0.apply(d, a, s, i)?;
        self.1.apply(d, a, s, i)
    }
}
impl<V: TextValue> sealed::Actions for Bind<V> {}
impl<V: TextValue> Actions for Bind<V> {
    fn validate(&self, d: &Compiled, s: &Scope, seen: &mut Vec<u16>) -> Result<()> {
        resolve(
            d,
            self.field.0,
            |k| matches!(k, Kind::Text | Kind::Attribute(_)),
            seen,
        )?;
        self.value.validate(s.runtime())
    }
    #[cfg(target_arch = "wasm32")]
    fn apply(
        self,
        d: &Compiled,
        _: &crate::template::App,
        s: &Scope,
        i: &crate::template::Instance,
    ) -> Result<()> {
        self.value.install(i, s, d.slot(self.field.0).unwrap().0)
    }
}
impl<F: FnMut() + 'static> sealed::Actions for On<F> {}
impl<F: FnMut() + 'static> Actions for On<F> {
    fn validate(&self, d: &Compiled, _: &Scope, seen: &mut Vec<u16>) -> Result<()> {
        resolve(d, self.click.0, |k| k == Kind::Click, seen)?;
        Ok(())
    }
    #[cfg(target_arch = "wasm32")]
    fn apply(
        self,
        d: &Compiled,
        _: &crate::template::App,
        _: &Scope,
        i: &crate::template::Instance,
    ) -> Result<()> {
        i.on_click(d.slot(self.click.0).unwrap().0, self.callback)
    }
}
/// Components explicitly declare Props; lowercase elements never enter this path.
pub trait Component {
    type Props;
    fn view(scope: &Scope, props: Self::Props) -> impl IntoView;
}
pub struct ComponentView<C: Component> {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    props: C::Props,
}
pub fn component<C: Component>(props: C::Props) -> ComponentView<C> {
    ComponentView { props }
}
impl<C: Component> sealed::Actions for Child<C> {}
impl<C: Component> Actions for Child<C> {
    fn validate(&self, d: &Compiled, _: &Scope, seen: &mut Vec<u16>) -> Result<()> {
        resolve(
            d,
            self.outlet.0,
            |k| matches!(k, Kind::Children | Kind::Range),
            seen,
        )?;
        Ok(())
    }
    #[cfg(target_arch = "wasm32")]
    fn apply(
        self,
        d: &Compiled,
        a: &crate::template::App,
        s: &Scope,
        i: &crate::template::Instance,
    ) -> Result<()> {
        let child = s.child();
        let mounted = component::<C>(self.props).mount(a, &child)?;
        mounted.place(Some(&i.children(d.slot(self.outlet.0).unwrap().0)?), None)?;
        i.retain_child(child)
    }
}
impl<C: Content> sealed::Actions for ChildValue<C> {}
impl<C: Content> Actions for ChildValue<C> {
    fn validate(&self, d: &Compiled, s: &Scope, seen: &mut Vec<u16>) -> Result<()> {
        resolve(
            d,
            self.outlet.0,
            |k| matches!(k, Kind::Children | Kind::Range),
            seen,
        )?;
        self.content.validate(s)
    }
    #[cfg(target_arch = "wasm32")]
    fn apply(
        self,
        d: &Compiled,
        a: &crate::template::App,
        s: &Scope,
        i: &crate::template::Instance,
    ) -> Result<()> {
        self.content
            .install(a, s, &i.children(d.slot(self.outlet.0).unwrap().0)?, i)
    }
}
impl<V: BoolValue> sealed::Actions for Toggle<V> {}
impl<V: BoolValue> Actions for Toggle<V> {
    fn validate(&self, d: &Compiled, s: &Scope, seen: &mut Vec<u16>) -> Result<()> {
        resolve(d, self.field.0, |k| matches!(k, Kind::Class(..)), seen)?;
        self.value.validate_bool(s.runtime())
    }
    #[cfg(target_arch = "wasm32")]
    fn apply(
        self,
        d: &Compiled,
        _: &crate::template::App,
        s: &Scope,
        i: &crate::template::Instance,
    ) -> Result<()> {
        self.value
            .install_bool(i, s, d.slot(self.field.0).unwrap().0)
    }
}
pub trait IntoView: sealed::View {
    #[cfg(target_arch = "wasm32")]
    fn mount(self, app: &crate::template::App, scope: &Scope) -> Result<Mounted>;
}
impl<A: Actions> sealed::View for View<A> {}
impl<A: Actions> View<A> {
    /// Structural/source validation is also available to native compiler tests.
    pub fn validate(&self, scope: &Scope) -> Result<()> {
        if !scope.is_alive() {
            return Err(error("disposed view scope"));
        }
        let definition = self.template.compile()?;
        self.actions.validate(&definition, scope, &mut Vec::new())
    }
}
impl<A: Actions> IntoView for View<A> {
    #[cfg(target_arch = "wasm32")]
    fn mount(self, app: &crate::template::App, scope: &Scope) -> Result<Mounted> {
        if !scope.is_alive() || scope.runtime() != app.runtime().id() {
            return Err(error("foreign or disposed view scope"));
        }
        let definition = app.prepare(self.template)?;
        self.actions.validate(&definition, scope, &mut Vec::new())?;
        let instance = app.mount(scope, self.template)?;
        if let Err(e) = self.actions.apply(&definition, app, scope, &instance) {
            instance.fail(e.clone());
            return Err(e);
        }
        Ok(Mounted { instance })
    }
}
impl<C: Component> sealed::View for ComponentView<C> {}
impl<C: Component> IntoView for ComponentView<C> {
    #[cfg(target_arch = "wasm32")]
    fn mount(self, app: &crate::template::App, scope: &Scope) -> Result<Mounted> {
        app.ensure_live()?;
        if !scope.is_alive() || scope.runtime() != app.runtime().id() {
            return Err(error("foreign or disposed view scope"));
        }
        scope.enter(|| C::view(scope, self.props)).mount(app, scope)
    }
}
#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
pub struct Mounted {
    instance: crate::template::Instance,
}
#[cfg(target_arch = "wasm32")]
impl Mounted {
    pub fn set(&self, field: Field, value: impl Display) -> Result<()> {
        let slot = self
            .instance
            .named(field.0, |k| matches!(k, Kind::Text | Kind::Attribute(_)))?;
        self.instance.set(slot, value.to_string())
    }
    pub fn on_click(&self, click: Click, callback: impl FnMut() + 'static) -> Result<()> {
        self.instance.on_click(
            self.instance.named(click.0, |k| k == Kind::Click)?,
            callback,
        )
    }
    pub fn children(&self, outlet: Outlet) -> Result<crate::template::Children> {
        self.instance.children(
            self.instance
                .named(outlet.0, |k| matches!(k, Kind::Children | Kind::Range))?,
        )
    }
    pub fn place(
        &self,
        parent: Option<&crate::template::Children>,
        before: Option<&Mounted>,
    ) -> Result<()> {
        self.instance.place(parent, before.map(|b| &b.instance))
    }
}
