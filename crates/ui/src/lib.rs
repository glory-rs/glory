//! Semantic UI construction and control flow. Builders mount through the runtime;
//! layout engines and DOM objects are never created by configuring a widget.

mod app;
mod buffer;
pub use buffer::BufferStats;
#[doc(hidden)]
pub mod branch;
mod builders;
mod control;
pub mod each;
pub use each::{Each, EachBuilder, each};
mod input;
mod plan;
pub use plan::{Plan, PlanInstance, PlanPart, plan};
pub mod list;
pub use app::{App, Error, Mounted, Result, Scope, Settle, UiStats, WeakScope, Widget};
pub use builders::*;
pub use control::{When, when};
pub use glory_render::{
    Current, DismissReason, Event, EventKind, FieldPart, FocusMode, FormAction, FormControl,
    GridCommand, InputAssist, InputAutofill, InputEdit, InputEditCapture, InputEditIntent,
    InputEditRejection, InputMode, ItemPosition, Layer, LayerPlacement, LinkOpen, Navigation,
    NavigationPanel, NavigationPanelCommand, Orientation, Purpose, RangeCommand, RangeValue, Role,
    ScrollAxes, ScrollCommand, ScrollMetrics, ScrollMovement, SelectionDirection, SemanticId,
    Semantics, SliderValue, SpinValue, TextKind, TextSelection,
};
pub use list::{List, list};

impl<W: Widget> Widget for Option<W> {
    fn is_empty(&self) -> bool {
        self.is_none()
    }
    fn compose(&mut self, _: &glory_core::Scope) -> Result<Option<Box<dyn Widget>>> {
        Ok(self.take().map(|value| Box::new(value) as Box<dyn Widget>))
    }
}

/// Marker for a required function-widget property that has not been supplied.
#[doc(hidden)]
pub struct Missing;

/// Defer widget construction until its owning scope is entered.
pub struct Deferred<F>(Option<F>);
pub fn defer<F, W>(factory: F) -> Deferred<F>
where
    F: FnOnce() -> W + 'static,
    W: Widget,
{
    Deferred(Some(factory))
}
impl<F, W> Widget for Deferred<F>
where
    F: FnOnce() -> W + 'static,
    W: Widget,
{
    fn compose(&mut self, _: &glory_core::Scope) -> Result<Option<Box<dyn Widget>>> {
        let factory = self.0.take().ok_or(Error::Gone)?;
        Ok(Some(Box::new(factory())))
    }
}

impl<W: Widget + ?Sized> Widget for Box<W> {
    fn is_empty(&self) -> bool {
        (**self).is_empty()
    }
    fn compose(&mut self, scope: &glory_core::Scope) -> Result<Option<Box<dyn Widget>>> {
        (**self).compose(scope)
    }
    fn mount_host(
        &mut self,
        scope: &glory_core::Scope,
        host: &mut dyn std::any::Any,
    ) -> Result<bool> {
        (**self).mount_host(scope, host)
    }
    fn static_plan(&mut self) -> Result<Plan> {
        (**self).static_plan()
    }
    fn build(&mut self, scope: &Scope) -> Result {
        (**self).build(scope)
    }
}
