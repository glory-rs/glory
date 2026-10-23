use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use blitz_dom::{BaseDocument, DocumentConfig, NodeId as EngineId, QualName, ScrollBehavior};
use blitz_traits::shell::Viewport;
use glory_html::{Edit, EditKind, Kind, Mapper, Op, Part, Property, Shape, Sink};
use glory_render::normalize_text;
use glory_render::profile::{Phase, Profile};
use glory_render::{
    Capabilities, Error, Event, EventKind, Input, Mutation, NodeId, Query, QueryToken, QueryValue,
    Receipt, Rect, Render, RuntimeId,
};
use std::borrow::Cow;
use std::sync::Arc;
#[path = "resource_wake.rs"]
mod resource_wake;
use style::computed_values::visibility::T as Visibility;

/// Typed inline acceptance for the pinned native engine. HTML-family CSS still
/// goes to Stylo, but parsing a declaration does not establish its layout
/// behaviour: beta.4 does not implement sticky offsets or automatic scroll
/// anchoring. Explicitly disabling anchoring (used by virtual lists) is valid.
/// Re-audit these exclusions against real geometry before widening the profile.
pub const STYLE_PROFILE: glory_render::style::Profile = glory_render::style::Profile {
    name: "blitz",
    version: 4,
    inline: glory_render::style::Support::ALL
        .without_value(glory_render::style::Value::Sticky)
        .without_value(glory_render::style::Value::AnchorAuto),
    css: glory_render::style::Css::Html,
};

fn check_style(style: &glory_render::style::Style) -> Result<(), Error> {
    style.validate().map_err(Error::InvalidProps)?;
    STYLE_PROFILE
        .inline
        .check(style)
        .map_err(|requirement| Error::UnsupportedStyle {
            profile: STYLE_PROFILE.name,
            version: STYLE_PROFILE.version,
            requirement,
        })
}

/// Native document configuration. No network/filesystem access is installed by
/// default. A provider must queue work and honor cancel/close without UI reentry.
#[derive(Clone)]
pub struct Config {
    pub base_url: blitz_traits::net::Url,
    pub resources: blitz_dom::ResourceLimits,
    pub provider: Option<Arc<dyn blitz_traits::net::NetProvider>>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            base_url: blitz_traits::net::Url::parse("glory://app/").expect("built-in base URL"),
            resources: blitz_dom::ResourceLimits::default(),
            provider: None,
        }
    }
}

#[cfg(feature = "paint")]
#[path = "paint.rs"]
mod paint;
#[cfg(feature = "paint")]
pub use paint::*;
#[cfg(all(
    feature = "shell",
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]
#[path = "shell.rs"]
pub mod shell;

fn tag(name: &str) -> QualName {
    QualName::new(
        None,
        blitz_dom::Namespace::from("http://www.w3.org/1999/xhtml"),
        blitz_dom::LocalName::from(name),
    )
}
fn attr(name: &str) -> QualName {
    QualName::new(
        None,
        blitz_dom::Namespace::from(""),
        blitz_dom::LocalName::from(name),
    )
}
fn invariant(message: &'static str) -> Error {
    Error::Backend(format!("Blitz range: {message}"))
}

struct Range {
    first: EngineId,
    last: EngineId,
    parts: BTreeMap<Part, EngineId>,
    children: Option<Part>,
    control: Option<Part>,
    edit: EditKind,
    editing: Editing,
    properties: BTreeMap<(Part, &'static str), Property>,
    pending: BTreeSet<(Part, &'static str)>,
    value_seed: Option<EngineId>,
    /// A disconnected parent for a multi-node range. It never enters layout.
    park: Option<EngineId>,
}
#[derive(Default)]
struct Editing {
    composing: bool,
    revision: u64,
    requested: u64,
    pending: Option<String>,
    // Winit sends empty Preedit before Commit, but also on cancellation. End
    // composition immediately; remember whether a later program write replaced
    // its base, so the trailing native Commit cannot insert into that new value.
    after_preedit: Option<bool>,
    replacement: Option<(String, bool)>,
}
impl Range {
    fn part(&self, part: Part) -> Result<EngineId, Error> {
        self.parts
            .get(&part)
            .copied()
            .ok_or_else(|| invariant("missing part"))
    }
}
struct Dom {
    doc: BaseDocument,
    body: EngineId,
    css: EngineId,
    ranges: HashMap<NodeId, Range>,
    owners: HashMap<EngineId, NodeId>,
    controls: HashMap<EngineId, NodeId>,
    dirty: bool,
    time: f64,
    revision: u64,
    layouts: u64,
    property_writes: u64,
    resource_wake: Arc<resource_wake::ResourceWake>,
    profile: Option<Profile>,
}

/// Import after all engine workers have joined, including during unwinding.
/// The fixed snapshot owns no document/node/text and invokes no engine callback.
struct EngineRecording {
    engine: blitz_dom::profile::Profile,
    recording: glory_render::profile::Recording,
}
impl Drop for EngineRecording {
    fn drop(&mut self) {
        use blitz_dom::profile::Phase as Engine;
        for (phase, sample) in self.engine.snapshot().iter() {
            if sample.calls == 0 {
                continue;
            }
            let phase = match phase {
                Engine::Style => Phase::Style,
                Engine::Damage => Phase::Damage,
                Engine::Construct => Phase::Construct,
                Engine::PostConstruct => Phase::PostConstruct,
                Engine::BoxLayout => Phase::BoxLayout,
                Engine::Transforms => Phase::Transform,
                Engine::PaintTree => Phase::PaintTree,
                Engine::Cleanup => Phase::LayoutCleanup,
                Engine::Hover => Phase::Hover,
                Engine::Subdocuments => Phase::Subdocuments,
                Engine::TextShape => Phase::TextShape,
                Engine::TextBreak => Phase::TextBreak,
                Engine::TextAlign => Phase::TextAlign,
            };
            self.recording.record(
                phase,
                glory_render::profile::Sample {
                    calls: sample.calls,
                    units: sample.units,
                    elapsed: sample.elapsed,
                    max: sample.max,
                    invalid_clock: 0,
                },
            );
        }
    }
}
impl Dom {
    fn new(config: Config) -> Self {
        let resource_wake = Arc::new(resource_wake::ResourceWake::default());
        let mut doc = BaseDocument::new(DocumentConfig {
            viewport: Some(Viewport::new(800, 600, 1.0, Default::default())),
            incremental: Some(true),
            base_url: Some(config.base_url.to_string()),
            net_provider: config.provider.map(|inner| {
                Arc::new(resource_wake::Provider {
                    inner,
                    wake: resource_wake.clone(),
                }) as Arc<dyn blitz_traits::net::NetProvider>
            }),
            ..DocumentConfig::default()
        });
        doc.set_resource_limits(config.resources)
            .expect("new document has no resources");
        let root = doc.root_node().id;
        let (body, css) = {
            let mut m = doc.mutate();
            let html = m.create_element(tag("html"), Vec::new());
            let head = m.create_element(tag("head"), Vec::new());
            let body = m.create_element(tag("body"), Vec::new());
            let base = m.create_element(tag("style"), Vec::new());
            let baseline = m.create_text_node(&format!(
                "html,body{{margin:0;padding:0}}{}",
                glory_html::BASE_CSS
            ));
            let sheet = m.create_element(tag("style"), Vec::new());
            let css = m.create_text_node("");
            m.append_children(root, &[html]);
            m.append_children(html, &[head, body]);
            m.append_children(head, &[base, sheet]);
            m.append_children(base, &[baseline]);
            m.append_children(sheet, &[css]);
            (body, css)
        };
        Self {
            doc,
            body,
            css,
            ranges: HashMap::new(),
            owners: HashMap::new(),
            controls: HashMap::new(),
            dirty: true,
            time: 0.0,
            revision: 1,
            layouts: 0,
            property_writes: 0,
            resource_wake,
            profile: None,
        }
    }
    fn range(&self, id: NodeId) -> Result<&Range, Error> {
        self.ranges.get(&id).ok_or(Error::NodeGone(id))
    }
    fn nodes(&self, id: NodeId) -> Result<Vec<EngineId>, Error> {
        let range = self.range(id)?;
        if range.first == range.last {
            return Ok(vec![range.first]);
        }
        let first = self
            .doc
            .get_node(range.first)
            .ok_or_else(|| invariant("missing first anchor"))?;
        let parent = first
            .parent
            .and_then(|id| self.doc.get_node(id))
            .ok_or_else(|| invariant("unparented anchors"))?;
        let start = parent
            .children
            .iter()
            .position(|node| *node == range.first)
            .ok_or_else(|| invariant("missing start"))?;
        let end = parent
            .children
            .iter()
            .position(|node| *node == range.last)
            .filter(|end| *end >= start)
            .ok_or_else(|| invariant("missing end"))?;
        Ok(parent.children[start..=end].to_vec())
    }
    fn create(&mut self, id: NodeId, shape: Shape) -> Result<(), Error> {
        let mut m = self.doc.mutate();
        let mut parts = BTreeMap::new();
        let mut properties = BTreeMap::new();
        let mut value_seed = None;
        let (first, last, park) = if shape.pieces.is_empty() {
            let park = m.create_element(tag("gly-park"), Vec::new());
            let first = m.create_comment_node("gly:group");
            let last = m.create_comment_node("gly:/group");
            m.append_children(park, &[first, last]);
            (first, last, Some(park))
        } else {
            for piece in shape.pieces {
                let node = match piece.kind {
                    Kind::Text(text) => m.create_text_node(&text),
                    Kind::Element {
                        tag: name,
                        attributes,
                        properties: props,
                    } => {
                        let node = m.create_element(tag(name), Vec::new());
                        if name == "textarea" {
                            // Blitz creates its Parley editor lazily at layout,
                            // reading a textarea's child text rather than value.
                            // This private seed also covers changes before the
                            // first layout; it is not a semantic child or label.
                            let value = match props.get("value") {
                                Some(Property::Text(value)) => value.as_str(),
                                _ => "",
                            };
                            let seed = m.create_text_node(value);
                            m.append_children(node, &[seed]);
                            value_seed = Some(seed);
                        }
                        for (name, value) in attributes {
                            m.set_attribute(node, attr(name), &value);
                            if name == "data-gly-virtual" {
                                // Unlike SSR preview, this document has a layout engine.
                                m.set_attribute(node, attr("data-gly-live"), "");
                            }
                        }
                        for (name, value) in props {
                            properties.insert((piece.part, name), value);
                        }
                        node
                    }
                };
                if let Some(parent) = piece.parent {
                    let parent = parts
                        .get(&parent)
                        .copied()
                        .ok_or_else(|| invariant("part order"))?;
                    m.append_children(parent, &[node]);
                }
                parts.insert(piece.part, node);
            }
            let main = parts
                .get(&Part::Main)
                .copied()
                .ok_or_else(|| invariant("no main part"))?;
            (main, main, None)
        };
        drop(m);
        if let Some(main) = parts.get(&Part::Main) {
            self.owners.insert(*main, id);
        }
        if let Some(control) = shape.control.and_then(|part| parts.get(&part)) {
            self.controls.insert(*control, id);
        }
        let pending = properties.keys().copied().collect();
        self.ranges.insert(
            id,
            Range {
                first,
                last,
                parts,
                children: shape.children,
                control: shape.control,
                edit: shape.edit,
                editing: Editing {
                    revision: shape.revision,
                    requested: shape.revision,
                    ..Editing::default()
                },
                properties,
                pending,
                value_seed,
                park,
            },
        );
        self.reflect(id)?;
        Ok(())
    }
    fn reflect_value(
        &mut self,
        id: NodeId,
        part: Part,
        name: &'static str,
        value: &Property,
    ) -> Result<(), Error> {
        let node = self.range(id)?.part(part)?;
        let value_seed = self.range(id)?.value_seed;
        let normalized = if name == "value"
            && let Property::Text(text) = value
        {
            let multiline = self
                .doc
                .get_node(node)
                .and_then(|node| node.element_data())
                .is_some_and(|element| &*element.name.local == "textarea");
            match normalize_text(text, multiline) {
                Cow::Borrowed(_) => None,
                Cow::Owned(value) => Some(Property::Text(value)),
            }
        } else {
            None
        };
        let value = normalized.as_ref().unwrap_or(value);
        if name == "value"
            && let Property::Text(value) = value
        {
            let current = self.text_value(id)?;
            let editing = &mut self.ranges.get_mut(&id).expect("live range").editing;
            if editing.composing && current != *value {
                editing.pending = Some(value.clone());
                return Ok(());
            }
            if current != *value
                && let Some(superseded) = &mut editing.after_preedit
            {
                *superseded = true;
            }
            editing.revision = editing.requested;
            editing.pending = None;
            // Echoes acknowledge the new revision without touching the editor,
            // preserving native selection, preedit and horizontal scroll.
            if current == *value {
                return Ok(());
            }
        }
        self.property_writes += 1;
        let mut m = self.doc.mutate();
        match (name, value) {
            ("value", Property::Text(value)) => {
                if let Some(seed) = value_seed {
                    m.set_node_text(seed, value);
                }
                m.set_attribute(node, attr("value"), value);
            }
            // Blitz beta.4 parses the controlled checked attribute as bool.
            // Empty HTML boolean attributes only initialize it on attachment.
            ("checked", Property::Bool(true)) => m.set_attribute(node, attr(name), "true"),
            ("disabled", Property::Bool(true)) => m.set_attribute(node, attr(name), ""),
            ("disabled" | "checked", Property::Bool(false)) => m.clear_attribute(node, attr(name)),
            _ => return Err(Error::Unsupported("Blitz form property")),
        }
        Ok(())
    }
    fn text_value(&self, id: NodeId) -> Result<String, Error> {
        let control = self.control(id)?;
        let element = self
            .doc
            .get_node(control)
            .and_then(|node| node.element_data())
            .ok_or_else(|| invariant("missing control"))?;
        Ok(element.text_input_data().map_or_else(
            || {
                element
                    .attr(blitz_dom::LocalName::from("value"))
                    .unwrap_or_default()
                    .to_owned()
            },
            |input| input.editor.raw_text().to_owned(),
        ))
    }
    fn finish_composition(
        &mut self,
        id: NodeId,
        committed: bool,
    ) -> Result<Option<(String, u64)>, Error> {
        let control = self.control(id)?;
        let range = self.ranges.get_mut(&id).ok_or(Error::NodeGone(id))?;
        let composing = std::mem::take(&mut range.editing.composing);
        let pending = range.editing.pending.take();
        let replacement = range.editing.replacement.take();
        let revision = range.editing.revision;
        if composing {
            if !committed {
                self.doc.with_text_input(control, |mut driver| {
                    driver.clear_compose();
                    if let Some((text, backwards)) = replacement {
                        let start = driver.editor.raw_selection().text_range().start;
                        driver.insert_or_replace_selection(&text);
                        if backwards {
                            driver.select_byte_range(start + text.len(), start);
                        } else {
                            driver.select_byte_range(start, start + text.len());
                        }
                    }
                });
            }
            self.invalidate();
        }
        let terminal = if composing {
            Some((self.text_value(id)?, revision))
        } else {
            None
        };
        if let Some(value) = pending {
            let part = self.range(id)?.control.expect("text control");
            self.reflect_value(id, part, "value", &Property::Text(value))?;
        }
        Ok(terminal)
    }
    fn reflect(&mut self, id: NodeId) -> Result<(), Error> {
        let properties: Vec<_> = self
            .range(id)?
            .pending
            .iter()
            .map(|key| {
                (
                    *key,
                    self.range(id).expect("live range").properties[key].clone(),
                )
            })
            .collect();
        for ((part, name), value) in properties {
            self.reflect_value(id, part, name, &value)?;
            let node = self.range(id)?.part(part)?;
            if self
                .doc
                .get_node(node)
                .is_some_and(|node| node.flags.is_in_document())
            {
                self.ranges
                    .get_mut(&id)
                    .expect("live range")
                    .pending
                    .remove(&(part, name));
            }
        }
        Ok(())
    }
    fn place(
        &mut self,
        id: NodeId,
        parent: Option<NodeId>,
        before: Option<NodeId>,
    ) -> Result<(), Error> {
        let nodes = self.nodes(id)?;
        let (parent, end) = if let Some(parent) = parent {
            let range = self.range(parent)?;
            if range.parts.is_empty() {
                (
                    self.doc
                        .get_node(range.last)
                        .and_then(|node| node.parent)
                        .ok_or_else(|| invariant("group has no parent"))?,
                    Some(range.last),
                )
            } else {
                (
                    range.part(range.children.ok_or_else(|| invariant("leaf parent"))?)?,
                    None,
                )
            }
        } else {
            (self.body, None)
        };
        let anchor = before
            .map(|id| self.range(id).map(|range| range.first))
            .transpose()?
            .or(end);
        let mut m = self.doc.mutate();
        if let Some(anchor) = anchor {
            m.insert_nodes_before(anchor, &nodes);
        } else {
            m.append_children(parent, &nodes);
        }
        drop(m);
        // Form behaviour (notably checked) is finalized on document insertion.
        // Replay only controlled state, never recreate the node or its children.
        let mut pending = nodes;
        let mut owned = Vec::new();
        while let Some(node) = pending.pop() {
            if let Some(owner) = self.owners.get(&node) {
                owned.push(*owner);
            }
            if let Some(node) = self.doc.get_node(node) {
                pending.extend(node.children.iter().copied());
            }
        }
        for owner in owned {
            self.reflect(owner)?;
        }
        Ok(())
    }
    fn suspend(&mut self, id: NodeId) -> Result<(), Error> {
        let nodes = self.nodes(id)?;
        if self.range(id)?.parts.is_empty() {
            let park = self
                .range(id)?
                .park
                .ok_or_else(|| invariant("group has no park"))?;
            self.doc.mutate().append_children(park, &nodes);
        } else {
            self.doc.mutate().remove_node(nodes[0]);
        }
        Ok(())
    }
    fn dispose(&mut self, id: NodeId, subtree: Vec<NodeId>) -> Result<(), Error> {
        let nodes = self.nodes(id)?;
        for node in nodes {
            self.doc.mutate().remove_and_drop_node(node);
        }
        // Suspended children are no longer physically under their logical parent.
        // Explicitly reclaim every range, including empty/offline Group parks.
        for child in subtree {
            if let Some(range) = self.ranges.remove(&child) {
                if let Some(control) = range.control.and_then(|part| range.parts.get(&part)) {
                    self.controls.remove(control);
                }
                if let Some(main) = range.parts.get(&Part::Main) {
                    self.owners.remove(main);
                }
                for node in [Some(range.first), Some(range.last), range.park]
                    .into_iter()
                    .flatten()
                {
                    if self.doc.get_node(node).is_some() {
                        self.doc.mutate().remove_and_drop_node(node);
                    }
                }
            }
        }
        Ok(())
    }
    fn resolve(&mut self) {
        if self.resources() {
            self.invalidate();
        }
        if self.dirty {
            let _span = self
                .profile
                .as_ref()
                .map(|profile| profile.span(Phase::Layout));
            let _engine = self.profile.as_ref().map(|profile| {
                let mut engine = self.doc.profile().expect("attached engine collector");
                if !engine.reset() {
                    // An exhausted interval never aliases an old span's epoch.
                    engine = blitz_dom::profile::Profile::default();
                    self.doc.set_profile(Some(engine.clone()));
                }
                EngineRecording {
                    engine,
                    recording: profile.recording(),
                }
            });
            self.doc.resolve(self.time);
            self.dirty = false;
            self.layouts += 1;
        }
    }
    fn resources(&mut self) -> bool {
        let _span = self
            .profile
            .as_ref()
            .map(|profile| profile.span(Phase::Resources));
        self.doc.handle_messages()
    }
    fn invalidate(&mut self) {
        self.dirty = true;
        self.invalidate_pixels();
    }
    fn invalidate_pixels(&mut self) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("Blitz visual revision exhausted");
    }
    fn main(&self, id: NodeId) -> Result<EngineId, Error> {
        self.range(id)?
            .parts
            .get(&Part::Main)
            .copied()
            .ok_or(Error::Unsupported("Blitz Group has no physical box"))
    }
    fn control(&self, id: NodeId) -> Result<EngineId, Error> {
        let range = self.range(id)?;
        range.part(
            range
                .control
                .ok_or(Error::Unsupported("Blitz Group has no control"))?,
        )
    }
    fn owner(&self, mut physical: EngineId) -> Option<NodeId> {
        loop {
            if let Some(id) = self
                .controls
                .get(&physical)
                .or_else(|| self.owners.get(&physical))
            {
                return Some(*id);
            }
            physical = self.doc.get_node(physical)?.parent?;
        }
    }
    fn selection(&self, id: NodeId) -> Result<QueryValue, Error> {
        if self.range(id)?.edit != EditKind::Text {
            return Err(Error::Unsupported("selection requires a text input"));
        }
        let node = self.control(id)?;
        let input = self
            .doc
            .get_node(node)
            .and_then(|node| node.element_data())
            .and_then(|element| element.text_input_data())
            .ok_or(Error::Unsupported("Blitz text editor unavailable"))?;
        let range = input.editor.raw_selection().text_range();
        let text = input.editor.raw_text();
        let start = text
            .get(..range.start)
            .ok_or_else(|| invariant("invalid selection boundary"))?
            .encode_utf16()
            .count();
        let end = text
            .get(..range.end)
            .ok_or_else(|| invariant("invalid selection boundary"))?
            .encode_utf16()
            .count();
        Ok(QueryValue::Selection { start, end })
    }
    fn focus_visible(&self, mut id: EngineId) -> bool {
        if !self.doc.get_node(id).is_some_and(|node| {
            node.has_boxes()
                && node
                    .primary_styles()
                    .is_none_or(|style| style.clone_visibility() == Visibility::Visible)
        }) {
            return false;
        }
        loop {
            let Some(node) = self.doc.get_node(id) else {
                return false;
            };
            if node
                .primary_styles()
                .is_some_and(|style| style.clone_display().is_none())
            {
                return false;
            }
            let Some(parent) = node.parent else {
                return true;
            };
            id = parent;
        }
    }
    fn geometry(&self, id: NodeId) -> Result<(Rect, Rect), Error> {
        let node = self.main(id)?;
        let layout = self
            .doc
            .get_node(node)
            .ok_or(Error::NodeGone(id))?
            .final_layout();
        let size = Rect {
            width: f64::from(layout.size.width),
            height: f64::from(layout.size.height),
            ..Rect::default()
        };
        let viewport = Rect {
            width: f64::from(
                (layout.size.width
                    - layout.border.left
                    - layout.border.right
                    - layout.scrollbar_size.width)
                    .max(0.0),
            ),
            height: f64::from(
                (layout.size.height
                    - layout.border.top
                    - layout.border.bottom
                    - layout.scrollbar_size.height)
                    .max(0.0),
            ),
            ..Rect::default()
        };
        Ok((size, viewport))
    }
}
impl Sink for Dom {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        match op {
            Op::Create { id, shape, .. } => self.create(id, shape)?,
            Op::Place { id, parent, before } => self.place(id, parent, before)?,
            Op::Suspend { id } => self.suspend(id)?,
            Op::Dispose { id, subtree } => self.dispose(id, subtree)?,
            Op::Patch { id, edits } => {
                if edits.is_empty() {
                    return Ok(());
                }
                for edit in edits {
                    match edit {
                        Edit::Revision(revision) => {
                            self.ranges
                                .get_mut(&id)
                                .ok_or(Error::NodeGone(id))?
                                .editing
                                .requested = revision
                        }
                        Edit::Text { part, value } => {
                            let node = self.range(id)?.part(part)?;
                            self.doc.mutate().set_node_text(node, &value);
                        }
                        Edit::Attribute { part, name, value } => {
                            let range = self.range(id)?;
                            if (range.edit == EditKind::Text && name == "value")
                                || (range.edit == EditKind::Check && name == "checked")
                            {
                                // Controlled properties below are the single
                                // editor/checkbox write path, including IME.
                                continue;
                            }
                            let node = self.range(id)?.part(part)?;
                            let mut m = self.doc.mutate();
                            if let Some(value) = value {
                                m.set_attribute(node, attr(name), &value);
                            } else {
                                m.clear_attribute(node, attr(name));
                            }
                        }
                        Edit::Property { part, name, value } => {
                            self.reflect_value(id, part, name, &value)?;
                            let node = self.range(id)?.part(part)?;
                            let attached = self
                                .doc
                                .get_node(node)
                                .is_some_and(|node| node.flags.is_in_document());
                            let range = self.ranges.get_mut(&id).expect("validated range");
                            range.properties.insert((part, name), value);
                            if attached {
                                range.pending.remove(&(part, name));
                            } else {
                                range.pending.insert((part, name));
                            }
                        }
                    }
                }
            }
        }
        self.invalidate();
        Ok(())
    }
}

struct State {
    #[cfg(feature = "paint")]
    runtime: RuntimeId,
    mapper: Mapper,
    dom: Dom,
    receipt: Receipt,
    interests: BTreeMap<(NodeId, EventKind), Option<Event>>,
    scroll_requests: BTreeMap<NodeId, glory_render::ScrollAck>,
    inputs: Vec<Input>,
    observed_revision: u64,
    observation_pending: bool,
    interactive: bool,
    focused: Option<NodeId>,
    subscriptions: BTreeSet<(NodeId, EventKind)>,
    input_error: Option<Error>,
    space: Option<NodeId>,
}
impl State {
    fn end_editing(&mut self, node: NodeId) -> Result<(), Error> {
        if !self
            .dom
            .ranges
            .get(&node)
            .is_some_and(|range| range.edit == EditKind::Text)
        {
            return Ok(());
        }
        self.dom
            .ranges
            .get_mut(&node)
            .expect("live range")
            .editing
            .after_preedit = None;
        if let Some((value, revision)) = self.dom.finish_composition(node, false)? {
            self.inputs.push(Input::Event {
                node,
                event: Event::Composition { active: false },
            });
            self.inputs.push(Input::Event {
                node,
                event: Event::ValueChanged {
                    value,
                    composing: false,
                    revision,
                },
            });
        }
        Ok(())
    }
    fn focus_changed(&mut self, next: Option<NodeId>) {
        if self.focused == next {
            return;
        }
        self.space = None;
        if let Some(node) = self.focused.take() {
            if let Err(error) = self.end_editing(node) {
                self.input_error = Some(error);
            }
            self.inputs.push(Input::Event {
                node,
                event: Event::FocusChanged(false),
            });
        }
        self.focused = next;
        if let Some(node) = next {
            self.inputs.push(Input::Event {
                node,
                event: Event::FocusChanged(true),
            });
        }
    }
    fn sync_focus(&mut self) {
        if !self.interactive {
            return;
        }
        let physical = self.dom.doc.get_focussed_node_id();
        let next = physical.and_then(|node| self.dom.owner(node));
        if let Some(next) = next
            && (!self.mapper.tree().active(next)
                || physical.is_some_and(|node| !self.dom.focus_visible(node))
                || !self
                    .mapper
                    .tree()
                    .node(next)
                    .is_ok_and(|node| node.descriptor.props.enabled()))
        {
            self.dom.doc.clear_focus();
            self.dom.invalidate();
            self.focus_changed(None);
        } else {
            self.focus_changed(next);
        }
    }
    fn focus(&mut self, id: NodeId) -> Result<(), Error> {
        if !self.interactive {
            return Err(Error::Unsupported("headless Blitz focus"));
        }
        let descriptor = &self.mapper.tree().node(id)?.descriptor;
        if !descriptor.props.enabled() {
            return Err(Error::Unsupported("focus on disabled control"));
        }
        let explicit = descriptor.focus != glory_render::FocusMode::Auto;
        let control = self.dom.control(id)?;
        self.dom.resolve();
        if !self.dom.focus_visible(control) {
            return Err(Error::Unsupported("focus on hidden control"));
        }
        if !explicit
            && !self
                .dom
                .doc
                .get_node(control)
                .is_some_and(|node| node.is_focussable())
        {
            return Err(Error::Unsupported("node is not focusable"));
        }
        if self.dom.doc.set_focus_to(control) {
            self.dom.invalidate();
        }
        self.sync_focus();
        Ok(())
    }
}
/// A cloneable handle to one native document. No window, GPU or implicit executor
/// is created. Drive App::pump/flush explicitly; shell integration is separate.
#[derive(Clone)]
pub struct Renderer(Rc<RefCell<State>>);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub nodes: usize,
    pub engine_nodes: usize,
    pub interests: usize,
    pub layouts: u64,
    pub applied: u64,
    pub property_writes: u64,
    pub visual_revision: u64,
}
impl Renderer {
    pub fn profile(&self) -> Option<Profile> {
        self.0.borrow().dom.profile.clone()
    }
    pub fn new(runtime: RuntimeId) -> Self {
        Self::configured(runtime, false)
    }
    pub fn with_config(runtime: RuntimeId, config: Config) -> Self {
        Self::configured_with(runtime, false, config)
    }
    fn configured(runtime: RuntimeId, interactive: bool) -> Self {
        Self::configured_with(runtime, interactive, Config::default())
    }
    fn configured_with(runtime: RuntimeId, interactive: bool, config: Config) -> Self {
        Self(Rc::new(RefCell::new(State {
            #[cfg(feature = "paint")]
            runtime,
            mapper: Mapper::new(runtime),
            dom: Dom::new(config),
            receipt: Receipt::default(),
            interests: BTreeMap::new(),
            scroll_requests: BTreeMap::new(),
            inputs: Vec::new(),
            observed_revision: 0,
            observation_pending: true,
            interactive,
            focused: None,
            subscriptions: BTreeSet::new(),
            input_error: None,
            space: None,
        })))
    }
    /// Physical viewport and DPI. A zero-sized minimized surface is permitted.
    pub fn set_viewport(&self, width: u32, height: u32, scale: f32) -> Result<(), Error> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err(Error::InvalidProps("invalid Blitz scale"));
        }
        let mut state = self.0.borrow_mut();
        let viewport = Viewport::new(width, height, scale, Default::default());
        let (logical_width, logical_height) = viewport.logical_size();
        if !logical_width.is_finite() || !logical_height.is_finite() {
            return Err(Error::InvalidProps("Blitz logical viewport overflow"));
        }
        if state.dom.doc.viewport() != &viewport {
            state.dom.doc.set_viewport(viewport);
            state.dom.invalidate();
        }
        Ok(())
    }
    /// Replace one author stylesheet. The engine owns parsing/cascade/layout;
    /// external loads require an explicitly configured resource provider.
    pub fn set_css(&self, css: &str) {
        let mut state = self.0.borrow_mut();
        let node = state.dom.css;
        if state
            .dom
            .doc
            .get_node(node)
            .is_some_and(|node| node.text_content() == css)
        {
            return;
        }
        state.dom.doc.mutate().set_node_text(node, css);
        state.dom.invalidate();
    }
    #[cfg(feature = "css")]
    pub fn set_sheet(
        &self,
        sheet: &glory_html::Sheet,
    ) -> Result<(), glory_render::style::sheet::Error> {
        let css = glory_html::sheet(sheet, STYLE_PROFILE)?;
        self.set_css(&css);
        Ok(())
    }
    pub fn stats(&self) -> Stats {
        let state = self.0.borrow();
        Stats {
            nodes: state.dom.ranges.len(),
            engine_nodes: state.dom.doc.tree().len(),
            interests: state.subscriptions.len(),
            layouts: state.dom.layouts,
            applied: state.receipt.applied,
            property_writes: state.dom.property_writes,
            visual_revision: state.dom.revision,
        }
    }
    /// Cache reuse bytes exclude resources still owned by live elements and
    /// backend surfaces. Inspect both those lifetimes and these bounded tables.
    pub fn resource_stats(&self) -> blitz_dom::ResourceStats {
        self.0.borrow().dom.doc.resource_stats()
    }
    /// Register/acknowledge a host turn. Completions wake once until the next
    /// observation; synchronous pumping remains explicit for headless callers.
    pub fn observe_resources(&self, waker: &std::task::Waker) {
        self.0.borrow().dom.resource_wake.observe(waker);
    }
    /// Ingest completed engine resources without running business callbacks or
    /// committing staged UI mutations. Headless hosts may also use App::pump.
    pub fn poll_resources(&self) {
        let mut state = self.0.borrow_mut();
        if state.dom.resources() {
            state.dom.invalidate();
        }
    }
    /// Read-only engine inspection. Keep engine identities outside core/UI state.
    pub fn inspect<T>(&self, f: impl FnOnce(&BaseDocument) -> T) -> T {
        f(&self.0.borrow().dom.doc)
    }
    pub fn engine_id(&self, id: NodeId, part: Part) -> Result<EngineId, Error> {
        self.0.borrow().dom.range(id)?.part(part)
    }
    /// Request fresh layout/paint after an external engine resource or host
    /// invalidation. Does not poll tasks or enter application callbacks.
    pub fn invalidate(&self) {
        self.0.borrow_mut().dom.invalidate();
    }
    /// Advance the document's monotonic animation clock (seconds). Headless
    /// consumers may drive it deterministically; shell uses its window clock.
    /// Once attached to a shell, that host exclusively drives this clock.
    /// Static documents do not acquire a visual invalidation from time alone.
    pub fn advance_time(&self, seconds: f64) -> Result<(), Error> {
        let mut state = self.0.borrow_mut();
        if !seconds.is_finite() || seconds < state.dom.time {
            return Err(Error::InvalidProps("invalid Blitz animation time"));
        }
        if seconds > state.dom.time {
            state.dom.time = seconds;
            if state.dom.doc.is_animating() {
                state.dom.invalidate();
            }
        }
        Ok(())
    }
}
impl Render for Renderer {
    fn preflight(
        &self,
        node: &glory_render::Descriptor,
        patch: Option<&glory_render::Patch>,
    ) -> Result<(), Error> {
        match patch {
            None => check_style(&node.style)?,
            Some(glory_render::Patch::Style(glory_render::Change::Set(style))) => {
                check_style(style)?
            }
            _ => {}
        }
        glory_html::preflight(node, patch)
    }
    fn preflight_place(
        &self,
        tree: &glory_render::memory::Memory,
        child: glory_render::memory::Subtree<'_>,
        parent: Option<glory_render::NodeId>,
    ) -> Result<(), Error> {
        glory_html::preflight_place(tree, child, parent)
    }
    fn needs_observe(&self) -> bool {
        let state = self.0.borrow();
        state.observation_pending
            || state.dom.dirty
            || state.observed_revision != state.dom.revision
            || !state.inputs.is_empty()
            || state.dom.doc.resource_stats().completions != 0
    }
    fn set_profile(&mut self, profile: Option<Profile>) -> Option<Profile> {
        let engine = profile
            .as_ref()
            .map(|_| blitz_dom::profile::Profile::default());
        let mut state = self.0.borrow_mut();
        state.dom.doc.set_profile(engine);
        std::mem::replace(&mut state.dom.profile, profile)
    }
    fn style_profile(&self) -> glory_render::style::Profile {
        STYLE_PROFILE
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            measure: true,
            scroll: true,
            focus: self.0.borrow().interactive,
            input: self.0.borrow().interactive,
        }
    }
    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, Error> {
        let mut state = self.0.borrow_mut();
        let State { mapper, dom, .. } = &mut *state;
        // Check at each operation, not ahead of the entire batch: direct users
        // must retain an accepted prefix and its query watermark on rejection.
        // Only new/changed styles are examined; unrelated patches keep the
        // field-only HTML lowering path and never inspect a large value payload.
        let result = (|| {
            for mutation in mutations {
                match mutation {
                    Mutation::Create { node, .. } => check_style(&node.style)?,
                    Mutation::Patch {
                        id,
                        patch: glory_render::Patch::Style(glory_render::Change::Set(style)),
                    } => {
                        mapper.tree().node(*id)?;
                        check_style(style)?;
                    }
                    _ => {}
                }
                mapper.apply(std::slice::from_ref(mutation), dom)?;
            }
            Ok(mapper.receipt())
        })();
        state.receipt = state.mapper.receipt();
        state.dom.doc.prune_resource_interests();
        let State {
            mapper,
            scroll_requests,
            ..
        } = &mut *state;
        scroll_requests.retain(|id, _| mapper.tree().active(*id));
        let State {
            mapper,
            interests,
            subscriptions,
            ..
        } = &mut *state;
        interests.retain(|(id, _), _| mapper.tree().node(*id).is_ok());
        subscriptions.retain(|(id, _)| mapper.tree().node(*id).is_ok());
        state.sync_focus();
        result
    }
    fn listen(&mut self, node: NodeId, kind: EventKind, enabled: bool) -> Result<(), Error> {
        let mut state = self.0.borrow_mut();
        let descriptor = &state.mapper.tree().node(node)?.descriptor;
        if enabled {
            kind.validate_target(descriptor)?;
        }
        if enabled && matches!(kind, EventKind::Scroll | EventKind::Resize) {
            state.dom.main(node)?;
        }
        if enabled {
            state.subscriptions.insert((node, kind));
        } else {
            state.subscriptions.remove(&(node, kind));
        }
        if matches!(kind, EventKind::Scroll | EventKind::Resize) {
            if enabled {
                state.interests.entry((node, kind)).or_insert(None);
                state.observation_pending = true;
            } else {
                state.interests.remove(&(node, kind));
            }
        }
        Ok(())
    }
    fn action(&mut self, action: glory_render::Action) -> Result<(), Error> {
        let mut state = self.0.borrow_mut();
        action.validate_target(&state.mapper.tree().node(action.node())?.descriptor)?;
        if !state.mapper.tree().active(action.node()) {
            return Err(Error::Unsupported("action on disconnected Blitz range"));
        }
        match action {
            glory_render::Action::Focus(id) => state.focus(id),
            glory_render::Action::ScrollTo { node, x, y } => {
                let semantic = node;
                let node = state.dom.main(node)?;
                state.dom.resolve();
                let before = *state
                    .dom
                    .doc
                    .get_node(node)
                    .ok_or_else(|| invariant("missing scroller"))?
                    .scroll_offset();
                state.dom.doc.scroll_to(node, x, y, ScrollBehavior::Instant);
                let after = *state
                    .dom
                    .doc
                    .get_node(node)
                    .ok_or_else(|| invariant("missing scroller"))?
                    .scroll_offset();
                state.scroll_requests.insert(
                    semantic,
                    glory_render::ScrollAck {
                        requested: glory_render::ScrollRequest { x, y },
                        x: after.x,
                        y: after.y,
                    },
                );
                // Even a clamped/no-op assignment has a new acknowledgement.
                state.observation_pending = true;
                if before != after {
                    // scroll_to updates the offset and the engine's geometry
                    // generation immediately. Reuse boxes/text unless moving
                    // the content under the pointer changes CSS hover state.
                    let hovered = {
                        let _span = state.dom.profile.as_ref().map(|p| p.span(Phase::Hover));
                        state.dom.doc.refresh_hover()
                    };
                    if hovered {
                        state.dom.invalidate();
                    } else {
                        state.dom.invalidate_pixels();
                    }
                }
                Ok(())
            }
        }
    }
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        let mut state = self.0.borrow_mut();
        state.mapper.tree().node(query.node())?;
        let result = (|| {
            query.validate_target(&state.mapper.tree().node(query.node())?.descriptor)?;
            if !state.mapper.tree().active(query.node()) {
                return Err(Error::Unsupported("query on disconnected Blitz range"));
            }
            let node = state.dom.main(query.node())?;
            state.dom.resolve();
            match query {
                Query::Focus(id) if state.interactive => {
                    state.sync_focus();
                    let control = state.dom.control(id)?;
                    Ok(QueryValue::Focus(
                        state.dom.doc.get_focussed_node_id() == Some(control),
                    ))
                }
                Query::Focus(_) => Err(Error::Unsupported("headless Blitz focus")),
                Query::Bounds(_) => {
                    let rect = state
                        .dom
                        .doc
                        .get_client_bounding_rect(node)
                        .ok_or(Error::Unsupported("Blitz bounds on this node"))?;
                    // beta.4 physical_unrounded_geometry subtracts this node's
                    // own scroll as well as its ancestors'. A scroller's border
                    // box stays put while only its content moves. Compensate at
                    // this pinned-version boundary; do not change descendant
                    // offsets or add the viewport scroll back a second time.
                    let own_scroll = *state
                        .dom
                        .doc
                        .get_node(node)
                        .ok_or(Error::NodeGone(query.node()))?
                        .scroll_offset();
                    Ok(QueryValue::Bounds(Rect {
                        x: rect.x + own_scroll.x,
                        y: rect.y + own_scroll.y,
                        width: rect.width,
                        height: rect.height,
                    }))
                }
                Query::Scroll(_) => {
                    let offset = state
                        .dom
                        .doc
                        .get_node(node)
                        .ok_or(Error::NodeGone(query.node()))?
                        .scroll_offset();
                    Ok(QueryValue::Scroll {
                        x: offset.x,
                        y: offset.y,
                    })
                }
                Query::Selection(id) if state.interactive => state.dom.selection(id),
                Query::Selection(_) => Err(Error::Unsupported("headless Blitz selection")),
            }
        })();
        let applied = state.receipt.applied;
        state.inputs.push(Input::Response {
            token,
            applied,
            result,
        });
        Ok(())
    }
    fn drain(&mut self, inputs: &mut Vec<Input>) {
        let mut state = self.0.borrow_mut();
        state.dom.resolve();
        state.sync_focus();
        let State {
            mapper,
            dom,
            interests,
            scroll_requests,
            inputs: queued,
            ..
        } = &mut *state;
        for ((id, kind), previous) in interests.iter_mut() {
            if !mapper.tree().active(*id) {
                continue;
            }
            let Ok((size, viewport)) = dom.geometry(*id) else {
                continue;
            };
            let event = if *kind == EventKind::Resize {
                Event::Resize { size, viewport }
            } else {
                let Ok(node) = dom.main(*id) else { continue };
                let Some(node) = dom.doc.get_node(node) else {
                    continue;
                };
                let offset = node.scroll_offset();
                Event::Scroll {
                    x: offset.x,
                    y: offset.y,
                    viewport,
                    ack: scroll_requests.get(id).copied(),
                }
            };
            if previous.as_ref() != Some(&event) {
                *previous = Some(event.clone());
                queued.push(Input::Event { node: *id, event });
            }
        }
        inputs.append(queued);
        state.observed_revision = state.dom.revision;
        state.observation_pending = false;
    }
}

#[cfg(test)]
mod scroll_invalidation_tests {
    use super::*;
    use glory_render::Action;
    use glory_render::style::{Length, Overflow, Style};
    use glory_ui::{App, text, view};

    fn fixture() -> (App, Renderer, NodeId, NodeId) {
        let mut renderer = None;
        let app = App::new(|id| {
            let value = Renderer::new(id);
            renderer = Some(value.clone());
            value
        });
        let renderer = renderer.unwrap();
        renderer.set_viewport(320, 240, 1.).unwrap();
        app.mount(
            view()
                .class("scroller")
                .style(
                    Style::new()
                        .width(Length::Px(160.))
                        .height(Length::Px(80.))
                        .overflow_y(Overflow::Auto),
                )
                .fill((
                    view()
                        .class("first")
                        .style(Style::new().height(Length::Px(100.)))
                        .fill(text().content("first")),
                    view()
                        .class("second")
                        .style(Style::new().height(Length::Px(100.)))
                        .fill(text().content("second")),
                )),
        )
        .unwrap();
        app.settle(8).unwrap();
        let (root, second) = {
            let tree = app.tree();
            let root = tree.roots()[0];
            (root, tree.node(root).unwrap().children[1])
        };
        (app, renderer, root, second)
    }

    fn rect(renderer: &Renderer, node: NodeId) -> Rect {
        let engine = renderer.engine_id(node, Part::Main).unwrap();
        let value = renderer.inspect(|doc| doc.get_client_bounding_rect(engine).unwrap());
        Rect {
            x: value.x,
            y: value.y,
            width: value.width,
            height: value.height,
        }
    }

    #[test]
    fn scroll_reuses_boxes_updates_geometry_and_invalidates_pixels_without_losing_ack() {
        let (_app, mut renderer, root, second) = fixture();
        renderer.listen(root, EventKind::Scroll, true).unwrap();
        renderer.drain(&mut Vec::new());
        let profile = Profile::native();
        renderer.set_profile(Some(profile.clone()));
        let before = renderer.stats();
        let old = rect(&renderer, second);
        renderer
            .action(Action::ScrollTo {
                node: root,
                x: 0.,
                y: 40.,
            })
            .unwrap();
        let mut inputs = Vec::new();
        renderer.drain(&mut inputs);
        assert_eq!(renderer.stats().layouts, before.layouts);
        assert!(renderer.stats().visual_revision > before.visual_revision);
        assert!((rect(&renderer, second).y - (old.y - 40.)).abs() < 0.01);
        assert_eq!(profile.snapshot().sample(Phase::Style).calls, 0);
        assert_eq!(profile.snapshot().sample(Phase::TextShape).calls, 0);
        assert!(inputs.iter().any(|input| matches!(input, Input::Event { node,event:Event::Scroll { y:40.,ack:Some(ack),.. } } if *node==root && ack.requested.y==40. && ack.y==40.)));
        let revision = renderer.stats().visual_revision;
        renderer
            .action(Action::ScrollTo {
                node: root,
                x: 0.,
                y: 40.,
            })
            .unwrap();
        renderer.drain(&mut Vec::new());
        assert_eq!(renderer.stats().visual_revision, revision);
        renderer.set_css(".second { padding-top: 5px }");
        renderer.drain(&mut Vec::new());
        assert!(renderer.stats().layouts > before.layouts);
    }

    #[test]
    fn scrolling_a_new_hover_target_under_the_pointer_retains_css_layout_invalidation() {
        use blitz_dom::Document;
        use blitz_traits::events::{
            BlitzPointerEvent, BlitzPointerId, MouseEventButton, MouseEventButtons, PointerCoords,
            UiEvent,
        };
        let (_app, mut renderer, root, second) = fixture();
        renderer.set_css(".second:hover { padding-top: 11px }");
        renderer.drain(&mut Vec::new());
        let bounds = rect(&renderer, root);
        let x = (bounds.x + 10.) as f32;
        let y = (bounds.y + 20.) as f32;
        {
            let mut state = renderer.0.borrow_mut();
            state
                .dom
                .doc
                .handle_ui_event(UiEvent::PointerMove(BlitzPointerEvent {
                    id: BlitzPointerId::Mouse,
                    is_primary: true,
                    coords: PointerCoords {
                        page_x: x,
                        page_y: y,
                        client_x: x,
                        client_y: y,
                        screen_x: x,
                        screen_y: y,
                    },
                    button: MouseEventButton::Main,
                    buttons: MouseEventButtons::empty(),
                    mods: Default::default(),
                    details: Default::default(),
                    element: Default::default(),
                    active_pointers: Default::default(),
                }));
            state.dom.invalidate();
        }
        renderer.drain(&mut Vec::new());
        let before = renderer.stats().layouts;
        renderer
            .action(Action::ScrollTo {
                node: root,
                x: 0.,
                y: 90.,
            })
            .unwrap();
        renderer.drain(&mut Vec::new());
        assert!(renderer.stats().layouts > before);
        let engine = renderer.engine_id(second, Part::Main).unwrap();
        renderer.inspect(|doc| {
            assert!(doc.get_node(engine).unwrap().final_layout().padding.top >= 11.)
        });
    }
}
