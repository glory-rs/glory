//! Native renderer scaffold.

pub use glory_desktop::{WryCommand as NativeCommand, WryEventPayload as NativeEventPayload, WryNode as NativeNode};

pub type NativeRenderer = glory_desktop::WryRenderer;
