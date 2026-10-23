use std::collections::{BTreeMap, HashMap, HashSet};
use std::rc::Rc;

use glory_html::{EditKind, Kind, Mapper, Part, Property, Shape, Sink};
use glory_render::{Action, Event, EventKind, Input, NodeId, Rect, Role};
use glory_ui::{App, Scope, Widget};
use wasm_bindgen::JsCast;
use web_sys::{Element, Node};

use super::{Renderer, WebApp, backend, dom, frame::Frame, perform};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HydrationError {
    Ui(glory_ui::Error),
    #[cfg(feature = "resources")]
    Resources(glory_resource::Error),
    Version,
    Unstable,
    Mismatch {
        key: Option<u64>,
        reason: &'static str,
    },
}
impl From<glory_ui::Error> for HydrationError {
    fn from(error: glory_ui::Error) -> Self {
        Self::Ui(error)
    }
}
impl From<glory_render::Error> for HydrationError {
    fn from(error: glory_render::Error) -> Self {
        Self::Ui(error.into())
    }
}
impl std::fmt::Display for HydrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for HydrationError {}
type Result<T = ()> = std::result::Result<T, HydrationError>;
fn mismatch(key: Option<u64>, reason: &'static str) -> HydrationError {
    HydrationError::Mismatch { key, reason }
}

struct Candidate {
    id: NodeId,
    key: u64,
    shape: Shape,
    parts: BTreeMap<Part, Node>,
    first: Node,
    last: Node,
}
struct Empty {
    parent: Node,
    before: Option<Node>,
    node: Node,
}
struct Plan {
    entries: Vec<Candidate>,
    empty: Vec<Empty>,
    inputs: Vec<Input>,
}
struct Validator<'a> {
    dom: &'a dom::Dom,
    mapper: &'a Mapper,
    plan: Plan,
}
impl Validator<'_> {
    fn comment(&self, cursor: &mut Option<Node>, expected: &str, key: u64) -> Result<Node> {
        let node = cursor
            .clone()
            .filter(|node| {
                node.node_type() == Node::COMMENT_NODE
                    && node.node_value().as_deref() == Some(expected)
            })
            .ok_or_else(|| mismatch(Some(key), "comment boundary"))?;
        *cursor = node.next_sibling();
        Ok(node)
    }
    fn node(&mut self, id: NodeId, cursor: &mut Option<Node>) -> Result {
        let model = self.mapper.tree().node(id)?;
        if model.suspended {
            return Ok(());
        }
        let key = self.mapper.key(id).expect("live HTML key");
        let children = model.children.clone();
        let shape = self.mapper.shape(id)?;
        let mut parts = BTreeMap::new();
        let (first, last) = if shape.pieces.is_empty() {
            let first = self.comment(cursor, &format!("gly:s:{key}"), key)?;
            for child in &children {
                self.node(*child, cursor)?;
            }
            let last = self.comment(cursor, &format!("gly:e:{key}"), key)?;
            (first, last)
        } else {
            let root = self.piece(&shape, Part::Main, key, &children, cursor, &mut parts)?;
            (root.clone(), root)
        };
        if let Some(control) = shape
            .control
            .and_then(|part| parts.get(&part))
            .and_then(|node| node.dyn_ref::<Element>())
        {
            let piece = shape
                .pieces
                .iter()
                .find(|piece| Some(piece.part) == shape.control)
                .expect("control piece");
            let Kind::Element { properties, .. } = &piece.kind else {
                unreachable!()
            };
            match shape.edit {
                EditKind::Text => {
                    let composing = dom::preload_composing(control);
                    if composing {
                        self.plan.inputs.push(Input::Event {
                            node: id,
                            event: Event::Composition { active: true },
                        });
                    }
                    let raw = js_sys::Reflect::get(control, &"value".into())
                        .map_err(backend)?
                        .as_string()
                        .unwrap_or_default();
                    // Validation must not mutate the existing document. The
                    // adopted Entry repairs only noncanonical host text later.
                    let actual =
                        glory_render::normalize_text(&raw, control.tag_name() == "TEXTAREA")
                            .into_owned();
                    let probe = control.clone_node().map_err(backend)?;
                    let Property::Text(initial) = &properties["value"] else {
                        unreachable!()
                    };
                    js_sys::Reflect::set(&probe, &"value".into(), &initial.into())
                        .map_err(backend)?;
                    let initial = js_sys::Reflect::get(&probe, &"value".into())
                        .map_err(backend)?
                        .as_string()
                        .unwrap_or_default();
                    if actual != initial {
                        self.plan.inputs.push(Input::Event {
                            node: id,
                            event: Event::ValueChanged {
                                value: actual,
                                composing,
                                revision: shape.revision,
                            },
                        });
                    }
                }
                EditKind::RichText => {
                    let (document, selection) = super::rich_text::read(control)?;
                    let composing = dom::preload_composing(control);
                    if properties.get("richDocument")
                        != Some(&Property::RichDocument(document.clone()))
                    {
                        self.plan.inputs.push(Input::Event {
                            node: id,
                            event: Event::RichTextChanged {
                                document,
                                selection,
                                composing,
                                revision: shape.revision,
                            },
                        });
                    }
                    if composing {
                        self.plan.inputs.push(Input::Event {
                            node: id,
                            event: Event::Composition { active: true },
                        });
                    }
                }
                EditKind::Check => {
                    let actual = js_sys::Reflect::get(control, &"checked".into())
                        .map_err(backend)?
                        .as_bool()
                        .unwrap_or(false);
                    if properties.get("checked") != Some(&Property::Bool(actual)) {
                        self.plan.inputs.push(Input::Event {
                            node: id,
                            event: Event::CheckedChanged(actual),
                        });
                    }
                }
                EditKind::None => {}
            }
            if self.dom.document.active_element().as_ref() == Some(control) {
                self.plan.inputs.push(Input::Event {
                    node: id,
                    event: Event::FocusChanged(true),
                });
            }
        }
        self.plan.entries.push(Candidate {
            id,
            key,
            shape,
            parts,
            first,
            last,
        });
        Ok(())
    }
    fn piece(
        &mut self,
        shape: &Shape,
        part: Part,
        key: u64,
        children: &[NodeId],
        cursor: &mut Option<Node>,
        parts: &mut BTreeMap<Part, Node>,
    ) -> Result<Node> {
        let piece = shape
            .pieces
            .iter()
            .find(|piece| piece.part == part)
            .expect("shape part");
        let node = match &piece.kind {
            Kind::Text(expected) => {
                let marker = self.comment(cursor, &format!("gly:t:{key}"), key)?;
                if let Some(node) = cursor
                    .clone()
                    .filter(|node| node.node_type() == Node::TEXT_NODE)
                {
                    if node.node_value().as_deref() != Some(expected) {
                        return Err(mismatch(Some(key), "text"));
                    }
                    *cursor = node.next_sibling();
                    node
                } else if expected.is_empty() {
                    let node: Node = self.dom.document.create_text_node("").into();
                    self.plan.empty.push(Empty {
                        parent: marker.parent_node().expect("text marker parent"),
                        before: cursor.clone(),
                        node: node.clone(),
                    });
                    node
                } else {
                    return Err(mismatch(Some(key), "missing text"));
                }
            }
            Kind::Element {
                tag,
                attributes,
                properties,
            } => {
                let element = cursor
                    .clone()
                    .and_then(|node| node.dyn_into::<Element>().ok())
                    .ok_or_else(|| mismatch(Some(key), "element"))?;
                if element.local_name() != *tag
                    || element.namespace_uri().as_deref() != Some("http://www.w3.org/1999/xhtml")
                {
                    return Err(mismatch(Some(key), "tag/namespace"));
                }
                if element.get_attribute("data-gly-key") != Some(key.to_string())
                    || element.get_attribute("data-gly-part").as_deref() != Some(part.name())
                {
                    return Err(mismatch(Some(key), "key/part"));
                }
                if element.get_attribute_names().length() as usize != attributes.len() + 2 {
                    return Err(mismatch(Some(key), "unexpected attributes"));
                }
                for (name, expected) in attributes {
                    // Node identity belongs to the new client runtime. Validate
                    // the complete server relation below before rebasing any ID.
                    if matches!(*name, "id" | "aria-controls") {
                        continue;
                    }
                    if element.get_attribute(name).as_ref() != Some(expected) {
                        return Err(mismatch(Some(key), "attribute"));
                    }
                }
                let mut inner = element.first_child();
                if *tag == "textarea" {
                    let Some(Property::Text(value)) = properties.get("value") else {
                        unreachable!()
                    };
                    if element.text_content().as_deref() != Some(value) {
                        return Err(mismatch(Some(key), "textarea default"));
                    }
                    inner = None;
                }
                if properties.contains_key("richDocument") {
                    super::rich_text::read(&element)?;
                    inner = None;
                }
                for child in shape
                    .pieces
                    .iter()
                    .filter(|piece| piece.parent == Some(part))
                {
                    self.piece(shape, child.part, key, children, &mut inner, parts)?;
                }
                if shape.children == Some(part) {
                    for child in children {
                        self.node(*child, &mut inner)?;
                    }
                }
                if inner.is_some() {
                    return Err(mismatch(Some(key), "unexpected child"));
                }
                *cursor = element.next_sibling();
                element.into()
            }
        };
        parts.insert(part, node.clone());
        Ok(node)
    }
}

/// Adopt a versioned SSR Snapshot without remounting or replaying initial props.
/// A mismatch is diagnosed before mutating the existing subtree. Existing user
/// values/checked state win initial state via normal queued semantic events.
pub fn hydrate(root: Element, widget: impl Widget) -> Result<WebApp> {
    hydrate_with(root, move |_| widget)
}
pub fn hydrate_with<W: Widget>(root: Element, factory: impl FnOnce(&Scope) -> W) -> Result<WebApp> {
    if root.has_attribute("data-gly-app") {
        return Err(mismatch(None, "mount is already claimed"));
    }
    let start = root.first_child().ok_or(HydrationError::Version)?;
    let end = root.last_child().ok_or(HydrationError::Version)?;
    let version = glory_html::FORMAT_VERSION;
    if start.node_type() != Node::COMMENT_NODE
        || start.node_value().as_deref() != Some(&format!("gly:v:{version}"))
        || end.node_type() != Node::COMMENT_NODE
        || end.node_value().as_deref() != Some(&format!("gly:/v:{version}"))
    {
        return Err(HydrationError::Version);
    }
    let document = root
        .owner_document()
        .ok_or_else(|| mismatch(None, "document"))?;
    let window = document
        .default_view()
        .ok_or_else(|| mismatch(None, "window"))?;
    super::base_style(&document)?;
    let user_focus = document
        .active_element()
        .is_some_and(|active| root.contains(Some(active.as_ref())));
    let document_id = super::reserve_document_id(&document)?;
    let frame = Frame::new(window);
    let dom = dom::Dom::new(document, root, frame.clone());
    dom.staging.set(true);
    dom.live.set(false);
    dom.owned.set(false);
    dom.hold_actions.set(true);
    let mut mapper = None;
    let app = Rc::new(App::with_host(frame.clone(), |runtime| {
        let renderer = Renderer::new(runtime, dom.clone(), document_id);
        mapper = Some(renderer.mapper.clone());
        renderer
    }));
    frame.attach(&app);
    let guard = WebApp { app, frame };
    guard.app.mount_with(|| factory(guard.app.scope()))?;
    let mapper = mapper.expect("created renderer");
    mapper.borrow_mut().canonicalize()?;
    let mut plan = {
        let mapping = mapper.borrow();
        let mut validator = Validator {
            dom: &dom,
            mapper: &mapping,
            plan: Plan {
                entries: Vec::new(),
                empty: Vec::new(),
                inputs: Vec::new(),
            },
        };
        let mut cursor = start.next_sibling();
        for id in mapping.tree().roots() {
            validator.node(*id, &mut cursor)?;
        }
        if cursor.as_ref() != Some(&end) {
            return Err(mismatch(None, "extra root content"));
        }
        validator.plan
    };
    validate_disclosures(&dom, &mapper.borrow(), &plan.entries)?;
    let anchors = flow_anchors(&mapper.borrow(), &plan.entries)?;
    let anchored: HashSet<_> = anchors
        .iter()
        .filter_map(|input| {
            if let Input::Event { node, .. } = input {
                Some(*node)
            } else {
                None
            }
        })
        .collect();
    // Validation is complete. Only now create missing empty slots and attach
    // runtime metadata; no initial value setter or element move is replayed.
    *dom.root_end.borrow_mut() = Some(end);
    for empty in plan.empty {
        empty
            .parent
            .insert_before(&empty.node, empty.before.as_ref())
            .map_err(backend)?;
    }
    for entry in plan.entries {
        if let Some(element) = entry
            .parts
            .get(&Part::Main)
            .and_then(|node| node.dyn_ref::<Element>())
        {
            if let Some(glory_html::Piece {
                kind: Kind::Element { attributes, .. },
                ..
            }) = entry
                .shape
                .pieces
                .iter()
                .find(|piece| piece.part == Part::Main)
            {
                for name in ["id", "aria-controls"] {
                    if let Some(value) = attributes.get(name) {
                        element.set_attribute(name, value).map_err(backend)?;
                    }
                }
            }
        }
        dom.parents
            .borrow_mut()
            .insert(entry.id, mapper.borrow().tree().node(entry.id)?.parent);
        dom.adopt(
            entry.id,
            entry.key,
            &entry.shape,
            entry.parts,
            entry.first,
            entry.last,
        )?;
    }
    dom.staging.set(false);
    for id in mapper.borrow().tree().roots() {
        missing(&dom, &mapper.borrow(), *id)?;
    }
    let listeners = std::mem::take(&mut *dom.listeners.borrow_mut());
    for ((id, kind), enabled) in listeners {
        if dom.entry(id).is_ok() {
            dom.listen(id, kind, enabled)?;
        }
    }
    dom.owned.set(true);
    let startup = std::mem::take(&mut *dom.actions.borrow_mut());
    let mut sizes = HashMap::new();
    plan.inputs.extend(measurements(&dom, &mut sizes));
    plan.inputs.extend(anchors);
    dom.inputs.borrow_mut().extend(plan.inputs);
    guard.app.pump()?;
    let mut settled = false;
    for _ in 0..1024 {
        let inputs = measurements(&dom, &mut sizes);
        if inputs.is_empty() {
            settled = true;
            break;
        }
        dom.inputs.borrow_mut().extend(inputs);
        guard.app.pump()?;
    }
    if !settled {
        return Err(HydrationError::Unstable);
    }
    dom.activate()?;
    dom.hold_actions.set(false);
    let following = std::mem::take(&mut *dom.actions.borrow_mut());
    for action in startup
        .into_iter()
        .filter(|action| {
            !anchored.contains(&action.node())
                && !(user_focus && matches!(action, Action::Focus(_)))
        })
        .chain(following)
    {
        if guard.app.tree().active(action.node()) {
            perform(&dom, action)?;
        }
    }
    Ok(guard)
}

fn missing(dom: &Rc<dom::Dom>, mapper: &Mapper, id: NodeId) -> Result {
    let model = mapper.tree().node(id)?;
    let absent = dom.entry(id).is_err();
    if absent {
        dom.create(id, mapper.key(id).expect("HTML key"), mapper.shape(id)?)?;
    }
    for child in &model.children {
        missing(dom, mapper, *child)?;
        if absent && !mapper.tree().node(*child)?.suspended {
            dom::Sink(dom.clone()).apply(glory_html::Op::Place {
                id: *child,
                parent: Some(id),
                before: None,
            })?;
        }
    }
    Ok(())
}
fn layout_size(dom: &dom::Dom, element: &Element) -> Rect {
    if let Some(style) = dom
        .document
        .default_view()
        .and_then(|window| window.get_computed_style(element).ok().flatten())
    {
        let read = |name| {
            style
                .get_property_value(name)
                .ok()?
                .trim_end_matches("px")
                .parse::<f64>()
                .ok()
        };
        if let (Some(mut width), Some(mut height)) = (read("width"), read("height")) {
            if style.get_property_value("box-sizing").ok().as_deref() != Some("border-box") {
                width += [
                    "padding-left",
                    "padding-right",
                    "border-left-width",
                    "border-right-width",
                ]
                .into_iter()
                .map(|name| read(name).unwrap_or(0.0))
                .sum::<f64>();
                height += [
                    "padding-top",
                    "padding-bottom",
                    "border-top-width",
                    "border-bottom-width",
                ]
                .into_iter()
                .map(|name| read(name).unwrap_or(0.0))
                .sum::<f64>();
            }
            return Rect {
                width,
                height,
                ..Rect::default()
            };
        }
    }
    let rect = element.get_bounding_client_rect();
    Rect {
        width: rect.width(),
        height: rect.height(),
        ..Rect::default()
    }
}
fn measurements(dom: &dom::Dom, sizes: &mut HashMap<NodeId, Rect>) -> Vec<Input> {
    let entries: Vec<_> = dom
        .entries
        .borrow()
        .iter()
        .map(|(id, entry)| (*id, entry.clone()))
        .collect();
    let mut inputs = Vec::new();
    for (id, entry) in entries {
        if !entry.interests.borrow().contains(&EventKind::Resize) {
            continue;
        }
        let Ok(element) = entry.element() else {
            continue;
        };
        if !element.is_connected() {
            continue;
        }
        let size = layout_size(dom, &element);
        if sizes.insert(id, size) != Some(size) {
            inputs.push(Input::Event {
                node: id,
                event: Event::Resize {
                    size,
                    viewport: Rect {
                        width: element.client_width() as f64,
                        height: element.client_height() as f64,
                        ..Rect::default()
                    },
                },
            });
        }
    }
    inputs
}
fn flow_anchors(mapper: &Mapper, entries: &[Candidate]) -> Result<Vec<Input>> {
    let by_id: HashMap<_, _> = entries.iter().map(|entry| (entry.id, entry)).collect();
    let mut inputs = Vec::new();
    for candidate in entries {
        let model = mapper.tree().node(candidate.id)?;
        if model.descriptor.role != Some(Role::List) {
            continue;
        }
        let Some(surface) = model
            .children
            .first()
            .and_then(|id| mapper.tree().node(*id).ok())
        else {
            continue;
        };
        if surface
            .descriptor
            .options
            .get::<glory_render::VirtualPart>()
            != Some(&glory_render::VirtualPart::Surface)
        {
            continue;
        }
        let Some(root) = candidate
            .parts
            .get(&Part::Main)
            .and_then(|node| node.dyn_ref::<Element>())
        else {
            continue;
        };
        let viewport = Rect {
            width: root.client_width() as f64,
            height: root.client_height() as f64,
            ..Rect::default()
        };
        if viewport.height == 0.0 {
            continue;
        }
        let rect = root.get_bounding_client_rect();
        let scale = root
            .dyn_ref::<web_sys::HtmlElement>()
            .map(|element| element.offset_height())
            .filter(|height| *height > 0)
            .map_or(1.0, |height| rect.height() / height as f64);
        let top = rect.top() + root.client_top() as f64 * scale;
        for row in &surface.children {
            let Some(element) = by_id
                .get(row)
                .and_then(|entry| entry.parts.get(&Part::Main))
                .and_then(|node| node.dyn_ref::<Element>())
            else {
                continue;
            };
            let bounds = element.get_bounding_client_rect();
            if bounds.bottom() > top && bounds.top() < top + viewport.height * scale {
                inputs.push(Input::Event {
                    node: candidate.id,
                    event: Event::Anchor {
                        row: *row,
                        within: ((top - bounds.top()) / scale).max(0.0),
                        viewport,
                    },
                });
                break;
            }
        }
    }
    Ok(inputs)
}

// This pass only reads the candidate subtree. Cross-root/stale/malformed IDs
// reject adoption before input, ID or DOM ownership changes.
fn validate_disclosures(dom: &dom::Dom, mapper: &Mapper, entries: &[Candidate]) -> Result {
    let elements: HashMap<_, _> = entries
        .iter()
        .filter_map(|entry| {
            entry
                .parts
                .get(&Part::Main)
                .and_then(|node| node.dyn_ref::<Element>())
                .map(|element| (entry.id, element))
        })
        .collect();
    let mut ids = HashSet::new();
    for entry in entries {
        let descriptor = &mapper.tree().node(entry.id)?.descriptor;
        if matches!(descriptor.props, glory_render::Props::View) {
            let element = elements[&entry.id];
            let id = element.id();
            if !valid_server_id(&id)
                || !ids.insert(id.clone())
                || dom.document.get_element_by_id(&id).as_ref() != Some(element)
            {
                return Err(mismatch(Some(entry.key), "view identity"));
            }
        }
        if let Some(disclosure) = descriptor.disclosure() {
            let target = elements
                .get(&disclosure.controls)
                .ok_or_else(|| mismatch(Some(entry.key), "disclosure target not in snapshot"))?;
            if elements[&entry.id]
                .get_attribute("aria-controls")
                .as_deref()
                != Some(target.id().as_str())
            {
                return Err(mismatch(Some(entry.key), "disclosure target"));
            }
        }
    }
    Ok(())
}

fn valid_server_id(value: &str) -> bool {
    let Some(value) = value.strip_prefix("gly-server-") else {
        return false;
    };
    let mut fields = value.split('-');
    let valid = |value: Option<&str>, maximum: u64| {
        value.is_some_and(|value| {
            !value.is_empty()
                && (value == "0" || !value.starts_with('0'))
                && value.bytes().all(|ch| ch.is_ascii_digit())
                && value.parse::<u64>().is_ok_and(|value| value <= maximum)
        })
    };
    valid(fields.next(), u64::MAX)
        && valid(fields.next(), u32::MAX as u64)
        && valid(fields.next(), u32::MAX as u64)
        && fields.next().is_none()
}
