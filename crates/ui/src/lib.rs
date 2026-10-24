//! Semantic UI construction and control flow. Builders mount through the runtime;
//! layout engines and DOM objects are never created by configuring a component.

mod app;
mod buffer;
pub use buffer::BufferStats;
mod builders;
mod control;
mod input;
mod plan;
pub use plan::{Plan, PlanInstance, PlanPart, plan};
pub mod list;
pub use app::{App, Error, Mounted, Result, Scope, Settle, UiStats, WeakScope, Widget};
pub use builders::*;
pub use control::{Each, When, each, when};
pub use glory_render::{
    Current, DismissReason, Event, EventKind, FieldPart, FocusMode, FormAction, FormControl,
    GridCommand, InputAssist, InputAutofill, InputEdit, InputEditCapture, InputEditIntent,
    InputEditRejection, InputMode, ItemPosition, Layer, LayerPlacement, LinkOpen, Navigation,
    NavigationPanel, NavigationPanelCommand, Orientation, Purpose, RangeCommand, RangeValue, Role,
    ScrollAxes, ScrollCommand, ScrollMetrics, ScrollMovement, SelectionDirection, SemanticId,
    Semantics, SliderValue, SpinValue, TextKind, TextSelection,
};
pub use list::{List, list};

impl<W: Widget + ?Sized> Widget for Box<W> {
    fn static_plan(&mut self) -> Result<Plan> {
        (**self).static_plan()
    }
    fn build(&mut self, scope: &Scope) -> Result {
        (**self).build(scope)
    }
}
