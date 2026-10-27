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
#[cfg(feature = "clipboard")]
#[path = "clipboard_io.rs"]
mod clipboard_io;
#[cfg(feature = "files")]
#[path = "files_io.rs"]
mod files_io;
#[path = "image_observation.rs"]
mod image_observation;
#[path = "resource_wake.rs"]
mod resource_wake;
use image_observation::ImageObserver;
#[path = "choice.rs"]
mod choice;
#[path = "focus_scope.rs"]
mod focus_scope;
#[path = "graphic.rs"]
mod graphic;
#[cfg(feature = "paint")]
#[path = "graphic_motion.rs"]
mod graphic_motion;
#[path = "layers.rs"]
mod layers;
#[path = "native_select.rs"]
mod native_select;
#[path = "scroll_into_view.rs"]
mod scroll_into_view;
#[cfg(feature = "shell")]
#[path = "swipe.rs"]
mod swipe;
#[path = "translation.rs"]
mod translation;
use style::computed_values::visibility::T as Visibility;

/// Typed inline acceptance for the pinned native engine. HTML-family CSS still
/// goes to Stylo, but parsing a declaration does not establish its layout
/// behaviour: beta.4 does not implement sticky offsets or automatic scroll
/// anchoring. Explicitly disabling anchoring (used by virtual lists) is valid.
/// Intrinsic editor sizing and text-overflow rendering are also unavailable.
/// Re-audit these exclusions against real geometry/paint before widening them.
pub const STYLE_PROFILE: glory_render::style::Profile = glory_render::style::Profile {
    name: "blitz",
    version: 11,
    inline: glory_render::style::Support::ALL
        .without_value(glory_render::style::Value::Sticky)
        .without_value(glory_render::style::Value::AnchorAuto)
        .without_property(glory_render::style::Property::FieldSizing)
        .without_property(glory_render::style::Property::TextOverflow),
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

// The pinned organization fork provides real viewport layout/paint/hit roots.
// Widen only the Layer kinds whose focus/input driver is implemented here.
fn check_layer<'a>(
    node: impl Into<glory_render::DescriptorView<'a>>,
    patch: Option<&glory_render::Patch>,
) -> Result<(), Error> {
    let node = node.into();
    use glory_render::{Change, Patch};
    let value = match patch {
        None => node.semantics.as_deref(),
        Some(Patch::Semantics(Change::Set(value))) => value.as_deref(),
        Some(Patch::Semantics(Change::Clear)) => None,
        _ => node.semantics.as_deref(),
    };
    if value.is_some_and(|s| s.sheet_grip) {
        return Err(Error::Unsupported("native sheet gestures"));
    }
    if value.is_some_and(|s| s.collapse.is_some()) {
        return Err(Error::Unsupported(
            "native intrinsic-height collapse motion",
        ));
    }
    if let Some(layer) = value.and_then(|s| s.layer.as_ref()) {
        let role = match patch {
            Some(Patch::Role(Change::Set(role))) => *role,
            Some(Patch::Role(Change::Clear)) => None,
            _ => node.role,
        };
        let style = match patch {
            Some(Patch::Style(Change::Set(style))) => style,
            Some(Patch::Style(Change::Clear)) => &glory_render::style::Style::default(),
            _ => node.style,
        };
        if !matches!(
            role,
            Some(
                glory_render::Role::Dialog
                    | glory_render::Role::AlertDialog
                    | glory_render::Role::Listbox
                    | glory_render::Role::Tree
                    | glory_render::Role::Menu
            )
        ) || layer.sheet.is_some()
            || !matches!(
                layer.placement,
                glory_render::LayerPlacement::Center
                    | glory_render::LayerPlacement::Anchor(_)
                    | glory_render::LayerPlacement::Viewport(_)
                    | glory_render::LayerPlacement::ContextPoint(_)
            )
            || value.is_some_and(|s| s.translation.is_some())
            || style
                .position
                .is_some_and(|position| position != glory_render::style::Position::Fixed)
        {
            return Err(Error::Unsupported("native top layers"));
        }
    }
    if value.is_some_and(|s| s.link_open == Some(glory_render::LinkOpen::NewContext)) {
        return Err(Error::Unsupported("native separate link contexts"));
    }
    Ok(())
}

/// Keep native control admission in sync with actual engine and input support.
fn check_mode<'a>(
    node: impl Into<glory_render::DescriptorView<'a>>,
    patch: Option<&glory_render::Patch>,
    files_enabled: bool,
) -> Result<(), Error> {
    let node = node.into();
    use glory_render::Props;
    if matches!(node.props, Props::RichEditor(_)) {
        return Err(Error::Unsupported("native rich editing"));
    }
    #[cfg(feature = "files")]
    files_io::check(node, patch, files_enabled)?;
    #[cfg(not(feature = "files"))]
    {
        let _ = (patch, files_enabled);
        if matches!(node.props, Props::File(_)) {
            return Err(Error::Unsupported("native file picker controls"));
        }
    }
    Ok(())
}

/// Native document configuration. No network/filesystem access is installed by
/// default. A provider must queue work and honor cancel/close without UI reentry.
#[derive(Clone)]
pub struct Config {
    #[cfg(feature = "files")]
    pub files: Option<crate::files::Files>,
    #[cfg(feature = "clipboard")]
    pub clipboard: Option<crate::clipboard::Clipboard>,
    pub motion: glory_render::MotionPreference,
    pub base_url: blitz_traits::net::Url,
    pub resources: blitz_dom::ResourceLimits,
    pub provider: Option<Arc<dyn blitz_traits::net::NetProvider>>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            #[cfg(feature = "files")]
            files: None,
            #[cfg(feature = "clipboard")]
            clipboard: None,
            motion: glory_render::MotionPreference::Reduce,
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
    any(
        target_os = "windows",
        target_os = "linux",
        target_os = "macos",
        target_os = "android",
        target_os = "ios"
    )
))]
#[path = "shell.rs"]
pub mod shell;

fn tag(name: &str) -> QualName {
    QualName::new(
        None,
        blitz_dom::Namespace::from(glory_html::element_namespace(name)),
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
    select: Option<native_select::Control>,
    image: Option<ImageObserver>,
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
    #[cfg(feature = "shell")]
    input_before: Option<(
        u64,
        glory_render::TextSelection,
        glory_render::InputEditIntent,
    )>,
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
    open_select: Option<NodeId>,
    translations: translation::Translations,
    #[cfg(feature = "paint")]
    motion: graphic_motion::Motion,
    doc: BaseDocument,
    body: EngineId,
    css: EngineId,
    ranges: HashMap<NodeId, Range>,
    owners: HashMap<EngineId, NodeId>,
    controls: HashMap<EngineId, NodeId>,
    image_ready: BTreeMap<u128, NodeId>,
    image_sequence: u128,
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
        let (body, css, translation_sheet) = {
            let mut m = doc.mutate();
            let html = m.create_element(tag("html"), Vec::new());
            let head = m.create_element(tag("head"), Vec::new());
            let body = m.create_element(tag("body"), Vec::new());
            let base = m.create_element(tag("style"), Vec::new());
            let baseline = m.create_text_node(&format!(
                "html,body{{margin:0;padding:0}}{}[data-gly-layer=center]{{left:var(--gly-layer-x,0px);top:var(--gly-layer-y,0px);transform:none}}[data-gly-rotate]{{animation:none!important;transform:none!important}}[data-gly-path-transition]{{transition:none!important}}",
                format_args!("{}{}", glory_html::BASE_CSS, native_select::CSS)
            ));
            let sheet = m.create_element(tag("style"), Vec::new());
            let css = m.create_text_node("");
            let translations = m.create_element(tag("style"), Vec::new());
            let translation_sheet = m.create_text_node("");
            m.append_children(root, &[html]);
            m.append_children(html, &[head, body]);
            m.append_children(head, &[base, sheet, translations]);
            m.append_children(base, &[baseline]);
            m.append_children(sheet, &[css]);
            m.append_children(translations, &[translation_sheet]);
            (body, css, translation_sheet)
        };
        Self {
            translations: translation::Translations::new(translation_sheet, config.motion),
            #[cfg(feature = "paint")]
            motion: graphic_motion::Motion::new(config.motion),
            open_select: None,
            doc,
            body,
            css,
            ranges: HashMap::new(),
            owners: HashMap::new(),
            controls: HashMap::new(),
            image_ready: BTreeMap::new(),
            image_sequence: 0,
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
        let image = shape
            .pieces
            .iter()
            .any(|piece| matches!(&piece.kind, Kind::Element { tag: "img", .. }))
            .then(|| ImageObserver::new(shape.revision));
        let physical_root = if shape.pieces.is_empty() {
            None
        } else {
            Some(shape.root_part()?)
        };
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
            let root = parts
                .get(&physical_root.expect("non-group physical root"))
                .copied()
                .ok_or_else(|| invariant("no physical root part"))?;
            (root, root, None)
        };
        drop(m);
        let select = shape
            .select_options
            .map(|options| native_select::Control::create(&mut self.doc, first, options));
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
                select,
                image,
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
        if let ("selectedKeys", Property::SelectedKeys(keys)) = (name, value) {
            return native_select::reflect(self, id, keys);
        }
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
            // Live properties stay independent from default HTML attributes.
            ("indeterminate", Property::Bool(value)) => {
                // Radio has no mixed presentation. The shared shape resets the
                // property to false when switching out of checkbox modes.
                if !*value
                    && m.doc.get_node(node).is_some_and(|n| {
                        n.attr(blitz_dom::LocalName::from("type")) == Some("radio")
                    })
                {
                    return Ok(());
                }
                if !m.set_input_indeterminate(node, *value) {
                    return Err(invariant("indeterminate requires a native checkbox"));
                }
            }
            ("checked", Property::Bool(value)) => {
                if !m.set_input_checked(node, *value) {
                    return Err(invariant("checked requires a native check control"));
                }
            }
            ("disabled", Property::Bool(true)) => m.set_attribute(node, attr(name), ""),
            ("disabled", Property::Bool(false)) => m.clear_attribute(node, attr(name)),
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
            let too_long = self
                .doc
                .get_node(control)
                .and_then(|n| n.element_data())
                .is_some_and(|element| {
                    element
                        .attr(attr("data-gly-input-edit").local)
                        .and_then(|v| v.parse::<usize>().ok())
                        .is_some_and(|max| {
                            element
                                .text_input_data()
                                .is_some_and(|i| i.editor.raw_text().len() > max)
                        })
                });
            Some((
                if too_long {
                    String::new()
                } else {
                    self.text_value(id)?
                },
                revision,
            ))
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
            self.sync_image(owner)?;
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
            #[cfg(feature = "paint")]
            self.motion.remove(child);
            if let Some(range) = self.ranges.remove(&child) {
                if let Some(ready) = range.image.and_then(|image| image.ready) {
                    self.image_ready.remove(&ready);
                }
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
            self.layout();
        }
        if native_select::position(self) {
            self.layout();
        }
        #[cfg(feature = "paint")]
        self.refresh_graphic_motion();
    }
    // Each actual engine pass owns the same phase collectors, including a
    // second pass needed to position the current intrinsic Select popup.
    fn layout(&mut self) {
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
                self.sync_image(id)?;
            }
        }
        self.invalidate();
        Ok(())
    }
}

#[cfg(feature = "shell")]
struct DragSession {
    from: NodeId,
    pointer: blitz_traits::events::BlitzPointerId,
    x: f64,
    y: f64,
    started: bool,
    captured: bool,
}
struct State {
    #[cfg(feature = "files")]
    files: files_io::Io,
    #[cfg(feature = "clipboard")]
    clipboard: clipboard_io::Io,
    #[cfg(feature = "shell")]
    keyboard_clock: std::time::Instant,
    // One AppKit command awaiting its immediately following physical key.
    #[cfg(feature = "shell")]
    apple_input: Option<(NodeId, shell::AppleInput)>,
    layers: layers::Layers,
    focus_scopes: focus_scope::Scopes,
    color_scheme: Option<bool>,
    #[cfg(feature = "shell")]
    drag: Option<DragSession>,
    #[cfg(feature = "shell")]
    swipe: Option<swipe::Session>,
    #[cfg(feature = "shell")]
    suppress_click: bool,
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
impl Drop for State {
    fn drop(&mut self) {
        #[cfg(feature = "files")]
        self.files.close();
        self.dom.resource_wake.close();
    }
}
impl State {
    #[cfg(feature = "shell")]
    fn captured_text_value(&self, node: NodeId) -> Result<String, Error> {
        if let Some(max) = self
            .mapper
            .tree()
            .node(node)?
            .descriptor
            .view()
            .semantics
            .as_ref()
            .and_then(|s| s.input_edit)
            .map(|c| c.max_bytes)
        {
            let control = self.dom.control(node)?;
            if self
                .dom
                .doc
                .get_node(control)
                .and_then(|n| n.element_data())
                .and_then(|e| e.text_input_data())
                .is_some_and(|i| i.editor.raw_text().len() > max)
            {
                return Ok(String::new());
            }
        }
        self.dom.text_value(node)
    }
    fn terminal_input_event(
        &self,
        node: NodeId,
        value: String,
        revision: u64,
    ) -> Result<Event, Error> {
        let Some(capture) = self.mapper.tree().node(node).ok().and_then(|n| {
            n.descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|s| s.input_edit)
        }) else {
            return Ok(Event::ValueChanged {
                value,
                revision,
                composing: false,
            });
        };
        let reject =
            |reason| Event::InputEdit(glory_render::InputEdit::Rejected { revision, reason });
        if self.dom.range(node)?.editing.revision != revision {
            return Ok(reject(
                glory_render::InputEditRejection::SelectionUnavailable,
            ));
        }
        let control = self.dom.control(node)?;
        let Some(input) = self
            .dom
            .doc
            .get_node(control)
            .and_then(|n| n.element_data())
            .and_then(|e| e.text_input_data())
        else {
            return Ok(reject(
                glory_render::InputEditRejection::SelectionUnavailable,
            ));
        };
        let text = input.editor.raw_text();
        if text.len() > capture.max_bytes {
            return Ok(reject(glory_render::InputEditRejection::TooLong));
        }
        let selected = input.editor.raw_selection();
        let a = selected.anchor().index();
        let f = selected.focus().index();
        let start = text
            .get(..a.min(f))
            .ok_or(Error::InvalidEvent("invalid native selection"))?
            .encode_utf16()
            .count();
        let end = text
            .get(..a.max(f))
            .ok_or(Error::InvalidEvent("invalid native selection"))?
            .encode_utf16()
            .count();
        Ok(Event::InputEdit(glory_render::InputEdit::Value {
            value,
            revision,
            composing: false,
            before: None,
            intent: glory_render::InputEditIntent::Replace,
            selection: glory_render::TextSelection {
                start,
                end,
                direction: if a > f {
                    glory_render::SelectionDirection::Backward
                } else if a < f {
                    glory_render::SelectionDirection::Forward
                } else {
                    glory_render::SelectionDirection::None
                },
            },
        }))
    }
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
            let event = self.terminal_input_event(node, value, revision)?;
            self.inputs.push(Input::Event { node, event });
        }
        Ok(())
    }
    fn focus_changed(&mut self, next: Option<NodeId>) {
        if self.focused == next {
            return;
        }
        self.space = None;
        #[cfg(all(feature = "clipboard", feature = "shell"))]
        clipboard_io::editor::cancel(self);
        if self.dom.open_select != next {
            native_select::close(&mut self.dom);
        }
        #[cfg(feature = "shell")]
        {
            self.apple_input = None;
        }
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
        layers::repair_focus(self);
        focus_scope::repair(self);
        let physical = self.dom.doc.get_focussed_node_id();
        let next = physical.and_then(|node| self.dom.owner(node));
        if let Some(next) = next
            && (!self.mapper.tree().active(next)
                || physical.is_some_and(|node| !self.dom.focus_visible(node))
                || !self.mapper.tree().enabled(next))
        {
            self.dom.doc.clear_focus();
            self.dom.invalidate();
            self.focus_changed(None);
        } else {
            #[cfg(feature = "shell")]
            if self.focused != next {
                layers::outside_focus(self, next);
            }
            self.focus_changed(next);
        }
    }
    fn focus(&mut self, id: NodeId) -> Result<(), Error> {
        if !self.interactive {
            return Err(Error::Unsupported("headless Blitz focus"));
        }
        let descriptor = self.mapper.tree().node(id)?.descriptor.view();
        if !self.mapper.tree().enabled(id)
            || !self.layers.allowed(self.mapper.tree(), id)
            || !focus_scope::allowed(self, id)
        {
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
        #[cfg(feature = "files")]
        let (config, files) = {
            let mut config = config;
            let files = files_io::Io::new(config.files.clone());
            files.install_provider(&mut config);
            (config, files)
        };
        Self(Rc::new(RefCell::new(State {
            #[cfg(feature = "files")]
            files,
            #[cfg(feature = "clipboard")]
            clipboard: clipboard_io::Io::new(config.clipboard.clone()),
            #[cfg(feature = "shell")]
            keyboard_clock: std::time::Instant::now(),
            #[cfg(feature = "shell")]
            apple_input: None,
            layers: layers::Layers::default(),
            focus_scopes: focus_scope::Scopes::default(),
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
            #[cfg(feature = "shell")]
            drag: None,
            #[cfg(feature = "shell")]
            swipe: None,
            color_scheme: None,
            #[cfg(feature = "shell")]
            suppress_click: false,
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
            #[cfg(feature = "shell")]
            swipe::cancel(&mut state);
            state.dom.doc.set_viewport(viewport);
            state.dom.invalidate();
            let State { mapper, dom, .. } = &mut *state;
            dom.sync_translations(mapper.tree(), &[])?;
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
            let was_animating = state.dom.doc.is_animating();
            state.dom.time = seconds;
            #[cfg(feature = "paint")]
            state.dom.motion.clock.set(seconds);
            #[cfg(feature = "paint")]
            let transition_finished = state.dom.motion.finish_transitions();
            #[cfg(not(feature = "paint"))]
            let transition_finished = false;
            if was_animating || transition_finished || state.dom.doc.is_animating() {
                state.dom.invalidate();
                let wake = state.dom.resource_wake.clone();
                drop(state);
                wake.wake();
            }
        }
        Ok(())
    }
}
impl Render for Renderer {
    fn preflight(
        &self,
        node: glory_render::DescriptorView<'_>,
        patch: Option<&glory_render::Patch>,
    ) -> Result<(), Error> {
        check_layer(node, patch)?;
        let files_enabled = self.capabilities().files.is_some();
        check_mode(node, patch, files_enabled)?;
        graphic::check(node, patch)?;
        match patch {
            None => check_style(node.style)?,
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
            || state.dom.resource_wake.is_pending()
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
            clipboard_write: {
                #[cfg(feature = "clipboard")]
                {
                    self.0.borrow().clipboard.configured()
                }
                #[cfg(not(feature = "clipboard"))]
                {
                    false
                }
            },
            files: {
                #[cfg(feature = "files")]
                {
                    self.0.borrow().files.capabilities()
                }
                #[cfg(not(feature = "files"))]
                {
                    None
                }
            },
            measure: true,
            scroll: true,
            focus: self.0.borrow().interactive,
            input: self.0.borrow().interactive,
            collapse_completion: false,
        }
    }
    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, Error> {
        let mut state = self.0.borrow_mut();
        #[cfg(feature = "files")]
        let files_enabled = state.files.capabilities().is_some();
        #[cfg(not(feature = "files"))]
        let files_enabled = false;
        let State { mapper, dom, .. } = &mut *state;
        // Check at each operation, not ahead of the entire batch: direct users
        // must retain an accepted prefix and its query watermark on rejection.
        // Only new/changed styles are examined; unrelated patches keep the
        // field-only HTML lowering path and never inspect a large value payload.
        let result = (|| {
            for mutation in mutations {
                match mutation {
                    Mutation::CreatePlan { plan, .. } => {
                        for node in plan.nodes() {
                            check_layer(&node.descriptor, None)?;
                            check_mode(&node.descriptor, None, files_enabled)?;
                            graphic::check(node.descriptor.view(), None)?;
                            check_style(node.descriptor.view().style)?;
                        }
                    }
                    Mutation::Create { node, .. } => {
                        check_layer(node, None)?;
                        check_mode(node, None, files_enabled)?;
                        graphic::check(node.view(), None)?;
                    }
                    Mutation::Patch { id, patch } => {
                        let descriptor = mapper.tree().node(*id)?.descriptor.view();
                        check_layer(descriptor, Some(patch))?;
                        check_mode(descriptor, Some(patch), files_enabled)?;
                        graphic::check(descriptor, Some(patch))?;
                    }
                    _ => {}
                }
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
                #[cfg(feature = "paint")]
                let previous_graphic = if let Mutation::Patch {
                    id,
                    patch: glory_render::Patch::Graphic(glory_render::Change::Set(next)),
                } = mutation
                {
                    if next.transition.is_some() {
                        match mapper.tree().node(*id)?.descriptor.view().props {
                            glory_render::Props::Graphic(old) => Some(old.commands.clone()),
                            _ => None,
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
                mapper.apply(std::slice::from_ref(mutation), dom)?;
                #[cfg(feature = "paint")]
                match mutation {
                    Mutation::Create { id, .. }
                    | Mutation::Patch {
                        id,
                        patch: glory_render::Patch::Graphic(_),
                    } if !matches!(
                        mutation,
                        Mutation::Patch {
                            patch: glory_render::Patch::Graphic(glory_render::Change::Keep),
                            ..
                        }
                    ) =>
                    {
                        if let glory_render::Props::Graphic(graphic) =
                            mapper.tree().node(*id)?.descriptor.view().props
                        {
                            dom.install_graphic(*id, graphic, previous_graphic.as_deref())?;
                        }
                    }
                    Mutation::CreatePlan { ids, .. } => {
                        for id in ids {
                            if let glory_render::Props::Graphic(graphic) =
                                mapper.tree().node(*id)?.descriptor.view().props
                            {
                                dom.install_graphic(*id, graphic, None)?;
                            }
                        }
                    }
                    Mutation::Place { id, .. } | Mutation::Resume { id, .. } => {
                        // Engine detachment unloads custom widgets. Restore only
                        // missing widgets in this live moved subtree after attach;
                        // ordinary metadata/style patches never walk descendants.
                        let mut pending = vec![*id];
                        while let Some(id) = pending.pop() {
                            let node = mapper.tree().node(id)?;
                            pending.extend(&node.children);
                            if let glory_render::Props::Graphic(graphic) =
                                node.descriptor.view().props
                            {
                                let main = dom.main(id)?;
                                if dom.doc.get_node(main).is_some_and(|node| {
                                    node.element_data().is_some_and(|element| {
                                        element.custom_widget_data().is_none()
                                    })
                                }) {
                                    dom.install_graphic(id, graphic, None)?;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(mapper.receipt())
        })();
        state.receipt = state.mapper.receipt();
        #[cfg(feature = "shell")]
        swipe::update(&mut state);
        let translated = {
            let State { mapper, dom, .. } = &mut *state;
            dom.sync_translations(mapper.tree(), mutations)
        };
        if result.is_ok() {
            translated?;
        }
        #[cfg(feature = "paint")]
        {
            state.dom.motion.permissions(state.mapper.tree());
            state.dom.refresh_graphic_motion();
        }
        state.dom.doc.prune_resource_interests();
        let layered = layers::sync(&mut state, mutations);
        if result.is_ok() {
            layered?;
        }
        focus_scope::sync(&mut state);
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
        native_select::sync(&mut state);
        #[cfg(feature = "files")]
        let retired = {
            files_io::sync(&mut state);
            files_io::take_retired(&mut state)
        };
        drop(state);
        #[cfg(feature = "files")]
        drop(retired);
        result
    }
    fn listen(&mut self, node: NodeId, kind: EventKind, enabled: bool) -> Result<(), Error> {
        let mut state = self.0.borrow_mut();
        let descriptor = state.mapper.tree().node(node)?.descriptor.view();
        if enabled {
            kind.validate_target(descriptor)?;
        }
        if enabled && matches!(kind, EventKind::Scroll | EventKind::Resize) {
            state.dom.main(node)?;
        }
        if enabled {
            state.subscriptions.insert((node, kind));
            if kind == EventKind::ImageStatus {
                state.dom.queue_image_result(node);
                state.observation_pending = true;
            }
            if kind == EventKind::ColorSchemeChanged
                && let Some(dark) = state.color_scheme
            {
                state.inputs.push(Input::Event {
                    node,
                    event: Event::ColorSchemeChanged { dark },
                });
            }
        } else {
            state.subscriptions.remove(&(node, kind));
            #[cfg(feature = "shell")]
            if kind == EventKind::Swipe
                && state
                    .swipe
                    .as_ref()
                    .is_some_and(|current| current.node == node)
            {
                swipe::cancel(&mut state);
            }
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
        state.mapper.tree().validate_clipboard_action(action)?;
        if matches!(action, glory_render::Action::Focus(_))
            && !state.mapper.tree().enabled(action.node())
        {
            return Err(Error::Unsupported("focus in disabled fieldset"));
        }
        // ExitMenu runs after a controlled close has hidden the Menu. Its
        // driver separately verifies mounted ancestry and the current return
        // target; other actions still require an active presentation.
        if !state.mapper.tree().active(action.node())
            && !matches!(action, glory_render::Action::ExitMenu { .. })
        {
            return Err(Error::Unsupported("action on disconnected Blitz range"));
        }
        #[cfg(feature = "files")]
        if matches!(
            action,
            glory_render::Action::PickFiles { .. }
                | glory_render::Action::ClearFiles { .. }
                | glory_render::Action::RemoveFile(_)
        ) {
            let result = files_io::action(&mut state, action);
            let retired = files_io::take_retired(&mut state);
            drop(state);
            drop(retired);
            return result;
        }
        match action {
            glory_render::Action::ScrollIntoView(id) => scroll_into_view::reveal(&mut state, id),
            glory_render::Action::PickFiles { .. }
            | glory_render::Action::SelectRich { .. }
            | glory_render::Action::ClearFiles { .. }
            | glory_render::Action::RemoveFile(_) => Err(Error::Unsupported("native file access")),
            glory_render::Action::CopyText {
                node,
                revision,
                request,
            } => {
                #[cfg(feature = "clipboard")]
                {
                    clipboard_io::copy(&mut state, node, revision, request)
                }
                #[cfg(not(feature = "clipboard"))]
                {
                    let _ = (node, revision, request);
                    Err(Error::Unsupported("native text clipboard write"))
                }
            }
            glory_render::Action::RaiseLayer(id) => layers::raise(&mut state, id),
            glory_render::Action::SelectText {
                node,
                revision,
                selection,
            } => {
                if !state.interactive {
                    return Err(Error::Unsupported("headless Blitz text selection"));
                }
                let range = state.dom.range(node)?;
                if range.editing.revision != revision || range.editing.composing {
                    return Err(Error::Unsupported(
                        "text selection revision or composition changed",
                    ));
                }
                let control = state.dom.control(node)?;
                state.dom.resolve();
                let bytes = {
                    let input = state
                        .dom
                        .doc
                        .get_node(control)
                        .and_then(|n| n.element_data())
                        .and_then(|e| e.text_input_data())
                        .ok_or(Error::Unsupported("Blitz text editor unavailable"))?;
                    let text = input.editor.raw_text();
                    let glory_render::Props::Input(expected) =
                        state.mapper.tree().node(node)?.descriptor.view().props
                    else {
                        unreachable!()
                    };
                    if text != expected.value {
                        return Err(Error::Unsupported("text selection value changed"));
                    }
                    let bytes = selection.byte_range(text)?;
                    // Parley's cursor snaps to shaped cluster starts. Validate
                    // both endpoints against the actual layout before touching
                    // selection, so combining/ZWJ interiors cannot be rounded.
                    let layout = input
                        .editor
                        .try_layout()
                        .ok_or(Error::Unsupported("text selection layout unavailable"))?;
                    let mut start = bytes.start == 0 || bytes.start == text.len();
                    let mut end = bytes.end == 0 || bytes.end == text.len();
                    for cluster in layout
                        .lines()
                        .flat_map(|line| line.runs())
                        .flat_map(|run| run.clusters())
                    {
                        let index = cluster.text_range().start;
                        start |= index == bytes.start;
                        end |= index == bytes.end;
                        if start && end {
                            break;
                        }
                    }
                    if !start || !end {
                        return Err(Error::Unsupported(
                            "exact selection inside a shaped text cluster",
                        ));
                    }
                    bytes
                };
                let (anchor, focus) =
                    if selection.direction == glory_render::SelectionDirection::Backward {
                        (bytes.end, bytes.start)
                    } else {
                        (bytes.start, bytes.end)
                    };
                state.dom.doc.with_text_input(control, |mut driver| {
                    driver.select_byte_range(anchor, focus)
                });
                #[cfg(all(feature = "clipboard", feature = "shell"))]
                clipboard_io::editor::cancel(&mut state);
                state.dom.invalidate_pixels();
                state.observation_pending = true;
                Ok(())
            }
            glory_render::Action::RevealOption { node } => choice::reveal(&mut state, node),
            glory_render::Action::ExitMenu { node, backwards } => {
                #[cfg(feature = "shell")]
                {
                    shell::menu::exit(&mut state, node, backwards)
                }
                #[cfg(not(feature = "shell"))]
                {
                    let _ = (node, backwards);
                    Err(Error::Unsupported("headless Blitz menu focus"))
                }
            }
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
        #[cfg(feature = "files")]
        if matches!(query, Query::FileRead(_) | Query::FilePreview(_)) {
            let result = layers::sync(&mut state, &[])
                .and_then(|()| files_io::request(&mut state, query, token));
            if let Err(error) = result {
                let applied = state.receipt.applied;
                state.inputs.push(Input::Response {
                    token,
                    applied,
                    result: Err(error),
                });
            }
            return Ok(());
        }
        let result = (|| {
            layers::sync(&mut state, &[])?;
            query.validate_target(&state.mapper.tree().node(query.node())?.descriptor)?;
            if !state.mapper.tree().active(query.node()) {
                return Err(Error::Unsupported("query on disconnected Blitz range"));
            }
            let node = state.dom.main(query.node())?;
            state.dom.resolve();
            match query {
                Query::RichSelection(_) | Query::FileRead(_) | Query::FilePreview(_) => {
                    Err(Error::Unsupported("native file access"))
                }
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
                    let translation = state.dom.translations.offset(
                        state.mapper.tree(),
                        query.node(),
                        state.dom.time,
                    )?;
                    Ok(QueryValue::Bounds(Rect {
                        x: rect.x + own_scroll.x + translation.x,
                        y: rect.y + own_scroll.y + translation.y,
                        width: rect.width,
                        height: rect.height,
                    }))
                }
                Query::ScrollMetrics(_) => {
                    let host = state
                        .dom
                        .doc
                        .get_node(node)
                        .ok_or(Error::NodeGone(query.node()))?;
                    let offset = host.scroll_offset();
                    Ok(QueryValue::ScrollMetrics(glory_render::ScrollMetrics {
                        x: offset.x,
                        y: offset.y,
                        viewport_width: f64::from(host.client_width().max(0.0)),
                        viewport_height: f64::from(host.client_height().max(0.0)),
                        content_width: f64::from(host.scroll_width().max(0.0)),
                        content_height: f64::from(host.scroll_height().max(0.0)),
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
        self.0.borrow().dom.resource_wake.acknowledge();
        #[cfg(feature = "clipboard")]
        clipboard_io::poll(self);
        #[cfg(feature = "files")]
        files_io::poll(self);
        let mut state = self.0.borrow_mut();
        if let Err(error) = layers::sync(&mut state, &[]) {
            state.input_error = Some(error);
        }
        state.dom.resolve();
        state.collect_image_inputs();
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
        layers::drained(&mut state);
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
