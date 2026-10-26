//! Document-owned, bounded prototypes of explicitly declared semantic plans.
//! Only static declarations enter prototypes; identities and bindings do not.
use super::{Boundary, Dom, Entry, Parts, backend, virtual_shape};
use glory_html::{Edit, Kind, Part, PlanShape};
use glory_render::{Error, NodeId, Plan, WeakPlan};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    fmt::Write,
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};
use web_sys::{Document, Node};

#[path = "plan_parts.rs"]
mod addresses;
pub(super) use addresses::index as part_index;
use addresses::{Layout, Retirement};

#[path = "plan_fields.rs"]
mod fields;

// Private experiment boundary: the generic build changes only this constant.
const DEEP_CLONE: bool = true;

#[wasm_bindgen(inline_js = "
export function gloryPlan(root, parts, owners, virtualParts, fields) {
    const positions = new Map(parts.map((part, index) => [part, index]));
    // Parts are registered in construction order. Each non-root part can be
    // found from one earlier part, without retracing ancestors from the root
    // or obtaining a live childNodes collection for every path segment.
    const links = parts.map((part, index) => {
        if (part === root) return null;
        const previous = part.previousSibling;
        const source = positions.get(previous || part.parentNode);
        if (source === undefined || source >= index) throw new Error('Invalid physical plan order');
        return [source, !!previous];
    });
    return { root, links, owners, virtualParts, fields };
}
export function gloryClonePlan(plan, tokens, live) {
    const root = plan.root.cloneNode(true);
    const parts = new Array(plan.links.length);
    for (let index = 0; index < parts.length; index++) {
        const link = plan.links[index];
        parts[index] = link === null ? root : link[1] ? parts[link[0]].nextSibling : parts[link[0]].firstChild;
    }
    for (const [index, name, value] of plan.fields) parts[index].setAttribute(name, value);
    const identities = tokens.split('\\0');
    for (let index = 0; index < parts.length; index++) {
        const part = parts[index];
        if (part.nodeType === 1) {
            part.setAttribute('data-gly-node', identities[plan.owners[index]]);
            if (live && plan.virtualParts[index]) part.setAttribute('data-gly-live', '');
        }
    }
    return parts;
}
export function gloryRetirePlanParts(plan, parts, masks) {
    // The typed u32 slice is borrowed for this synchronous call only.
    for (let index = 0; index < parts.length; index++) {
        const owner = plan.owners[index];
        if (masks[owner >>> 5] & (1 << (owner & 31))) parts[index] = null;
    }
}
export function gloryPlanText(parts, address, value) {
    parts[address].nodeValue = value;
}
export function gloryPlanAttribute(parts, address, name, value) {
    const node = parts[address];
    if (value === undefined) node.removeAttribute(name);
    else node.setAttribute(name, value);
}
export function gloryPlanFields(parts, records, strings) {
    // Native event ingress can reenter WASM and grow its memory during a DOM
    // write. Snapshot this bounded borrowed view before the first host call.
    const fields = records.slice();
    let start = 0;
    for (let index = 0; index < fields.length; index += 4) {
        const node = parts[fields[index]], kind = fields[index + 1];
        const nameEnd = fields[index + 2], valueEnd = fields[index + 3];
        const value = strings.substring(nameEnd, valueEnd);
        if (kind === 0) node.nodeValue = value;
        else {
            const name = strings.substring(start, nameEnd);
            if (kind === 1) node.setAttribute(name, value);
            else if (kind === 2) node.removeAttribute(name);
            else throw new Error('Invalid Plan field kind');
        }
        start = valueEnd;
    }
}
export function gloryPlaceDetachedPlan(parts, address, parent, before, rangeEnd) {
    const node = parts[address];
    // Connected moves keep the existing Rust focus/selection/IME path. This
    // reply is observation only: no partial mutation precedes the fallback.
    if (node.isConnected) return 0;
    const container = rangeEnd ? parent.parentNode : parent;
    if (!container) return 2;
    container.insertBefore(node, before || (rangeEnd ? parent : null));
    return 1;
}
export function gloryRemovePlanPart(parts, address) {
    const node = parts[address], parent = node.parentNode;
    if (parent) parent.removeChild(node);
}
")]
extern "C" {
    #[wasm_bindgen(catch, js_name = gloryPlan)]
    fn prototype(
        root: &Node,
        parts: &js_sys::Array,
        owners: &js_sys::Array,
        virtual_parts: &js_sys::Array,
        fields: &js_sys::Array,
    ) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = gloryClonePlan)]
    fn clone_plan(plan: &JsValue, tokens: &str, live: bool) -> Result<js_sys::Array, JsValue>;
    #[wasm_bindgen(catch, js_name = gloryRetirePlanParts)]
    fn retire_parts(plan: &JsValue, parts: &js_sys::Array, masks: &[u32]) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = gloryPlanText)]
    fn write_text(parts: &js_sys::Array, address: u32, value: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = gloryPlanAttribute)]
    fn write_attribute(
        parts: &js_sys::Array,
        address: u32,
        name: &str,
        value: Option<&str>,
    ) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = gloryPlanFields)]
    fn write_fields(parts: &js_sys::Array, records: &[u32], strings: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(catch, js_name = gloryPlaceDetachedPlan)]
    fn place_detached(
        parts: &js_sys::Array,
        address: u32,
        parent: &Node,
        before: Option<&Node>,
        range_end: bool,
    ) -> Result<u32, JsValue>;
    #[wasm_bindgen(catch, js_name = gloryRemovePlanPart)]
    fn remove_part(parts: &js_sys::Array, address: u32) -> Result<(), JsValue>;
}

/// A host destination, not a replacement for semantic placement validation.
pub(super) enum Destination<'a> {
    Children(&'a Node),
    RangeEnd(&'a Node),
}

/// A private, validated physical target. The instance retains the original Node
/// array; no host traversal or identity reconstruction is needed for field work.
pub(super) struct Target<'a> {
    instance: &'a Instance,
    address: u32,
}
impl Target<'_> {
    pub fn place_detached(
        &self,
        destination: Destination<'_>,
        before: Option<&Node>,
    ) -> Result<bool, Error> {
        let (parent, range_end) = match destination {
            Destination::Children(parent) => (parent, false),
            Destination::RangeEnd(end) => (end, true),
        };
        let reply = place_detached(
            &self.instance.values,
            self.address,
            parent,
            before,
            range_end,
        )
        .map_err(backend)?;
        match reply {
            0 => Ok(false),
            1 => Ok(true),
            2 => Err(Error::InvalidPlacement("detached Group end")),
            _ => Err(Error::InvalidProps("host placement reply")),
        }
    }
    pub fn remove(&self) -> Result<(), Error> {
        remove_part(&self.instance.values, self.address).map_err(backend)
    }
}
struct Prototype {
    host: JsValue,
    layout: Layout,
}
/// One detached clone's original physical parts. All semantic members keep
/// their independent Entry/NodeId; this only shares their host addressing.
pub(super) struct Instance {
    prototype: Rc<Prototype>,
    values: js_sys::Array,
    retirement: RefCell<Retirement>,
}
impl Instance {
    pub fn target(&self, member: usize, part: Part) -> Option<Target<'_>> {
        self.address(member, part).map(|address| Target {
            instance: self,
            address,
        })
    }
    /// Execute only the leading run of ordinary fields. Revision/property edits
    /// are explicit barriers and stay with the controlled-input implementation.
    pub fn fields(&self, member: usize, edits: &[Edit]) -> Result<usize, Error> {
        if !matches!(
            edits.first(),
            Some(Edit::Text { .. } | Edit::Attribute { .. })
        ) {
            return Ok(0);
        }
        if let [edit] = edits {
            match edit {
                Edit::Text { part, value } => {
                    let address = self
                        .address(member, *part)
                        .ok_or(Error::Unsupported("HTML part"))?;
                    write_text(&self.values, address, value).map_err(backend)?;
                    return Ok(1);
                }
                Edit::Attribute { part, name, value } => {
                    let address = self
                        .address(member, *part)
                        .ok_or(Error::Unsupported("HTML part"))?;
                    write_attribute(&self.values, address, name, value.as_deref())
                        .map_err(backend)?;
                    return Ok(1);
                }
                _ => return Ok(0),
            }
        }
        let batch = fields::Batch::new(edits, |part| self.address(member, part))?;
        if batch.len() != 0 {
            write_fields(&self.values, batch.records(), batch.strings()).map_err(backend)?;
        }
        Ok(batch.len())
    }
    pub fn address(&self, member: usize, part: Part) -> Option<u32> {
        self.prototype.layout.address(member, part)
    }
    pub fn node(&self, address: u32) -> Node {
        // Layout and clone come from the same validated immutable prototype.
        // Like the eager path, these are native Nodes constructed by our helper.
        self.values.get(address).unchecked_into()
    }
    pub fn retire(&self, member: usize) {
        self.retirement.borrow_mut().retire(member);
    }
    pub fn flush_retired(&self) -> Result<(), Error> {
        let pending = self.retirement.borrow_mut().take();
        if let Some(masks) = pending {
            retire_parts(&self.prototype.host, &self.values, &masks).map_err(backend)?;
        }
        Ok(())
    }
}
struct Cached {
    source: WeakPlan,
    prototype: Rc<Prototype>,
    nodes: usize,
    bytes: usize,
}
#[derive(Default)]
pub(super) struct Plans(RefCell<HashMap<usize, Cached>>);
fn deferred(name: &str) -> bool {
    matches!(
        name,
        "src" | "href" | "value" | "checked" | "id" | "aria-label"
    )
}
impl Plans {
    fn get(
        &self,
        document: &Document,
        plan: &Plan,
        shape: &PlanShape,
    ) -> Result<Option<Rc<Prototype>>, Error> {
        if !DEEP_CLONE || shape.shapes.iter().any(|node| node.pieces.is_empty()) {
            return Ok(None);
        }
        let key = plan.identity();
        if let Some(cached) = self.0.borrow().get(&key)
            && cached.source.upgrade().is_some_and(|p| p.same(plan))
        {
            return Ok(Some(cached.prototype.clone()));
        }
        let (mut nodes, mut bytes) = shape.weight();
        {
            let mut cache = self.0.borrow_mut();
            cache.retain(|_, p| p.source.upgrade().is_some());
            for cached in cache.values() {
                nodes += cached.nodes;
                bytes += cached.bytes;
            }
            if cache.len() >= 64 || nodes > 4096 || bytes > 1024 * 1024 {
                return Ok(None);
            }
        }
        let all_parts = js_sys::Array::new();
        let owners = js_sys::Array::new();
        let virtual_parts = js_sys::Array::new();
        let fields = js_sys::Array::new();
        let mut parts: Vec<BTreeMap<Part, Node>> = Vec::with_capacity(shape.shapes.len());
        for (index, node) in shape.shapes.iter().enumerate() {
            let mut own: BTreeMap<Part, Node> = BTreeMap::new();
            for piece in &node.pieces {
                let part: Node = match &piece.kind {
                    Kind::Text(value) => document.create_text_node(value).into(),
                    Kind::Element {
                        tag, attributes, ..
                    } => {
                        let element = document
                            .create_element_ns(Some(glory_html::element_namespace(tag)), tag)
                            .map_err(backend)?;
                        for (name, value) in attributes {
                            if deferred(name) {
                                let field = js_sys::Array::new();
                                field.push(&all_parts.length().into());
                                field.push(&JsValue::from_str(name));
                                field.push(&JsValue::from_str(value));
                                fields.push(&field);
                            } else {
                                element.set_attribute(name, value).map_err(backend)?;
                            }
                        }
                        element.into()
                    }
                };
                if let Some(parent) = piece.parent {
                    own.get(&parent)
                        .expect("shape parent")
                        .append_child(&part)
                        .map_err(backend)?;
                }
                all_parts.push(&part);
                owners.push(&(index as u32).into());
                virtual_parts.push(&virtual_shape(node).into());
                own.insert(piece.part, part);
            }
            if let Some(parent) = shape.parents[index] {
                let slot = shape.shapes[parent]
                    .children
                    .expect("validated plan child slot");
                parts[parent]
                    .get(&slot)
                    .expect("parent part")
                    .append_child(own.get(&node.root_part()?).expect("physical root part"))
                    .map_err(backend)?;
            }
            parts.push(own);
        }
        let root = parts[0]
            .get(&shape.shapes[0].root_part()?)
            .expect("plan root");
        let prototype = Rc::new(Prototype {
            host: prototype(root, &all_parts, &owners, &virtual_parts, &fields).map_err(backend)?,
            layout: Layout::new(shape)?,
        });
        let (nodes, bytes) = shape.weight();
        self.0.borrow_mut().insert(
            key,
            Cached {
                source: plan.downgrade(),
                prototype: prototype.clone(),
                nodes,
                bytes,
            },
        );
        Ok(Some(prototype))
    }
    pub fn clear(&self) {
        self.0.borrow_mut().clear();
    }
}
impl Dom {
    /// A physical placement goal. JS resolves a Group's end anchor and inserts
    /// the original part, without returning any Node handle to Rust. Semantic
    /// parent/before/content validation has already completed in Mapper.
    pub(super) fn place_detached_plan(
        &self,
        entry: &Entry,
        parent: Option<NodeId>,
        before: Option<NodeId>,
    ) -> Result<bool, Error> {
        let Boundary::Part(part) = entry.boundary else {
            return Ok(false);
        };
        let Some(target) = entry.parts.target(part) else {
            return Ok(false);
        };
        let parent = parent.map(|id| self.entry(id)).transpose()?;
        let before = before.map(|id| self.entry(id)).transpose()?;
        let root_end = self.root_end.borrow();
        let destination = if let Some(parent) = &parent {
            if let Some(part) = parent.children {
                Destination::Children(
                    parent
                        .parts
                        .get(&part)
                        .ok_or(Error::Unsupported("HTML part"))?,
                )
            } else if parent.parts.is_empty() {
                Destination::RangeEnd(parent.last())
            } else {
                return Err(Error::InvalidProps("node has no HTML child slot"));
            }
        } else if let Some(end) = root_end.as_ref() {
            Destination::RangeEnd(end)
        } else {
            Destination::Children(self.root.as_ref())
        };
        target.place_detached(destination, before.as_ref().map(|entry| entry.first()))
    }
    pub(super) fn create_plan(
        &self,
        plan: &Plan,
        ids: &[NodeId],
        keys: &[u64],
        shapes: &PlanShape,
    ) -> Result<(), Error> {
        shapes.validate_instance(plan, ids, keys)?;
        let Some(prototype) = self.plans.get(&self.document, plan, shapes)? else {
            for (index, shape) in shapes.shapes.iter().enumerate() {
                self.create(ids[index], keys[index], shape.clone())?;
                if let Some(parent) = shapes.parents[index] {
                    self.place(ids[index], Some(ids[parent]), None)?;
                }
            }
            return Ok(());
        };
        let mut tokens = String::with_capacity(ids.len() * 24);
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
        let values = clone_plan(&prototype.host, &tokens, self.live.get()).map_err(backend)?;
        let instance = Rc::new(Instance {
            prototype,
            values,
            retirement: RefCell::new(Retirement::new(ids.len())),
        });
        let mut entries = Vec::with_capacity(ids.len());
        for (index, shape) in shapes.shapes.iter().enumerate() {
            let own = Parts::planned(instance.clone(), index);
            let entry = self.make_entry(shape, own, None)?;
            entry
                .parent
                .set(shapes.parents[index].map(|parent| ids[parent]));
            entries.push(entry);
        }
        // All fallible host work completed while detached. Register the whole
        // instance together; subsequent internal edges need no physical writes.
        let mut table = self.entries.borrow_mut();
        for (id, entry) in ids.iter().zip(entries) {
            if entry.edit == glory_html::EditKind::Rich {
                self.rich_live.borrow_mut().insert(*id);
            }
            table.insert(*id, entry);
        }
        Ok(())
    }
}
