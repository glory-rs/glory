//! Desktop webview renderer scaffold.
//!
//! `WryRenderer` serializes renderer commands into an IPC-friendly JSON
//! protocol. A real window host can wire the sink to Wry's webview IPC.

use std::any::Any;
use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use glory_core::renderer::{EventPayload, InsertPosition, Renderer};
use serde::{Deserialize, Serialize};

pub const WRY_INTERPRETER_JS: &str = include_str!("wry_interpreter.js");

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WryNode {
    pub id: u64,
    pub name: String,
    pub is_void: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WryCommand {
    Create { id: u64, name: String, is_void: bool },
    SetAttribute { id: u64, name: String, value: String },
    RemoveAttribute { id: u64, name: String },
    SetProperty { id: u64, name: String, value: String },
    RemoveProperty { id: u64, name: String },
    AddClass { id: u64, value: String },
    RemoveClass { id: u64, value: String },
    SetText { id: u64, value: String },
    SetHtml { id: u64, value: String },
    Insert { parent: u64, child: u64, position: WryInsertPosition },
    Remove { parent: u64, child: u64 },
    AttachEvent { id: u64, name: String, bubbles: bool },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WryInsertPosition {
    Head,
    Tail,
    Before(u64),
    After(u64),
}

pub trait WryCommandSink: Send + Sync + 'static {
    fn send(&self, command: WryCommand);
}

#[derive(Default)]
pub struct RecordingSink {
    commands: parking_lot::RwLock<Vec<WryCommand>>,
}

impl RecordingSink {
    pub fn commands(&self) -> Vec<WryCommand> {
        self.commands.read().clone()
    }
}

impl WryCommandSink for RecordingSink {
    fn send(&self, command: WryCommand) {
        self.commands.write().push(command);
    }
}

#[derive(Clone)]
pub struct WryRenderer {
    next_id: Arc<AtomicU64>,
    sink: Arc<dyn WryCommandSink>,
}

impl WryRenderer {
    pub fn new(sink: Arc<dyn WryCommandSink>) -> Self {
        Self {
            next_id: Arc::new(AtomicU64::new(1)),
            sink,
        }
    }

    fn send(&self, command: WryCommand) {
        self.sink.send(command);
    }
}

impl Default for WryRenderer {
    fn default() -> Self {
        Self::new(Arc::new(RecordingSink::default()))
    }
}

impl fmt::Debug for WryRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WryRenderer").finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub struct WryEventPayload {
    name: Cow<'static, str>,
}

impl EventPayload for WryEventPayload {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.name)
    }
}

impl Renderer for WryRenderer {
    type Event = WryEventPayload;
    type Node = WryNode;

    fn create_element(&self, name: Cow<'static, str>, is_void: bool) -> Self::Node {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let node = WryNode {
            id,
            name: name.to_string(),
            is_void,
        };
        self.send(WryCommand::Create {
            id,
            name: node.name.clone(),
            is_void,
        });
        node
    }

    fn set_attribute(&self, node: &Self::Node, name: Cow<'static, str>, value: Cow<'static, str>) {
        self.send(WryCommand::SetAttribute {
            id: node.id,
            name: name.into_owned(),
            value: value.into_owned(),
        });
    }

    fn remove_attribute(&self, node: &Self::Node, name: &str) {
        self.send(WryCommand::RemoveAttribute {
            id: node.id,
            name: name.to_string(),
        });
    }

    fn set_property(&self, node: &Self::Node, name: Cow<'static, str>, value: Cow<'static, str>) {
        self.send(WryCommand::SetProperty {
            id: node.id,
            name: name.into_owned(),
            value: value.into_owned(),
        });
    }

    fn remove_property(&self, node: &Self::Node, name: &str) {
        self.send(WryCommand::RemoveProperty {
            id: node.id,
            name: name.to_string(),
        });
    }

    fn add_class(&self, node: &Self::Node, value: Cow<'static, str>) {
        self.send(WryCommand::AddClass {
            id: node.id,
            value: value.into_owned(),
        });
    }

    fn remove_class(&self, node: &Self::Node, value: &str) {
        self.send(WryCommand::RemoveClass {
            id: node.id,
            value: value.to_string(),
        });
    }

    fn set_text(&self, node: &Self::Node, value: Cow<'static, str>) {
        self.send(WryCommand::SetText {
            id: node.id,
            value: value.into_owned(),
        });
    }

    fn set_html(&self, node: &Self::Node, value: Cow<'static, str>) {
        self.send(WryCommand::SetHtml {
            id: node.id,
            value: value.into_owned(),
        });
    }

    fn insert_child(&self, parent: &Self::Node, child: &Self::Node, position: InsertPosition<'_, Self::Node>) {
        let position = match position {
            InsertPosition::Head => WryInsertPosition::Head,
            InsertPosition::Tail => WryInsertPosition::Tail,
            InsertPosition::Before(anchor) => WryInsertPosition::Before(anchor.id),
            InsertPosition::After(anchor) => WryInsertPosition::After(anchor.id),
        };
        self.send(WryCommand::Insert {
            parent: parent.id,
            child: child.id,
            position,
        });
    }

    fn remove_child(&self, parent: &Self::Node, child: &Self::Node) {
        self.send(WryCommand::Remove {
            parent: parent.id,
            child: child.id,
        });
    }

    fn node_identity_eq(&self, left: &Self::Node, right: &Self::Node) -> bool {
        left.id == right.id
    }

    fn attach_event(&self, node: &Self::Node, name: Cow<'static, str>, bubbles: bool, _handler: Box<dyn FnMut(Self::Event)>) {
        self.send(WryCommand::AttachEvent {
            id: node.id,
            name: name.into_owned(),
            bubbles,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_serializes_commands() {
        let sink = Arc::new(RecordingSink::default());
        let renderer = WryRenderer::new(sink.clone());
        let root = renderer.create_element("main".into(), false);
        let child = renderer.create_element("button".into(), false);

        renderer.set_text(&child, "Click".into());
        renderer.insert_child(&root, &child, InsertPosition::Tail);

        assert_eq!(
            sink.commands(),
            vec![
                WryCommand::Create {
                    id: 1,
                    name: "main".to_string(),
                    is_void: false
                },
                WryCommand::Create {
                    id: 2,
                    name: "button".to_string(),
                    is_void: false
                },
                WryCommand::SetText {
                    id: 2,
                    value: "Click".to_string()
                },
                WryCommand::Insert {
                    parent: 1,
                    child: 2,
                    position: WryInsertPosition::Tail
                }
            ]
        );
    }
}
