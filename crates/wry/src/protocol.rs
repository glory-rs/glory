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
        Property::SelectedKeys(keys) => json!(keys),
        Property::RichDocument(value) => super::rich::document(&value),
    })
}
fn shape(shape: Shape) -> Result<Value, Error> {
    fn option(value: &glory_render::SelectOption) -> Value {
        json!({"key":value.key.as_str(),"label":value.label,"value":value.value,"disabled":value.disabled})
    }
    let options = shape.select_options.as_ref().map(|config| {
        let entries = config
            .entries
            .iter()
            .map(|entry| match entry {
                glory_render::SelectEntry::Option(value) => option(value),
                glory_render::SelectEntry::Group(group) => json!({
                    "label":group.label,"disabled":group.disabled,
                    "options":group.options.iter().map(option).collect::<Vec<_>>()
                }),
            })
            .collect::<Vec<_>>();
        json!({"entries":entries,"placeholder":config.placeholder})
    });
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
    let mut value = json!({"pieces":pieces,"children":shape.children.map(|p|p.name()),
        "control":shape.control.map(|p|p.name()),"edit":format!("{:?}",shape.edit),
        "revision":shape.revision.to_string(),"prevent":shape.prevent_activate_default});
    if let Some(options) = options {
        value["options"] = options;
    }
    Ok(value)
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
    Files {
        observation: super::files::Observation,
    },
    LayerBounds {
        node: String,
        request: String,
        anchor: [f64; 4],
        panel: [f64; 2],
        viewport: [f64; 2],
    },
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
    RichEdit {
        snapshot: super::rich::Snapshot,
    },
    RichSelection {
        snapshot: super::rich::Snapshot,
    },
    RichRejected {
        revision: String,
    },
    RichCommand {
        revision: String,
        command: String,
    },
    Sheet {
        revision: String,
        viewport: f64,
        extent: f64,
    },
    FormSubmit {
        submitter: Option<String>,
    },
    FormReset,
    CollapseComplete {
        revision: String,
    },
    PresenceComplete {
        revision: String,
    },
    Swipe {
        phase: String,
        revision: String,
        x: f64,
        y: f64,
    },
    KeyBinding {
        binding: u16,
        from: Option<String>,
    },
    TreeSearch {
        from: String,
        text: String,
        at_millis: u64,
    },
    Palette,
    Tags {
        revision: String,
        data: WireTags,
    },
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
        #[serde(default)]
        from: Option<String>,
    },
    Value {
        value: String,
        composing: bool,
        revision: String,
    },
    InputEdit {
        revision: String,
        edit: WireInputEdit,
    },
    Commit {
        value: String,
        revision: String,
    },
    Select {
        keys: Vec<String>,
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
    ContextMenu {
        x: f64,
        y: f64,
        keyboard: bool,
    },
    Dismiss {
        reason: String,
    },
    Typeahead {
        text: String,
        at_millis: u64,
    },
    PlotPointer {
        pointer_id: i32,
        device: String,
        phase: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    Pointer {
        related: Option<String>,
        phase: String,
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
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum WireInputEdit {
    Value {
        value: String,
        selection: WireSelection,
        before: Option<WireSelection>,
        intent: WireIntent,
        composing: bool,
    },
    TooLong,
    SelectionUnavailable,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireSelection {
    start: u32,
    end: u32,
    direction: WireDirection,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum WireDirection {
    None,
    Forward,
    Backward,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireIntent {
    Insert,
    DeleteBackward,
    DeleteForward,
    Replace,
}

impl From<WireSelection> for glory_render::TextSelection {
    fn from(value: WireSelection) -> Self {
        use glory_render::SelectionDirection;
        Self {
            start: value.start as usize,
            end: value.end as usize,
            direction: match value.direction {
                WireDirection::None => SelectionDirection::None,
                WireDirection::Forward => SelectionDirection::Forward,
                WireDirection::Backward => SelectionDirection::Backward,
            },
        }
    }
}

impl WireInputEdit {
    pub fn decode(self, revision: u64) -> glory_render::InputEdit {
        use glory_render::{InputEdit, InputEditIntent, InputEditRejection};
        match self {
            Self::Value {
                value,
                selection,
                before,
                intent,
                composing,
            } => InputEdit::Value {
                value,
                revision,
                selection: selection.into(),
                before: before.map(Into::into),
                intent: match intent {
                    WireIntent::Insert => InputEditIntent::Insert,
                    WireIntent::DeleteBackward => InputEditIntent::DeleteBackward,
                    WireIntent::DeleteForward => InputEditIntent::DeleteForward,
                    WireIntent::Replace => InputEditIntent::Replace,
                },
                composing,
            },
            Self::TooLong => InputEdit::Rejected {
                revision,
                reason: InputEditRejection::TooLong,
            },
            Self::SelectionUnavailable => InputEdit::Rejected {
                revision,
                reason: InputEditRejection::SelectionUnavailable,
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum WireTags {
    Commit {
        value: String,
    },
    RemoveLast,
    Paste {
        value: String,
        text: String,
        start: u32,
        end: u32,
    },
    InputBudget,
    ClipboardBudget,
}
impl From<WireTags> for glory_render::TagsCommand {
    fn from(value: WireTags) -> Self {
        use glory_render::TagHostRejection;
        match value {
            WireTags::Commit { value } => Self::Commit { value },
            WireTags::RemoveLast => Self::RemoveLast,
            WireTags::Paste {
                value,
                text,
                start,
                end,
            } => Self::Paste {
                value,
                text,
                start,
                end,
            },
            WireTags::InputBudget => Self::Reject(TagHostRejection::InputBudget),
            WireTags::ClipboardBudget => Self::Reject(TagHostRejection::ClipboardBudget),
        }
    }
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
