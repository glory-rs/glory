use std::collections::HashMap;

use glory_html::display::{Frame, Placement, Queue, Value as SlotValue};
use glory_html::{Edit, Kind, Op, Property, Shape, Sink};
use glory_render::{Error, NodeId};
use serde_json::{Value, json};
use std::rc::Rc;

// Declaration identities are request-local wire keys, never pointer/Runtime IDs.
// Keep only declarations referenced by live (including suspended) instances.
struct Declaration {
    key: u64,
    references: usize,
    sent: bool,
}

pub(super) struct Edits {
    keys: HashMap<NodeId, u64>,
    pub values: Vec<Value>,
    pub recording: bool,
    bytes: usize,
    limit: usize,
    queue: Rc<Queue>,
    frames: HashMap<NodeId, Rc<Frame>>,
    owners: HashMap<*const Frame, NodeId>,
    declarations: HashMap<usize, Declaration>,
}
impl Edits {
    pub fn new(limit: usize) -> Self {
        Self {
            keys: HashMap::new(),
            values: Vec::new(),
            recording: false,
            bytes: 0,
            limit,
            queue: Rc::default(),
            frames: HashMap::new(),
            owners: HashMap::new(),
            declarations: HashMap::new(),
        }
    }
    pub fn take(&mut self) -> Vec<Value> {
        self.bytes = 0;
        std::mem::take(&mut self.values)
    }
    pub fn clear(&mut self) {
        self.frames.clear();
        self.owners.clear();
        self.declarations.clear();
        self.queue.take();
        self.keys = HashMap::new();
        self.values = Vec::new();
        self.bytes = 0;
    }
    pub fn ids(&self) -> Vec<NodeId> {
        self.keys.keys().copied().collect()
    }
    #[cfg(test)]
    pub(super) fn display_stats(&self) -> (usize, usize) {
        (self.frames.len(), self.declarations.len())
    }
    fn key(&self, id: NodeId) -> Result<u64, Error> {
        self.keys.get(&id).copied().ok_or(Error::NodeGone(id))
    }
    fn register(&mut self, id: NodeId, key: u64, shape: &Shape) {
        if let Some(Placement::Root(frame)) = &shape.display {
            self.declarations
                .entry(frame.compiled() as *const _ as usize)
                .or_insert(Declaration {
                    key,
                    references: 0,
                    sent: false,
                })
                .references += 1;
            self.owners.insert(Rc::as_ptr(frame), id);
            self.frames.insert(id, frame.clone());
            frame.bind(&self.queue, id);
        }
    }
    pub fn shape(&mut self, node: Shape) -> Result<Value, Error> {
        let placement = node.display.clone();
        let mut value = shape(node);
        if let Some(placement) = placement {
            value["display"] = match placement {
                Placement::Root(frame) => {
                    let declaration = self
                        .declarations
                        .get_mut(&(frame.compiled() as *const _ as usize))
                        .ok_or(Error::InvalidProps("unregistered stream template"))?;
                    let wire = (!declaration.sent).then(|| frame.compiled().wire.as_str());
                    declaration.sent = true;
                    let values = (0..frame.compiled().slots.len())
                        .map(|slot| frame.value(slot as u16).map(slot_value))
                        .collect::<Result<Vec<_>, _>>()?;
                    json!({"kind":"root","declaration":declaration.key.to_string(),"wire":wire,"values":values})
                }
                Placement::Outlet { frame, slot } => {
                    json!({"kind":"outlet","owner":self.key(*self.owners.get(&Rc::as_ptr(&frame)).ok_or(Error::InvalidPlacement("missing stream template owner"))?)?.to_string(),"slot":slot})
                }
            };
        }
        Ok(value)
    }
    fn push(&mut self, value: Value) -> Result<(), Error> {
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
        Ok(())
    }
    pub fn flush_display(&mut self) -> Result<(), Error> {
        for id in self.queue.take() {
            let Some(frame) = self.frames.get(&id) else {
                continue;
            };
            let changes = frame.take_changes();
            if self.recording && !changes.is_empty() {
                let values: Vec<_> = changes
                    .into_iter()
                    .map(|(slot, value)| json!([slot, slot_value(value)]))
                    .collect();
                self.push(json!({"op":"display","key":self.key(id)?.to_string(),"values":values}))?;
            }
        }
        Ok(())
    }
}
fn slot_value(value: SlotValue) -> Value {
    match value {
        SlotValue::Text(text) => json!(text),
        SlotValue::Flag(flag) => json!(flag),
        SlotValue::None => Value::Null,
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
                self.register(id, key, &node);
                if self.recording {
                    Some(json!({"op":"create","key":key.to_string(),"shape":self.shape(node)?}))
                } else {
                    None
                }
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
                    .filter_map(|id| {
                        if let Some(frame) = self.frames.remove(&id) {
                            self.owners.remove(&Rc::as_ptr(&frame));
                            let address = frame.compiled() as *const _ as usize;
                            let declaration = self
                                .declarations
                                .get_mut(&address)
                                .expect("live declaration");
                            declaration.references -= 1;
                            if declaration.references == 0 {
                                self.declarations.remove(&address);
                            }
                        }
                        self.keys.remove(&id)
                    })
                    .map(|key| key.to_string())
                    .collect();
                self.recording
                    .then(|| json!({"op":"dispose","key":key.to_string(),"subtree":removed}))
            }
        };
        if let Some(value) = value {
            self.push(value)?;
        }
        Ok(())
    }
}
