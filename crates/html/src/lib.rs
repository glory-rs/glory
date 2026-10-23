//! Common HTML lowering. No UI/core runtime, DOM, HTTP or window dependencies.
mod mapping;
mod patch;
mod plan;
pub use plan::PlanShape;
#[cfg(test)]
mod patch_tests;
mod semantics;
mod serialize;
#[cfg(feature = "css")]
mod sheets;
mod styles;
#[cfg(feature = "css")]
pub use glory_style::Sheet;
#[cfg(feature = "css")]
pub use sheets::sheet;

use glory_render::memory::{Memory, Subtree};
use glory_render::{Error, Mutation, NodeId, Receipt, Render, RuntimeId};
use std::collections::{BTreeMap, HashMap};

pub use mapping::{preflight, shape};
pub use serialize::{escape, serialize};
pub use styles::style;
pub const FORMAT_VERSION: u32 = 3;

pub const BASE_CSS: &str = concat!(
    "[data-gly-layer]:not(:popover-open){display:none!important}",
    "[data-gly-layer]{position:fixed;inset:auto;margin:0;max-width:calc(100vw - 24px);max-height:calc(100dvh - 24px);overflow:auto}",
    "[data-gly-layer=center]{top:50%;left:50%;transform:translate(-50%,-50%)}",
    "[data-gly-layer=left]{left:0;top:0;height:100dvh;max-height:100dvh}",
    "[data-gly-layer=right]{right:0;top:0;height:100dvh;max-height:100dvh}",
    "[data-gly-layer=top]{left:0;top:0;width:100vw;max-width:100vw}",
    "[data-gly-layer=bottom]{left:0;bottom:0;width:100vw;max-width:100vw}",
    "[data-gly-layer^=anchor-]{left:var(--gly-layer-x,12px);top:var(--gly-layer-y,12px)}",
    "[data-gly-layer][aria-modal=true]::backdrop{background:rgba(0,0,0,.35)}",
    "[data-gly-hidden]{display:none!important}",
    "[data-gly-kind]{box-sizing:border-box}[data-gly-kind=text]{margin:0;font:inherit}",
    "[data-gly-kind=button],[data-gly-kind=input],[data-gly-kind=check] input{font:inherit}",
    "[data-gly-virtual=surface]:not([data-gly-live]){height:auto!important;overflow:visible!important}",
    "[data-gly-virtual=row]:not([data-gly-live]){display:flow-root!important;position:relative!important;top:auto!important;left:auto!important}",
);

/// HTML parsing canonicalizes CR/CRLF and NUL. DOM creation must do so too.
pub fn normalize(value: &str) -> String {
    glory_render::normalize_text(value, true).into_owned()
}

/// Escape already serialized JSON for an HTML script raw-text body. HTML
/// entity escaping is wrong here: script text does not decode &quot;/&lt;.
pub fn escape_script_data(json: &str) -> String {
    let mut escaped = String::with_capacity(json.len());
    for ch in json.chars() {
        match ch {
            '<' => escaped.push_str("\\u003c"),
            '>' => escaped.push_str("\\u003e"),
            '&' => escaped.push_str("\\u0026"),
            '\u{2028}' => escaped.push_str("\\u2028"),
            '\u{2029}' => escaped.push_str("\\u2029"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Part {
    Main,
    Control,
    Content,
}
impl Part {
    pub fn name(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Control => "control",
            Self::Content => "content",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Property {
    Text(String),
    Bool(bool),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Element {
        tag: &'static str,
        attributes: BTreeMap<&'static str, String>,
        properties: BTreeMap<&'static str, Property>,
    },
    Text(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    pub part: Part,
    pub parent: Option<Part>,
    pub kind: Kind,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditKind {
    #[default]
    None,
    Text,
    Check,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shape {
    /// Empty pieces describe a comment-delimited Group, not an element.
    pub pieces: Vec<Piece>,
    pub children: Option<Part>,
    pub control: Option<Part>,
    pub edit: EditKind,
    pub revision: u64,
    pub prevent_activate_default: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    Revision(u64),
    Attribute {
        part: Part,
        name: &'static str,
        value: Option<String>,
    },
    Property {
        part: Part,
        name: &'static str,
        value: Property,
    },
    Text {
        part: Part,
        value: String,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Create {
        id: NodeId,
        key: u64,
        shape: Shape,
    },
    Patch {
        id: NodeId,
        edits: Vec<Edit>,
    },
    Place {
        id: NodeId,
        parent: Option<NodeId>,
        before: Option<NodeId>,
    },
    Suspend {
        id: NodeId,
    },
    Dispose {
        id: NodeId,
        subtree: Vec<NodeId>,
    },
}
pub trait Sink {
    fn apply(&mut self, op: Op) -> Result<(), Error>;
    /// A detached semantic construction unit. Hosts without subtree cloning
    /// expand it into their existing finite operations; stream v3 stays intact.
    fn create_plan(
        &mut self,
        plan: &glory_render::Plan,
        ids: &[NodeId],
        keys: &[u64],
        shape: &PlanShape,
    ) -> Result<(), Error> {
        shape.validate_instance(plan, ids, keys)?;
        for (index, node) in shape.shapes.iter().enumerate() {
            self.apply(Op::Create {
                id: ids[index],
                key: keys[index],
                shape: node.clone(),
            })?;
            if let Some(parent) = shape.parents[index] {
                self.apply(Op::Place {
                    id: ids[index],
                    parent: Some(ids[parent]),
                    before: None,
                })?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextTag {
    Span,
    Code,
    Kbd,
    Mark,
    P,
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
}
#[derive(Clone, Debug, Default)]
pub struct HtmlTextOptions {
    pub tag: Option<TextTag>,
}
impl HtmlTextOptions {
    pub fn tag(mut self, tag: TextTag) -> Self {
        self.tag = Some(tag);
        self
    }
}
#[derive(Clone, Debug, Default)]
pub struct HtmlEventOptions {
    pub prevent_default: bool,
}
impl HtmlEventOptions {
    pub fn prevent_default(mut self, value: bool) -> Self {
        self.prevent_default = value;
        self
    }
}

pub struct Mapper {
    tree: Memory,
    keys: HashMap<NodeId, u64>,
    next_key: u64,
    receipt: Receipt,
    plans: plan::Plans,
}
impl Mapper {
    pub fn new(runtime: RuntimeId) -> Self {
        Self {
            tree: Memory::new(runtime),
            keys: HashMap::new(),
            next_key: 0,
            receipt: Receipt::default(),
            plans: plan::Plans::default(),
        }
    }
    pub fn tree(&self) -> &Memory {
        &self.tree
    }
    pub fn key(&self, id: NodeId) -> Option<u64> {
        self.keys.get(&id).copied()
    }
    /// Last successful prefix, including when a later mutation failed.
    pub fn receipt(&self) -> Receipt {
        self.receipt
    }
    pub fn html(&self) -> Result<String, Error> {
        serialize(&self.tree, |id| self.key(id))
    }
    /// Final snapshot keys follow the logical tree, independent of asynchronous
    /// completion/creation order. Hidden/unplaced nodes get unique trailing
    /// keys; only visible tree keys participate in HTML adoption. A streaming
    /// consumer uses the returned pairs to relabel metadata without remounting.
    pub fn canonicalize(&mut self) -> Result<Vec<(u64, u64)>, Error> {
        let mut order = Vec::with_capacity(self.keys.len());
        let mut seen = std::collections::HashSet::new();
        let mut pending: Vec<_> = self.tree.roots().iter().rev().copied().collect();
        while let Some(id) = pending.pop() {
            if self.tree.node(id)?.suspended {
                continue;
            }
            if !seen.insert(id) {
                continue;
            }
            order.push(id);
            pending.extend(self.tree.node(id)?.children.iter().rev().copied());
        }
        let mut trailing: Vec<_> = self
            .keys
            .iter()
            .filter(|(id, _)| !seen.contains(id))
            .map(|(id, key)| (*key, *id))
            .collect();
        trailing.sort_by_key(|(key, _)| *key);
        order.extend(trailing.into_iter().map(|(_, id)| id));
        let mut pairs = Vec::with_capacity(order.len());
        for (index, id) in order.into_iter().enumerate() {
            let key = u64::try_from(index)
                .map_err(|_| Error::InvalidProps("HTML key space exhausted"))?;
            let old = self.keys.insert(id, key).expect("live HTML key");
            pairs.push((old, key));
        }
        self.next_key = u64::try_from(self.keys.len())
            .map_err(|_| Error::InvalidProps("HTML key space exhausted"))?;
        Ok(pairs)
    }
    pub fn apply(
        &mut self,
        mutations: &[Mutation],
        sink: &mut impl Sink,
    ) -> Result<Receipt, Error> {
        let mut receipt = self.receipt;
        for mutation in mutations {
            let op = match mutation {
                Mutation::CreatePlan { plan, ids } => {
                    preflight_place(&self.tree, Subtree::Plan(plan), None)?;
                    let shape = self.plans.shape(plan)?;
                    let end = self
                        .next_key
                        .checked_add(ids.len() as u64)
                        .ok_or(Error::InvalidProps("HTML key space exhausted"))?;
                    receipt = self.tree.apply(std::slice::from_ref(mutation))?;
                    let keys: Vec<_> = (self.next_key..end).collect();
                    for (id, key) in ids.iter().zip(&keys) {
                        self.keys.insert(*id, *key);
                    }
                    self.next_key = end;
                    sink.create_plan(plan, ids, &keys, &shape)?;
                    self.receipt = receipt;
                    continue;
                }
                Mutation::Create { id, node } => {
                    let shape = shape(node)?;
                    let key = self.next_key;
                    self.next_key = self
                        .next_key
                        .checked_add(1)
                        .ok_or(Error::InvalidProps("HTML key space exhausted"))?;
                    receipt = self.tree.apply(std::slice::from_ref(mutation))?;
                    self.keys.insert(*id, key);
                    Op::Create {
                        id: *id,
                        key,
                        shape,
                    }
                }
                Mutation::Patch { id, patch } => {
                    let descriptor = &self.tree.node(*id)?.descriptor;
                    preflight(descriptor, Some(patch))?;
                    let prepared = self.tree.prepare_patch(*id, patch)?;
                    let edits = patch::lower(prepared.descriptor(), prepared.previous());
                    receipt = prepared.commit();
                    Op::Patch { id: *id, edits }
                }
                Mutation::Place { id, parent, before }
                | Mutation::Resume { id, parent, before } => {
                    self.tree.validate_placement(
                        *id,
                        *parent,
                        *before,
                        matches!(mutation, Mutation::Resume { .. }),
                    )?;
                    preflight_place(&self.tree, Subtree::Existing(*id), *parent)?;
                    receipt = self.tree.apply(std::slice::from_ref(mutation))?;
                    let siblings = if let Some(parent) = parent {
                        &self.tree.node(*parent)?.children
                    } else {
                        self.tree.roots()
                    };
                    let before = before
                        .and_then(|before| siblings.iter().position(|id| *id == before))
                        .and_then(|index| {
                            siblings[index..]
                                .iter()
                                .copied()
                                .find(|id| !self.tree.node(*id).expect("sibling").suspended)
                        });
                    Op::Place {
                        id: *id,
                        parent: *parent,
                        before,
                    }
                }
                Mutation::Suspend { id } => {
                    receipt = self.tree.apply(std::slice::from_ref(mutation))?;
                    Op::Suspend { id: *id }
                }
                Mutation::Dispose { id } => {
                    let mut subtree = Vec::new();
                    let mut pending = vec![*id];
                    while let Some(id) = pending.pop() {
                        pending.extend(self.tree.node(id)?.children.iter().copied());
                        subtree.push(id);
                    }
                    receipt = self.tree.apply(std::slice::from_ref(mutation))?;
                    for id in &subtree {
                        self.keys.remove(id);
                    }
                    Op::Dispose { id: *id, subtree }
                }
            };
            sink.apply(op)?;
            self.receipt = receipt;
        }
        Ok(receipt)
    }
}

/// HTML ancestor checks over the supplied staged semantic tree, never a host
/// lookup. New children need no subtree allocation; ordinary list moves only
/// walk ancestors. A button destination additionally examines live descendants.
pub fn preflight_place(
    tree: &Memory,
    child: Subtree<'_>,
    parent: Option<NodeId>,
) -> Result<(), Error> {
    if let Subtree::Existing(id) = child {
        tree.node(id)?;
    }
    let mut cursor = parent;
    let mut button = false;
    while let Some(ancestor) = cursor {
        if matches!(child, Subtree::Existing(id) if ancestor == id) {
            return Err(Error::InvalidPlacement("tree cycle"));
        }
        let node = tree.node(ancestor)?;
        button |= matches!(node.descriptor.props, glory_render::Props::Button(_));
        cursor = node.parent;
    }
    // Semantic Inline text may explicitly map to <p>. That extension must
    // not bypass the button's phrasing-only content model, including when
    // a previously built Group is moved into a button. Ordinary list moves
    // do not scan their subtree.
    if button || matches!(child, Subtree::Plan(_)) {
        let check = |descriptor: &glory_render::Descriptor| {
            if matches!(descriptor.props, glory_render::Props::Text(_))
                && descriptor
                    .options
                    .get::<HtmlTextOptions>()
                    .is_some_and(|options| options.tag == Some(TextTag::P))
            {
                return Err(Error::InvalidProps(
                    "button text cannot map to an HTML paragraph",
                ));
            }
            Ok(())
        };
        match child {
            Subtree::Plan(plan) => {
                for (index, node) in plan.nodes().iter().enumerate() {
                    let mut ancestor = plan.nodes()[index].parent;
                    let mut in_button = button;
                    while let Some(parent) = ancestor {
                        in_button |= matches!(
                            plan.nodes()[parent].descriptor.props,
                            glory_render::Props::Button(_)
                        );
                        ancestor = plan.nodes()[parent].parent;
                    }
                    if in_button {
                        check(&node.descriptor)?;
                    }
                }
            }
            Subtree::New(descriptor) => check(descriptor)?,
            Subtree::Existing(id) => {
                let mut pending = vec![id];
                while let Some(id) = pending.pop() {
                    let node = tree.node(id)?;
                    check(&node.descriptor)?;
                    pending.extend(node.children.iter().copied());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
fn difference(
    old: &Shape,
    new: &Shape,
    checked_write: bool,
    mode_write: bool,
) -> Result<Vec<Edit>, Error> {
    if old.pieces.len() != new.pieces.len()
        || old.control != new.control
        || old.edit != new.edit
        || old.prevent_activate_default != new.prevent_activate_default
    {
        return Err(Error::InvalidProps(
            "HTML structure changes require remount",
        ));
    }
    let mut edits = Vec::new();
    let revision = old.revision != new.revision;
    if revision {
        edits.push(Edit::Revision(new.revision));
    }
    for (old, new) in old.pieces.iter().zip(&new.pieces) {
        if old.part != new.part || old.parent != new.parent {
            return Err(Error::InvalidProps(
                "HTML structure changes require remount",
            ));
        }
        match (&old.kind, &new.kind) {
            (Kind::Text(old), Kind::Text(new_value)) => {
                if old != new_value {
                    edits.push(Edit::Text {
                        part: new.part,
                        value: new_value.clone(),
                    });
                }
            }
            (
                Kind::Element {
                    tag: a,
                    attributes: old,
                    properties: old_props,
                },
                Kind::Element {
                    tag: b,
                    attributes: attrs,
                    properties,
                },
            ) if a == b => {
                for (name, value) in attrs {
                    if old.get(name) != Some(value) {
                        edits.push(Edit::Attribute {
                            part: new.part,
                            name,
                            value: Some(value.clone()),
                        });
                    }
                }
                for name in old.keys() {
                    if !attrs.contains_key(name) {
                        edits.push(Edit::Attribute {
                            part: new.part,
                            name,
                            value: None,
                        });
                    }
                }
                for (name, value) in properties {
                    if old_props.get(name) != Some(value)
                        || revision && *name == "value"
                        || checked_write && *name == "checked"
                        || *name == "checked" && old.get("type") != attrs.get("type")
                        || mode_write && *name == "indeterminate"
                    {
                        edits.push(Edit::Property {
                            part: new.part,
                            name,
                            value: value.clone(),
                        });
                    }
                }
            }
            _ => return Err(Error::InvalidProps("HTML tag changes require remount")),
        }
    }
    Ok(edits)
}
