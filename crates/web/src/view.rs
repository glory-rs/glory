//! Declarative template configuration implementing the portable Widget contract.
//! Direct sparse display::App mounting is browser-only.
use glory_core::{Bond, Cage, RuntimeId, Scope, Vary};
pub use glory_html::view::{Click, Field, Outlet, Template};
use glory_html::{template::Kind, view::Compiled};
pub use glory_ui::Widget;
use std::fmt::Display;
mod content;
mod flags;
#[doc(hidden)]
pub mod mounting;
#[doc(hidden)]
pub mod portable;
pub use content::{Content, Show, content, empty, lazy, show};
pub use flags::BoolValue;
pub use glory_ui::{Each, EachBuilder, each};

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
    pub trait Content {}
    pub trait Flag {}
}
/// Sources are explicit: values initialize once, handles and Computed observe.
pub trait TextValue: sealed::Text {
    fn validate(&self, runtime: RuntimeId) -> Result<()>;
    fn read_text(&mut self) -> Result<String>;
    fn reactive_text(&self) -> bool {
        false
    }
}
macro_rules! scalar {
    ($($ty:ty),*)=>{$(impl sealed::Text for $ty {}
    impl TextValue for $ty {
        fn read_text(&mut self)->Result<String>{Ok(self.to_string())}
        fn validate(&self,_runtime:RuntimeId)->Result<()>{Ok(())}

    })*};
}
scalar!(
    bool, char, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64
);
impl sealed::Text for String {}
impl TextValue for String {
    fn read_text(&mut self) -> Result<String> {
        Ok(self.clone())
    }
    fn validate(&self, _runtime: RuntimeId) -> Result<()> {
        if self.len() > 65536 {
            Err(error("field payload budget"))
        } else {
            Ok(())
        }
    }
}
impl sealed::Text for &str {}
impl TextValue for &str {
    fn read_text(&mut self) -> Result<String> {
        Ok(self.to_string())
    }
    fn validate(&self, _runtime: RuntimeId) -> Result<()> {
        if self.len() > 65536 {
            Err(error("field payload budget"))
        } else {
            Ok(())
        }
    }
}
macro_rules! handle {
    ($ty:ident) => {
        impl<T: Display + 'static> sealed::Text for $ty<T> {}
        impl<T: Display + 'static> TextValue for $ty<T> {
            fn read_text(&mut self) -> Result<String> {
                self.try_with(ToString::to_string).map_err(error)
            }
            fn reactive_text(&self) -> bool {
                true
            }
            fn validate(&self, runtime: RuntimeId) -> Result<()> {
                if self.runtime() != runtime || !self.is_alive() {
                    Err(error("foreign or disposed reactive input"))
                } else {
                    Ok(())
                }
            }
        }
    };
}
handle!(Cage);
handle!(Bond);
impl<T: Display + 'static> sealed::Text for Vary<T> {}
impl<T: Display + 'static> TextValue for Vary<T> {
    fn read_text(&mut self) -> Result<String> {
        self.try_with(ToString::to_string).map_err(error)
    }
    fn reactive_text(&self) -> bool {
        self.is_reactive()
    }
    fn validate(&self, runtime: RuntimeId) -> Result<()> {
        match self {
            Self::Value(_) => Ok(()),
            Self::Cage(v) => v.validate(runtime),
            Self::Bond(v) => v.validate(runtime),
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
    fn read_text(&mut self) -> Result<String> {
        Ok((self.0)().to_string())
    }
    fn reactive_text(&self) -> bool {
        true
    }
    fn validate(&self, _runtime: RuntimeId) -> Result<()> {
        Ok(())
    }
}

pub struct View<A = ()> {
    template: &'static Template,
    actions: Option<A>,
}
pub fn view(template: &'static Template) -> View {
    View {
        template,
        actions: Some(()),
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
            actions: Some((self.actions.expect("unbuilt view"), Toggle { field, value })),
        }
    }
    pub fn child<C: Content>(self, outlet: Outlet, content: C) -> View<(A, ChildValue<C>)> {
        View {
            template: self.template,
            actions: Some((
                self.actions.expect("unbuilt view"),
                ChildValue { outlet, content },
            )),
        }
    }
    pub fn bind<V: TextValue>(self, field: Field, value: V) -> View<(A, Bind<V>)> {
        View {
            template: self.template,
            actions: Some((self.actions.expect("unbuilt view"), Bind { field, value })),
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
            actions: Some((self.actions.expect("unbuilt view"), On { click, callback })),
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
    fn apply(self, mount: &mounting::Mount<'_>) -> glory_ui::Result;
}
impl sealed::Actions for () {}
impl Actions for () {
    fn validate(&self, _: &Compiled, _: &Scope, _: &mut Vec<u16>) -> Result<()> {
        Ok(())
    }
    fn apply(self, _: &mounting::Mount<'_>) -> glory_ui::Result {
        Ok(())
    }
}
impl<A: Actions, B: Actions> sealed::Actions for (A, B) {}
impl<A: Actions, B: Actions> Actions for (A, B) {
    fn validate(&self, d: &Compiled, s: &Scope, seen: &mut Vec<u16>) -> Result<()> {
        self.0.validate(d, s, seen)?;
        self.1.validate(d, s, seen)
    }
    fn apply(self, m: &mounting::Mount<'_>) -> glory_ui::Result {
        self.0.apply(m)?;
        self.1.apply(m)
    }
}
impl<V: TextValue> sealed::Actions for Bind<V> {}
impl<V: TextValue + 'static> Actions for Bind<V> {
    fn validate(&self, d: &Compiled, s: &Scope, seen: &mut Vec<u16>) -> Result<()> {
        resolve(
            d,
            self.field.0,
            |k| matches!(k, Kind::Text | Kind::Attribute(_)),
            seen,
        )?;
        self.value.validate(s.runtime())
    }
    fn apply(self, m: &mounting::Mount<'_>) -> glory_ui::Result {
        m.text(self.field, self.value)
    }
}
impl<F: FnMut() + 'static> sealed::Actions for On<F> {}
impl<F: FnMut() + 'static> Actions for On<F> {
    fn validate(&self, d: &Compiled, _: &Scope, seen: &mut Vec<u16>) -> Result<()> {
        resolve(d, self.click.0, |k| k == Kind::Click, seen)?;
        Ok(())
    }
    fn apply(self, m: &mounting::Mount<'_>) -> glory_ui::Result {
        m.on(self.click, self.callback)
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
    fn apply(self, m: &mounting::Mount<'_>) -> glory_ui::Result {
        m.child(self.outlet, self.content)
    }
}
impl<V: BoolValue> sealed::Actions for Toggle<V> {}
impl<V: BoolValue> Actions for Toggle<V> {
    fn validate(&self, d: &Compiled, s: &Scope, seen: &mut Vec<u16>) -> Result<()> {
        resolve(d, self.field.0, |k| matches!(k, Kind::Class(..)), seen)?;
        self.value.validate_bool(s.runtime())
    }
    fn apply(self, m: &mounting::Mount<'_>) -> glory_ui::Result {
        m.flag(self.field, self.value)
    }
}
impl<A: Actions> View<A> {
    #[cfg(target_arch = "wasm32")]
    pub fn mount(self, app: &crate::template::App, scope: &Scope) -> Result<Mounted>
    where
        A: 'static,
    {
        mount(self, app, scope)
    }
    pub fn validate(&self, scope: &Scope) -> Result<()> {
        if !scope.is_alive() {
            return Err(error("disposed view scope"));
        }
        let definition = self.template.cached()?;
        self.actions
            .as_ref()
            .ok_or_else(|| error("view already built"))?
            .validate(&definition, scope, &mut Vec::new())
    }
    #[cfg(target_arch = "wasm32")]
    fn mount_direct(&mut self, app: &crate::template::App, scope: &Scope) -> Result<Mounted> {
        if !scope.is_alive() || scope.runtime() != app.runtime().id() {
            return Err(error("foreign or disposed view scope"));
        }
        let definition = app.prepare(self.template)?;
        self.validate(scope)?;
        let actions = self
            .actions
            .take()
            .ok_or_else(|| error("view already built"))?;
        let instance = app.mount(scope, self.template)?;
        if let Err(e) = actions
            .apply(&mounting::Mount::sparse(&definition, app, scope, &instance))
            .map_err(error)
        {
            instance.fail(e.clone());
            return Err(e);
        }
        Ok(Mounted { instance })
    }
}
impl<A: Actions + 'static> Widget for View<A> {
    fn build(&mut self, scope: &glory_ui::Scope) -> glory_ui::Result {
        self.validate(scope).map_err(portable::failure)?;
        let actions = self.actions.take().ok_or(glory_ui::Error::Gone)?;
        portable::build(self.template, actions, scope)
    }
    fn mount_host(
        &mut self,
        scope: &Scope,
        host: &mut dyn std::any::Any,
    ) -> glory_ui::Result<bool> {
        #[cfg(target_arch = "wasm32")]
        if let Some(context) = host.downcast_mut::<DisplayMount>() {
            context.result = Some(self.mount_direct(&context.app, scope));
            return Ok(true);
        }
        let _ = (scope, host);
        Ok(false)
    }
}
#[cfg(target_arch = "wasm32")]
struct DisplayMount {
    app: crate::template::App,
    result: Option<Result<Mounted>>,
}
/// Mount any Widget; raw views retain their sparse template fast path.
#[cfg(target_arch = "wasm32")]
pub fn mount(
    mut widget: impl Widget,
    app: &crate::template::App,
    scope: &Scope,
) -> Result<Mounted> {
    app.ensure_live()?;
    if !scope.is_alive() || scope.runtime() != app.runtime().id() {
        return Err(error("foreign or disposed widget scope"));
    }
    if widget.is_empty() {
        return mount(empty(), app, scope);
    }
    if let Some(child) =
        glory_core::untracked(|| scope.enter(|| widget.compose(scope))).map_err(error)?
    {
        return mount(child, app, scope);
    }
    let mut context = DisplayMount {
        app: app.clone(),
        result: None,
    };
    if widget.mount_host(scope, &mut context).map_err(error)? {
        return context
            .result
            .ok_or_else(|| error("widget host did not mount"))?;
    }
    app.mount_widget(scope, widget)
        .map(|instance| Mounted { instance })
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
