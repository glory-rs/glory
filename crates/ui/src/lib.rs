//! Semantic UI construction and control flow. Builders mount through the runtime;
//! layout engines and DOM objects are never created by configuring a component.

mod app;
mod builders;
mod control;
pub mod list;
pub use app::{App, Error, Mounted, Result, Scope, Settle, UiStats, WeakScope, Widget};
pub use builders::*;
pub use control::{Each, When, each, when};
pub use glory_render::{
    Disclosure, Event, EventKind, FocusMode, ItemPosition, Navigation, Purpose, Role, TextKind,
};
pub use list::{List, list};

impl<W: Widget + ?Sized> Widget for Box<W> {
    fn build(&mut self, scope: &Scope) -> Result {
        (**self).build(scope)
    }
}
