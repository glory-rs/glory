//! Ratatui renderer scaffold.

pub use glory_desktop::{WryCommand as TuiCommand, WryEventPayload as TuiEventPayload, WryNode as TuiNode};
pub use ratatui;

pub type TuiRenderer = glory_desktop::WryRenderer;
