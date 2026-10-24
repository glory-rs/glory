use std::collections::HashMap;

use glory_html::{Edit, Kind, Op, Property, Shape, Sink};
use glory_render::{Error, NodeId};
use serde_json::{Value, json};

pub(super) struct Edits {
    keys: HashMap<NodeId, u64>,
    pub values: Vec<Value>,
    pub recording: bool,
    bytes: usize,
    limit: usize,
}
impl Edits {
    pub fn new(limit: usize) -> Self {
        Self {
            keys: HashMap::new(),
            values: Vec::new(),
            recording: false,
            bytes: 0,
            limit,
        }
    }
    pub fn take(&mut self) -> Vec<Value> {
        self.bytes = 0;
        std::mem::take(&mut self.values)
    }
    pub fn clear(&mut self) {
        self.keys = HashMap::new();
        self.values = Vec::new();
        self.bytes = 0;
    }
    pub fn ids(&self) -> Vec<NodeId> {
        self.keys.keys().copied().collect()
    }
    fn key(&self, id: NodeId) -> Result<u64, Error> {
        self.keys.get(&id).copied().ok_or(Error::NodeGone(id))
    }
}
fn property(value: Property) -> Value {
    match value {
        Property::Text(value) => json!(value),
        Property::Bool(value) => json!(value),
        Property::SelectedKeys(keys) => json!(keys),
        Property::RichDocument(document) => json!(document.blocks.iter().map(|block| {
            let kind=match block.kind {glory_render::RichBlockKind::Paragraph=>0,glory_render::RichBlockKind::Quote=>7,glory_render::RichBlockKind::Heading(level)=>level};
            json!({"kind":kind,"runs":block.runs.iter().map(|run|json!({"text":run.text,"bold":run.marks.bold,"italic":run.marks.italic,"code":run.marks.code,"link":run.marks.link})).collect::<Vec<_>>()})
        }).collect::<Vec<_>>()),
    }
}
pub(super) fn shape(shape: Shape) -> Value {
    fn option(value: &glory_render::SelectOption) -> Value {
        json!({"key":value.key.as_str(),"label":value.label,"value":value.value,"disabled":value.disabled})
    }
    let options = shape.select_options.as_ref().map(|config| {
        let entries: Vec<_> = config.entries.iter().map(|entry| match entry {
            glory_render::SelectEntry::Option(value) => option(value),
            glory_render::SelectEntry::Group(group) => json!({"label":group.label,"disabled":group.disabled,"options":group.options.iter().map(option).collect::<Vec<_>>()})
        }).collect();
        json!({"entries":entries,"placeholder":config.placeholder})
    });
    let pieces: Vec<_> = shape
        .pieces
        .into_iter()
        .map(|piece| {
            let mut value = match piece.kind {
                Kind::Text(text) => json!({"text":text}),
                Kind::Element {
                    tag,
                    attributes,
                    properties,
                } => {
                    let properties: serde_json::Map<_, _> = properties
                        .into_iter()
                        .map(|(name, value)| (name.into(), property(value)))
                        .collect();
                    json!({"tag":tag,"attributes":attributes,"properties":properties})
                }
            };
            value["part"] = json!(piece.part.name());
            value["parent"] = json!(piece.parent.map(|part| part.name()));
            value
        })
        .collect();
    json!({"pieces":pieces,"options":options,"children":shape.children.map(|part|part.name()),"control":shape.control.map(|part|part.name()),"revision":shape.revision})
}
impl Sink for Edits {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        let value = match op {
            Op::Create {
                id,
                key,
                shape: node,
            } => {
                self.keys.insert(id, key);
                self.recording
                    .then(|| json!({"op":"create","key":key.to_string(),"shape":shape(node)}))
            }
            Op::Patch { id, edits } => {
                if !self.recording {
                    return Ok(());
                }
                let edits: Vec<_> = edits
                    .into_iter()
                    .map(|edit| match edit {
                        Edit::Revision(revision) => json!({"edit":"revision","value":revision}),
                        Edit::Text { part, value } => json!({"edit":"text","part":part.name(),"value":value}),
                        Edit::Attribute { part, name, value } => json!({"edit":"attribute","part":part.name(),"name":name,"value":value}),
                        Edit::Property { part, name, value } => json!({"edit":"property","part":part.name(),"name":name,"value":property(value)}),
                    })
                    .collect();
                (!edits.is_empty())
                    .then(|| {
                        self.key(id)
                            .map(|key| json!({"op":"patch","key":key.to_string(),"edits":edits}))
                    })
                    .transpose()?
            }
            Op::Place { id, parent, before } => {
                if !self.recording {
                    return Ok(());
                }
                Some(
                    json!({"op":"place","key":self.key(id)?.to_string(),"parent":parent.map(|id|self.key(id).map(|key|key.to_string())).transpose()?,"before":before.map(|id|self.key(id).map(|key|key.to_string())).transpose()?}),
                )
            }
            Op::Suspend { id } => self
                .recording
                .then(|| {
                    self.key(id)
                        .map(|key| json!({"op":"suspend","key":key.to_string()}))
                })
                .transpose()?,
            Op::Dispose { id, subtree } => {
                let key = self.key(id)?;
                let removed: Vec<_> = subtree
                    .into_iter()
                    .filter_map(|id| self.keys.remove(&id))
                    .map(|key| key.to_string())
                    .collect();
                self.recording
                    .then(|| json!({"op":"dispose","key":key.to_string(),"subtree":removed}))
            }
        };
        if let Some(value) = value {
            let bytes = serde_json::to_vec(&value)
                .map_err(|error| Error::Backend(error.to_string()))?
                .len();
            self.bytes = self
                .bytes
                .checked_add(bytes + 1)
                .ok_or(Error::InvalidProps("stream edit size exhausted"))?;
            if self.bytes > self.limit {
                return Err(Error::InvalidProps("stream edit chunk byte limit"));
            }
            self.values.push(value);
        }
        Ok(())
    }
}
