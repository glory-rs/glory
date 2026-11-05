//! Document-local wire format. Core/HTML types deliberately remain unserialized.
use glory_html::{Edit, Kind, Op, Property, Shape};
use glory_render::{Error, NodeId};
use serde::Deserialize;
use serde_json::{Value, json};

pub const VERSION: u32 = 2;
// Controlled document snapshots can expand sixfold when JSON escapes text.
// Bound encoded bytes independently of the unescaped editor value.
pub const MAX_IPC_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PENDING_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_QUERIES: usize = 1024;
pub const MAX_INPUTS: usize = 4096;

pub fn id(node: NodeId) -> String {
    format!(
        "{}.{}.{}",
        node.runtime().value(),
        node.slot(),
        node.generation()
    )
}
fn property(value: Property) -> Result<Value, Error> {
    Ok(match value {
        Property::Text(text) => json!(text),
        Property::Bool(value) => json!(value),
        _ => return Err(Error::Unsupported("Wry structured control property")),
    })
}
fn shape(shape: Shape) -> Result<Value, Error> {
    let pieces = shape
        .pieces
        .into_iter()
        .map(|piece| {
            let kind = match piece.kind {
                Kind::Text(text) => json!({"text":text}),
                Kind::Element {
                    tag,
                    attributes,
                    properties,
                } => {
                    let properties = properties
                        .into_iter()
                        .map(|(k, v)| Ok((k.to_owned(), property(v)?)))
                        .collect::<Result<serde_json::Map<_, _>, Error>>()?;
                    json!({"tag":tag,"attributes":attributes,"properties":properties})
                }
            };
            Ok(json!({"part":piece.part.name(),"parent":piece.parent.map(|p|p.name()),"kind":kind}))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(
        json!({"pieces":pieces,"children":shape.children.map(|p|p.name()),
        "control":shape.control.map(|p|p.name()),"edit":format!("{:?}",shape.edit),
        "revision":shape.revision.to_string(),"prevent":shape.prevent_activate_default}),
    )
}
pub fn op(op: Op) -> Result<Value, Error> {
    Ok(match op {
        Op::Create {
            id: node,
            shape: value,
            ..
        } => json!({"op":"create","id":id(node),"shape":shape(value)?}),
        Op::Patch { id: node, edits } => {
            let edits = edits.into_iter().map(|edit| Ok(match edit {
                Edit::Revision(revision) => json!({"kind":"revision","value":revision.to_string()}),
                Edit::Attribute { part, name, value } => json!({"kind":"attribute","part":part.name(),"name":name,"value":value}),
                Edit::Property { part, name, value } => json!({"kind":"property","part":part.name(),"name":name,"value":property(value)?}),
                Edit::Text { part, value } => json!({"kind":"text","part":part.name(),"value":value}),
            })).collect::<Result<Vec<_>,Error>>()?;
            json!({"op":"patch","id":id(node),"edits":edits})
        }
        Op::Place {
            id: node,
            parent,
            before,
        } => json!({"op":"place","id":id(node),"parent":parent.map(id),"before":before.map(id)}),
        Op::Suspend { id: node } => json!({"op":"suspend","id":id(node)}),
        Op::Dispose { id: node, subtree } => {
            json!({"op":"dispose","id":id(node),"subtree":subtree.into_iter().map(id).collect::<Vec<_>>()})
        }
    })
}

#[derive(Deserialize)]
pub struct Incoming {
    pub v: u32,
    pub session: String,
    #[serde(flatten)]
    pub payload: Payload,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Payload {
    Ready,
    Ack {
        sequence: String,
    },
    Fault {
        message: String,
    },
    Event {
        node: String,
        event: WireEvent,
    },
    Response {
        token: String,
        value: Option<Value>,
        error: Option<String>,
    },
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WireEvent {
    Clipboard {
        request: String,
        revision: String,
        outcome: String,
    },
    Activate,
    Drag {
        phase: String,
        x: f64,
        y: f64,
    },
    Value {
        value: String,
        composing: bool,
        revision: String,
    },
    Commit {
        value: String,
        revision: String,
    },
    Checked {
        value: bool,
    },
    Composition {
        active: bool,
    },
    Focus {
        focused: bool,
    },
    Resize {
        width: f64,
        height: f64,
        viewport_width: f64,
        viewport_height: f64,
    },
    Scroll {
        x: f64,
        y: f64,
        viewport_width: f64,
        viewport_height: f64,
        ack: Option<WireAck>,
    },
    Keyboard {
        command: String,
    },
    Navigate {
        command: String,
    },
    ColorScheme {
        dark: bool,
    },
    Image {
        revision: String,
        loaded: bool,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireAck {
    pub requested_x: f64,
    pub requested_y: f64,
    pub x: f64,
    pub y: f64,
}
pub fn integer(text: &str) -> Result<u64, Error> {
    text.parse::<u64>()
        .ok()
        .filter(|n| n.to_string() == text)
        .ok_or(Error::InvalidResponse("noncanonical Wry integer"))
}
