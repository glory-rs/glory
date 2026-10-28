//! One bounded Plan host segment. Unsupported/input operations flush before
//! falling back. The mapper confirms exactly the JS-completed semantic prefix.
use super::*;
use glory_html::{Mapper, PreparedOp};
use glory_render::Mutation;

const NONE: u32 = u32::MAX;
const MAX_RECORDS: usize = 4096;
const MAX_BYTES: usize = 64 * 1024;

enum Confirm {
    Create(Vec<(NodeId, Rc<Entry>)>),
    Place(Rc<Entry>, Option<NodeId>),
    Dispose(NodeId, Vec<NodeId>, Rc<plans::Instance>),
    Fields,
}
struct Packet {
    objects: Vec<JsValue>,
    records: Vec<u32>,
    strings: String,
    units: u32,
    bank: js_sys::Array,
    parts: u32,
    entries: HashMap<NodeId, Rc<Entry>>,
    confirms: Vec<Confirm>,
}
impl Packet {
    fn new() -> Self {
        Self {
            objects: Vec::new(),
            records: Vec::new(),
            strings: String::new(),
            units: 0,
            bank: js_sys::Array::new(),
            parts: 0,
            entries: HashMap::new(),
            confirms: Vec::new(),
        }
    }
    fn object(&mut self, value: &JsValue) -> u32 {
        let index = self.objects.len() as u32;
        self.objects.push(value.clone());
        index
    }
    fn string(&mut self, value: &str) -> (u32, u32) {
        let start = self.units;
        self.strings.push_str(value);
        self.units += value.encode_utf16().count() as u32;
        (start, self.units)
    }
    fn record(&mut self, record: [u32; 10]) {
        self.records.extend(record);
    }
    fn entry(&self, dom: &Dom, id: NodeId) -> Result<Rc<Entry>, Error> {
        self.entries
            .get(&id)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| dom.entry(id))
    }
    fn target(&mut self, entry: &Entry, part: Part) -> Result<(u32, u32), Error> {
        if let Some((instance, member)) = &entry.parts.planned {
            let address = instance
                .address(*member, part)
                .ok_or(Error::Unsupported("HTML part"))?;
            Ok((self.object(&instance.values), address))
        } else {
            Ok((self.object(entry.node(part)?.as_ref()), NONE))
        }
    }
    fn destination(&mut self, dom: &Dom, parent: Option<NodeId>) -> Result<(u32, u32, u32), Error> {
        if let Some(parent) = parent {
            let entry = self.entry(dom, parent)?;
            if let Some(part) = entry.children {
                let (object, address) = self.target(&entry, part)?;
                return Ok((object, address, 0));
            }
            if entry.parts.is_empty() {
                return Ok((self.object(entry.last().as_ref()), NONE, 1));
            }
            return Err(Error::InvalidProps("node has no HTML child slot"));
        }
        if let Some(end) = dom.root_end.borrow().as_ref() {
            Ok((self.object(end.as_ref()), NONE, 1))
        } else {
            Ok((self.object(dom.root.as_ref()), NONE, 0))
        }
    }
    fn encode(&mut self, dom: &Dom, op: PreparedOp) -> Result<bool, Error> {
        match op {
            PreparedOp::Plan {
                plan,
                ids,
                keys,
                shape,
            } => {
                shape.validate_instance(&plan, &ids, &keys)?;
                if shape.shapes.iter().any(|shape| shape.edit != EditKind::None || shape.select_options.is_some() || shape.pieces.is_empty() || shape.pieces.iter().any(|piece| match &piece.kind {
                    Kind::Element { attributes, properties, .. } => properties.iter().any(|(name, value)| *name != "disabled" || !matches!(value, Property::Bool(disabled) if *disabled == attributes.contains_key("disabled"))),
                    Kind::Text(_) => false,
                })) { return Ok(false); }
                // Cold prototype DOM work must not overtake a queued prefix.
                if !self.confirms.is_empty() && !dom.plans.cached(&plan) {
                    return Ok(false);
                }
                let Some(prototype) = dom.plans.get(&dom.document, &plan, &shape)? else {
                    return Ok(false);
                };
                let mut tokens = String::with_capacity(ids.len() * 24);
                use std::fmt::Write;
                for (index, id) in ids.iter().enumerate() {
                    if index != 0 {
                        tokens.push('\0');
                    }
                    write!(
                        &mut tokens,
                        "{}.{}.{}",
                        id.runtime().value(),
                        id.slot(),
                        id.generation()
                    )
                    .expect("identity string");
                }
                if self.strings.len() + tokens.len() > MAX_BYTES {
                    return Ok(false);
                }
                let offset = self.parts;
                let instance = plans::Instance::pooled(
                    prototype.clone(),
                    self.bank.clone(),
                    offset,
                    ids.len(),
                );
                let mut entries = Vec::with_capacity(ids.len());
                for (index, node_shape) in shape.shapes.iter().enumerate() {
                    let parts = Parts::planned(instance.clone(), index);
                    let entry = Entry::new(
                        node_shape,
                        parts,
                        Boundary::Part(node_shape.root_part()?),
                        None,
                        None,
                        false,
                    );
                    entries.push((ids[index], entry));
                }
                for (index, (_, entry)) in entries.iter().enumerate() {
                    entry.parent.set(shape.parents[index].map(|p| ids[p]));
                }
                self.parts += prototype.parts();
                let object = self.object(&prototype.host);
                let (start, end) = self.string(&tokens);
                self.record([1, object, offset, 0, 0, 0, 0, 0, start, end]);
                self.entries.extend(entries.iter().cloned());
                self.confirms.push(Confirm::Create(entries));
            }
            PreparedOp::Op(Op::Patch { id, edits }) => {
                let entry = self.entry(dom, id)?;
                if entry.parts.planned.is_none()
                    || edits
                        .iter()
                        .any(|edit| !matches!(edit, Edit::Text { .. } | Edit::Attribute { .. }))
                {
                    return Ok(false);
                }
                let bytes: usize = edits
                    .iter()
                    .map(|edit| match edit {
                        Edit::Text { value, .. } => value.len(),
                        Edit::Attribute { name, value, .. } => {
                            name.len() + value.as_ref().map_or(0, String::len)
                        }
                        _ => 0,
                    })
                    .sum();
                if self.strings.len() + bytes > MAX_BYTES {
                    return Ok(false);
                }
                for edit in edits {
                    match edit {
                        Edit::Text { part, value } => {
                            let (object, address) = self.target(&entry, part)?;
                            let (start, end) = self.string(&value);
                            self.record([2, object, address, 0, 0, 0, 0, 0, start, end]);
                        }
                        Edit::Attribute { part, name, value } => {
                            let (object, address) = self.target(&entry, part)?;
                            let (start, name_end) = self.string(name);
                            let kind = if value.is_some() { 3 } else { 4 };
                            let (_, end) = self.string(value.as_deref().unwrap_or_default());
                            self.record([kind, object, address, 0, 0, 0, 0, start, name_end, end]);
                        }
                        _ => unreachable!(),
                    }
                }
                self.confirms.push(Confirm::Fields);
            }
            PreparedOp::Op(Op::Place { id, parent, before }) => {
                let entry = self.entry(dom, id)?;
                let Boundary::Part(part) = entry.boundary else {
                    return Ok(false);
                };
                if entry.parts.planned.is_none() {
                    return Ok(false);
                }
                let (object, address) = self.target(&entry, part)?;
                let (dest, dest_address, range) = self.destination(dom, parent)?;
                let (before, before_address) = if let Some(before) = before {
                    let entry = self.entry(dom, before)?;
                    match entry.boundary {
                        Boundary::Part(part) => self.target(&entry, part)?,
                        Boundary::Range(_, _) => (self.object(entry.first().as_ref()), NONE),
                    }
                } else {
                    (NONE, NONE)
                };
                self.record([
                    5,
                    object,
                    address,
                    dest,
                    dest_address,
                    before,
                    before_address,
                    range,
                    0,
                    0,
                ]);
                self.confirms.push(Confirm::Place(entry, parent));
            }
            PreparedOp::Op(Op::Dispose { id, subtree }) => {
                let entry = self.entry(dom, id)?;
                let Some((instance, 0)) = &entry.parts.planned else {
                    return Ok(false);
                };
                let Boundary::Part(part) = entry.boundary else {
                    return Ok(false);
                };
                // Full original instances only. Partial/moved members and foreign
                // descendants retain the existing retirement implementation.
                if subtree.len() != instance.members()
                    || subtree.iter().any(|id| {
                        self.entry(dom, *id).map_or(true, |e| {
                            e.edit != EditKind::None
                                || e.interests.borrow().contains(&EventKind::Resize)
                                || e.parts
                                    .planned
                                    .as_ref()
                                    .is_none_or(|(other, _)| !Rc::ptr_eq(instance, other))
                        })
                    })
                {
                    return Ok(false);
                }
                let (object, address) = self.target(&entry, part)?;
                self.record([
                    6,
                    object,
                    address,
                    instance.offset,
                    instance.offset + instance.prototype.parts(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]);
                self.confirms
                    .push(Confirm::Dispose(id, subtree, instance.clone()));
            }
            _ => return Ok(false),
        }
        self.record([0; 10]);
        Ok(true)
    }
    fn confirm(self, dom: &Dom, accepted: usize) -> Result<(), Error> {
        for confirm in self.confirms.into_iter().take(accepted) {
            match confirm {
                Confirm::Create(entries) => {
                    let mut table = dom.entries.borrow_mut();
                    for (id, entry) in entries {
                        table.insert(id, entry);
                    }
                }
                Confirm::Place(entry, parent) => entry.parent.set(parent),
                Confirm::Dispose(id, subtree, instance) => {
                    instance.cleared.set(true);
                    dom.dispose_confirmed(id, subtree, true)?;
                }
                Confirm::Fields => {}
            }
        }
        Ok(())
    }
}

/// Ordered fallback is handled by the caller. Returning zero requests exactly
/// one scalar mutation before trying another segment.
pub(in crate::browser) fn apply(
    dom: &Dom,
    mapper: &mut Mapper,
    mutations: &[Mutation],
) -> Result<usize, Error> {
    if dom.staging.get() {
        return Ok(0);
    }
    let Ok(mut prepared) = mapper.submission() else {
        return Ok(0);
    };
    if !mutations.first().is_some_and(|m| prepared.supports(m)) {
        return Ok(0);
    }
    let mut packet = Packet::new();
    let mut pending_error = None;
    for mutation in mutations {
        if !prepared.supports(mutation) || packet.records.len() / 10 + 16 > MAX_RECORDS {
            break;
        }
        let op = match prepared.prepare(mutation) {
            Ok(op) => op,
            Err(Error::Unsupported(_)) => break,
            Err(error) => {
                pending_error = Some(error);
                break;
            }
        };
        match packet.encode(dom, op) {
            Ok(true) => {}
            Ok(false) | Err(Error::Unsupported(_)) => {
                prepared.discard_last();
                break;
            }
            Err(error) => {
                prepared.discard_last();
                pending_error = Some(error);
                break;
            }
        }
    }
    if prepared.is_empty() {
        return pending_error.map_or(Ok(0), Err);
    }
    let reply = plans::submit_plans(
        &packet.objects,
        &packet.records,
        &packet.strings,
        &packet.bank,
        dom.live.get(),
    )
    .map_err(backend)?;
    let accepted = reply
        .get(0)
        .as_f64()
        .ok_or(Error::InvalidProps("submission prefix"))? as usize;
    let status = reply
        .get(1)
        .as_f64()
        .ok_or(Error::InvalidProps("submission status"))? as u32;
    if accepted > prepared.len() || status > 2 || (status == 0 && accepted != prepared.len()) {
        return Err(Error::InvalidProps("invalid submission completion"));
    }
    let host_error = (status == 2).then(|| {
        Error::Backend(
            reply
                .get(2)
                .as_string()
                .unwrap_or_else(|| "Plan submission failed".into()),
        )
    });
    prepared.finish(accepted);
    packet.confirm(dom, accepted)?;
    if let Some(error) = host_error.or(pending_error.filter(|_| status == 0)) {
        return Err(error);
    }
    Ok(accepted)
}
