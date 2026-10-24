//! Document-owned, bounded prototypes of explicitly declared semantic plans.
//! Only static declarations enter prototypes; identities and bindings do not.
use super::{Dom, Parts, backend, virtual_shape};
use glory_html::{Kind, Part, PlanShape};
use glory_render::{Error, NodeId, Plan, WeakPlan};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    fmt::Write,
};
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};
use web_sys::{Document, Node};

// Private experiment boundary: the generic build changes only this constant.
const DEEP_CLONE: bool = true;

#[wasm_bindgen(inline_js = "
export function gloryPlan(root, parts, owners, virtualParts, fields) {
    const paths = parts.map(part => {
        const path = [];
        for (let node = part; node !== root; node = node.parentNode) {
            let index = 0;
            for (let previous = node.previousSibling; previous; previous = previous.previousSibling) index++;
            path.unshift(index);
        }
        return path;
    });
    return { root, paths, owners, virtualParts, fields };
}
export function gloryClonePlan(plan, tokens, live) {
    const root = plan.root.cloneNode(true);
    const parts = plan.paths.map(path => {
        let node = root;
        for (const index of path) node = node.childNodes[index];
        return node;
    });
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
}
struct Cached {
    source: WeakPlan,
    prototype: JsValue,
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
    ) -> Result<Option<JsValue>, Error> {
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
                    .append_child(own.get(&Part::Main).expect("main part"))
                    .map_err(backend)?;
            }
            parts.push(own);
        }
        let root = parts[0].get(&Part::Main).expect("plan root");
        let prototype =
            prototype(root, &all_parts, &owners, &virtual_parts, &fields).map_err(backend)?;
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
        let parts = clone_plan(&prototype, &tokens, self.live.get()).map_err(backend)?;
        let mut offset = 0;
        let mut entries = Vec::with_capacity(ids.len());
        for (index, shape) in shapes.shapes.iter().enumerate() {
            let mut own = Parts::default();
            for piece in &shape.pieces {
                own.insert(piece.part, parts.get(offset).unchecked_into());
                offset += 1;
            }
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
            table.insert(*id, entry);
        }
        Ok(())
    }
}
