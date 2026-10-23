use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use glory_html::{Edit, EditKind, Kind, Op, Part, Property, Shape};
use glory_render::{Error, Event, EventKind, Input, NodeId, Rect, ScrollAck};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, DocumentFragment, Element, HtmlElement, Node, ResizeObserver};

use super::{backend, frame::Frame};

#[cfg(feature = "hydrate")]
pub(super) fn preload_composing(element: &Element) -> bool {
    js_sys::Reflect::get(
        element,
        &js_sys::Symbol::for_("glory.preload.composition").into(),
    )
    .ok()
    .and_then(|value| value.as_bool())
    .unwrap_or(false)
}

#[derive(Default)]
pub(super) struct Editing {
    pub composing: bool,
    pub revision: u64,
    requested: u64,
    pending: Option<String>,
}
pub(super) struct Entry {
    token: String,
    pub parts: BTreeMap<Part, Node>,
    pub first: Node,
    pub last: Node,
    pub fragment: DocumentFragment,
    pub children: Option<Part>,
    control: Option<Part>,
    pub edit: EditKind,
    pub editing: RefCell<Editing>,
    pub interests: RefCell<BTreeSet<EventKind>>,
    pub scroll_request: Cell<Option<ScrollAck>>,
    pub prevent_default: bool,
}
impl Entry {
    pub fn element(&self) -> Result<Element, Error> {
        self.part(Part::Main)?
            .dyn_into()
            .map_err(|_| Error::Unsupported("Group has no element"))
    }
    pub fn part(&self, part: Part) -> Result<Node, Error> {
        self.parts
            .get(&part)
            .cloned()
            .ok_or(Error::Unsupported("HTML part"))
    }
    pub fn control(&self) -> Result<Element, Error> {
        self.part(
            self.control
                .ok_or(Error::Unsupported("Group has no control"))?,
        )?
        .dyn_into()
        .map_err(|value: Node| backend(value.into()))
    }
    pub fn value(&self) -> Result<String, Error> {
        let control = self.control()?;
        let raw = js_sys::Reflect::get(&control, &"value".into())
            .map_err(backend)?
            .as_string()
            .unwrap_or_default();
        match glory_render::normalize_text(&raw, control.tag_name() == "TEXTAREA") {
            std::borrow::Cow::Borrowed(_) => Ok(raw),
            std::borrow::Cow::Owned(value) => {
                // Browsers already normalize line breaks in these native
                // properties. NUL -> U+FFFD keeps UTF-16 length; preserve the
                // current selection when repairing noncanonical host text.
                let selection = ["selectionStart", "selectionEnd", "selectionDirection"]
                    .map(|name| (name, js_sys::Reflect::get(&control, &name.into()).ok()));
                js_sys::Reflect::set(&control, &"value".into(), &value.as_str().into())
                    .map_err(backend)?;
                for (name, saved) in selection {
                    if let Some(saved) =
                        saved.filter(|value| !value.is_null() && !value.is_undefined())
                    {
                        js_sys::Reflect::set(&control, &name.into(), &saved).map_err(backend)?;
                    }
                }
                Ok(value)
            }
        }
    }
    fn property(&self, part: Part, name: &str, value: &Property) -> Result<(), Error> {
        let node = self.part(part)?;
        if name == "value"
            && let Property::Text(value) = value
        {
            let current = self.value()?;
            let mut edit = self.editing.borrow_mut();
            if edit.composing && current != *value {
                edit.pending = Some(value.clone());
                return Ok(());
            }
            if current != *value {
                js_sys::Reflect::set(&node, &name.into(), &value.into()).map_err(backend)?;
            }
            edit.revision = edit.requested;
            edit.pending = None;
            return Ok(());
        }
        let value: JsValue = match value {
            Property::Text(value) => value.into(),
            Property::Bool(value) => (*value).into(),
        };
        js_sys::Reflect::set(&node, &name.into(), &value).map_err(backend)?;
        Ok(())
    }
    pub fn composition_end(&self) -> Result<(), Error> {
        let pending = {
            let mut edit = self.editing.borrow_mut();
            edit.composing = false;
            edit.pending.take()
        };
        if let Some(pending) = pending {
            self.property(
                self.control.expect("input control"),
                "value",
                &Property::Text(pending),
            )?;
        }
        Ok(())
    }
}

pub(super) struct Dom {
    pub document: Document,
    pub root: Element,
    pub frame: Rc<Frame>,
    pub entries: RefCell<HashMap<NodeId, Rc<Entry>>>,
    pub tokens: RefCell<HashMap<String, NodeId>>,
    pub inputs: RefCell<Vec<Input>>,
    pub layout: RefCell<BTreeSet<(NodeId, EventKind)>>,
    pub sizes: RefCell<HashMap<NodeId, Rect>>,
    pub observer: RefCell<Option<ResizeObserver>>,
    pub viewport_observer: RefCell<Option<ResizeObserver>>,
    pub error: RefCell<Option<Error>>,
    cleared: Cell<bool>,
    pub staging: Cell<bool>,
    pub live: Cell<bool>,
    pub owned: Cell<bool>,
    pub hold_actions: Cell<bool>,
    pub listeners: RefCell<BTreeMap<(NodeId, EventKind), bool>>,
    pub actions: RefCell<Vec<glory_render::Action>>,
    pub root_end: RefCell<Option<Node>>,
}
impl Dom {
    pub fn new(document: Document, root: Element, frame: Rc<Frame>) -> Rc<Self> {
        let error = root.set_attribute("data-gly-app", "").err().map(backend);
        Rc::new(Self {
            document,
            root,
            frame,
            entries: RefCell::new(HashMap::new()),
            tokens: RefCell::new(HashMap::new()),
            inputs: RefCell::new(Vec::new()),
            layout: RefCell::new(BTreeSet::new()),
            sizes: RefCell::new(HashMap::new()),
            observer: RefCell::new(None),
            viewport_observer: RefCell::new(None),
            error: RefCell::new(error),
            cleared: Cell::new(false),
            staging: Cell::new(false),
            live: Cell::new(true),
            owned: Cell::new(true),
            hold_actions: Cell::new(false),
            listeners: RefCell::new(BTreeMap::new()),
            actions: RefCell::new(Vec::new()),
            root_end: RefCell::new(None),
        })
    }
    pub fn entry(&self, id: NodeId) -> Result<Rc<Entry>, Error> {
        self.entries
            .borrow()
            .get(&id)
            .cloned()
            .ok_or(Error::NodeGone(id))
    }
    #[cfg(feature = "hydrate")]
    pub fn adopt(
        &self,
        id: NodeId,
        key: u64,
        shape: &Shape,
        parts: BTreeMap<Part, Node>,
        first: Node,
        last: Node,
    ) -> Result<(), Error> {
        let composing = shape
            .control
            .and_then(|part| parts.get(&part))
            .and_then(|node| node.dyn_ref::<Element>())
            .is_some_and(preload_composing);
        let token = format!("{}.{}.{}", id.runtime().value(), id.slot(), id.generation());
        for (part, node) in &parts {
            if let Some(element) = node.dyn_ref::<Element>() {
                element
                    .set_attribute("data-gly-node", &token)
                    .map_err(backend)?;
                debug_assert_eq!(element.get_attribute("data-gly-key"), Some(key.to_string()));
                debug_assert_eq!(
                    element.get_attribute("data-gly-part"),
                    Some(part.name().into())
                );
            }
        }
        let entry = Rc::new(Entry {
            token: token.clone(),
            parts,
            first,
            last,
            fragment: self.document.create_document_fragment(),
            children: shape.children,
            control: shape.control,
            edit: shape.edit,
            editing: RefCell::new(Editing {
                composing,
                revision: shape.revision,
                requested: shape.revision,
                ..Editing::default()
            }),
            interests: RefCell::new(BTreeSet::new()),
            scroll_request: Cell::new(None),
            prevent_default: shape.prevent_activate_default,
        });
        if entry.edit == EditKind::Text {
            entry.value()?;
        }
        self.entries.borrow_mut().insert(id, entry);
        self.tokens.borrow_mut().insert(token, id);
        Ok(())
    }
    #[cfg(feature = "hydrate")]
    pub fn activate(&self) -> Result<(), Error> {
        let entries: Vec<_> = self.entries.borrow().values().cloned().collect();
        for entry in entries {
            for node in entry.parts.values() {
                if let Some(element) = node.dyn_ref::<Element>() {
                    element
                        .set_attribute("data-gly-live", "")
                        .map_err(backend)?;
                }
            }
        }
        self.live.set(true);
        Ok(())
    }
    pub fn listen(&self, id: NodeId, kind: EventKind, enabled: bool) -> Result<(), Error> {
        if self.staging.get() {
            self.listeners.borrow_mut().insert((id, kind), enabled);
            return Ok(());
        }
        let entry = self.entry(id)?;
        if enabled {
            entry.interests.borrow_mut().insert(kind);
        } else {
            entry.interests.borrow_mut().remove(&kind);
        }
        if kind == EventKind::Resize {
            let element = entry.element()?;
            let observer = self.observer.borrow();
            let observer = observer
                .as_ref()
                .ok_or(Error::Unsupported("ResizeObserver"))?;
            if enabled {
                let options = web_sys::ResizeObserverOptions::new();
                options.set_box(web_sys::ResizeObserverBoxOptions::BorderBox);
                observer.observe_with_options(&element, &options);
            } else {
                observer.unobserve(&element);
                self.sizes.borrow_mut().remove(&id);
                self.layout.borrow_mut().remove(&(id, kind));
            }
        }
        if matches!(kind, EventKind::Resize | EventKind::Scroll) {
            let element = entry.element()?;
            let needs_viewport = {
                let interests = entry.interests.borrow();
                interests.contains(&EventKind::Resize) && interests.contains(&EventKind::Scroll)
            };
            if needs_viewport {
                // A scrollbar may change client dimensions while the outer
                // border box stays fixed. Only scroll consumers need this
                // second observation; ordinary measured rows use border-box.
                self.viewport_observer
                    .borrow()
                    .as_ref()
                    .ok_or(Error::Unsupported("viewport ResizeObserver"))?
                    .observe(&element);
            } else if let Some(observer) = self.viewport_observer.borrow().as_ref() {
                observer.unobserve(&element);
            }
        }
        Ok(())
    }
    pub fn queue(&self, id: NodeId, event: Event) {
        let mut inputs = self.inputs.borrow_mut();
        // Coalesce continuous input only inside the current uninterrupted event
        // segment. Composition/commit/activation preserve their ordering.
        // Focus transitions are ordering barriers. Replacing an earlier A
        // focus event across B loses the final target for rapid A→B→A moves.
        if matches!(event, Event::ValueChanged { .. }) {
            for input in inputs.iter_mut().rev() {
                match input {
                    Input::Event {
                        node,
                        event: previous,
                    } if *node == id && previous.kind() == event.kind() => {
                        *previous = event;
                        drop(inputs);
                        self.frame.input();
                        return;
                    }
                    Input::Event {
                        event: Event::ValueChanged { .. },
                        ..
                    } => {}
                    _ => break,
                }
            }
        }
        inputs.push(Input::Event { node: id, event });
        drop(inputs);
        self.frame.input();
    }
    pub fn read_layout(&self) {
        // Exactly one read phase before App processes events and writes DOM.
        let pending = std::mem::take(&mut *self.layout.borrow_mut());
        let mut inputs = self.inputs.borrow_mut();
        for (id, kind) in pending {
            let Ok(entry) = self.entry(id) else { continue };
            if !entry.interests.borrow().contains(&kind) {
                continue;
            }
            let Ok(element) = entry.element() else {
                continue;
            };
            if !element.is_connected() {
                continue;
            }
            let viewport = Rect {
                x: 0.0,
                y: 0.0,
                width: element.client_width() as f64,
                height: element.client_height() as f64,
            };
            let event = if kind == EventKind::Scroll {
                let x = number(&element, "scrollLeft");
                let y = number(&element, "scrollTop");
                Event::Scroll {
                    x,
                    y,
                    viewport,
                    ack: entry.scroll_request.get(),
                }
            } else {
                let size = self.sizes.borrow_mut().remove(&id).unwrap_or_else(|| {
                    let bounds = element.get_bounding_client_rect();
                    Rect {
                        width: bounds.width(),
                        height: bounds.height(),
                        ..Rect::default()
                    }
                });
                Event::Resize { size, viewport }
            };
            inputs.push(Input::Event { node: id, event });
        }
    }
    pub fn create(&self, id: NodeId, key: u64, shape: Shape) -> Result<(), Error> {
        let token = format!("{}.{}.{}", id.runtime().value(), id.slot(), id.generation());
        let mut parts = BTreeMap::<Part, Node>::new();
        let fragment = self.document.create_document_fragment();
        for piece in &shape.pieces {
            let node: Node = match &piece.kind {
                Kind::Text(value) => self.document.create_text_node(value).into(),
                Kind::Element {
                    tag, attributes, ..
                } => {
                    let element = self.document.create_element(tag).map_err(backend)?;
                    for (name, value) in attributes {
                        element.set_attribute(name, value).map_err(backend)?;
                    }
                    element
                        .set_attribute("data-gly-node", &token)
                        .map_err(backend)?;
                    element
                        .set_attribute("data-gly-key", &key.to_string())
                        .map_err(backend)?;
                    element
                        .set_attribute("data-gly-part", piece.part.name())
                        .map_err(backend)?;
                    if self.live.get() {
                        element
                            .set_attribute("data-gly-live", "")
                            .map_err(backend)?;
                    }
                    element.into()
                }
            };
            let parent: &Node = piece
                .parent
                .and_then(|part| parts.get(&part))
                .unwrap_or(fragment.as_ref());
            if matches!(piece.kind, Kind::Text(_)) {
                parent
                    .append_child(&self.document.create_comment(&format!("gly:t:{key}")))
                    .map_err(backend)?;
            }
            parent.append_child(&node).map_err(backend)?;
            parts.insert(piece.part, node);
        }
        if parts.is_empty() {
            fragment
                .append_child(&self.document.create_comment(&format!("gly:s:{key}")))
                .map_err(backend)?;
            fragment
                .append_child(&self.document.create_comment(&format!("gly:e:{key}")))
                .map_err(backend)?;
        }
        let entry = Rc::new(Entry {
            token: token.clone(),
            first: fragment.first_child().expect("created range"),
            last: fragment.last_child().expect("created range"),
            fragment,
            parts,
            children: shape.children,
            control: shape.control,
            edit: shape.edit,
            editing: RefCell::new(Editing {
                revision: shape.revision,
                requested: shape.revision,
                ..Editing::default()
            }),
            interests: RefCell::new(BTreeSet::new()),
            scroll_request: Cell::new(None),
            prevent_default: shape.prevent_activate_default,
        });
        for piece in &shape.pieces {
            if let Kind::Element { properties, .. } = &piece.kind {
                for (name, value) in properties {
                    entry.property(piece.part, name, value)?;
                }
            }
        }
        self.entries.borrow_mut().insert(id, entry);
        self.tokens.borrow_mut().insert(token, id);
        Ok(())
    }
    fn place(
        &self,
        id: NodeId,
        parent: Option<NodeId>,
        before: Option<NodeId>,
    ) -> Result<(), Error> {
        let entry = self.entry(id)?;
        let (parent, end) = if let Some(parent) = parent {
            let parent = self.entry(parent)?;
            if let Some(part) = parent.children {
                (parent.part(part)?, None)
            } else if parent.parts.is_empty() {
                (
                    parent
                        .last
                        .parent_node()
                        .ok_or(Error::InvalidPlacement("detached Group end"))?,
                    Some(parent.last.clone()),
                )
            } else {
                return Err(Error::InvalidProps("node has no HTML child slot"));
            }
        } else {
            (self.root.clone().into(), self.root_end.borrow().clone())
        };
        let before = if let Some(id) = before {
            Some(self.entry(id)?.first.clone())
        } else {
            end
        };
        let nodes = range(&entry)?;
        let selection = Selection::capture(&self.document);
        // moveBefore keeps focus, IME and element state on supporting engines.
        let mover = js_sys::Reflect::get(&parent, &"moveBefore".into())
            .ok()
            .and_then(|value| value.dyn_into::<js_sys::Function>().ok());
        for node in nodes {
            let used = node.is_connected()
                && parent.is_connected()
                && mover.as_ref().is_some_and(|mover| {
                    mover
                        .call2(
                            &parent,
                            &node,
                            before.as_ref().map_or(&JsValue::NULL, |node| node.as_ref()),
                        )
                        .is_ok()
                });
            if !used {
                parent
                    .insert_before(&node, before.as_ref())
                    .map_err(backend)?;
            }
        }
        selection.restore(&self.document);
        Ok(())
    }
    fn patch(&self, id: NodeId, edits: Vec<Edit>) -> Result<(), Error> {
        let entry = self.entry(id)?;
        for edit in edits {
            match edit {
                Edit::Revision(revision) => entry.editing.borrow_mut().requested = revision,
                Edit::Text { part, value } => entry.part(part)?.set_node_value(Some(&value)),
                Edit::Property { part, name, value } => entry.property(part, name, &value)?,
                Edit::Attribute { part, name, value } => {
                    let element = entry
                        .part(part)?
                        .dyn_into::<Element>()
                        .map_err(|value: Node| backend(value.into()))?;
                    if let Some(value) = value {
                        element.set_attribute(name, &value).map_err(backend)?;
                    } else {
                        element.remove_attribute(name).map_err(backend)?;
                    }
                }
            }
        }
        Ok(())
    }
    fn dispose(&self, id: NodeId, subtree: Vec<NodeId>) -> Result<(), Error> {
        let entry = self.entry(id)?;
        // Remove lookup/queued state before native focus events from removal.
        let removed: Vec<_> = {
            let mut entries = self.entries.borrow_mut();
            subtree.iter().filter_map(|id| entries.remove(id)).collect()
        };
        let subtree: HashSet<_> = subtree.into_iter().collect();
        {
            let mut tokens = self.tokens.borrow_mut();
            for entry in &removed {
                tokens.remove(&entry.token);
            }
        }
        self.layout
            .borrow_mut()
            .retain(|(id, _)| !subtree.contains(id));
        self.sizes
            .borrow_mut()
            .retain(|id, _| !subtree.contains(id));
        self.inputs
            .borrow_mut()
            .retain(|input| !matches!(input, Input::Event { node, .. } if subtree.contains(node)));
        if let Some(observer) = self.observer.borrow().as_ref() {
            for entry in &removed {
                if entry.interests.borrow().contains(&EventKind::Resize)
                    && let Ok(element) = entry.element()
                {
                    observer.unobserve(&element);
                    if let Some(observer) = self.viewport_observer.borrow().as_ref() {
                        observer.unobserve(&element);
                    }
                }
            }
        }
        for node in range(&entry)? {
            if let Some(parent) = node.parent_node() {
                parent.remove_child(&node).map_err(backend)?;
            }
        }
        Ok(())
    }
    pub fn clear(&self) {
        if self.cleared.replace(true) {
            return;
        }
        if let Some(observer) = self.observer.borrow_mut().take() {
            observer.disconnect();
        }
        if let Some(observer) = self.viewport_observer.borrow_mut().take() {
            observer.disconnect();
        }
        let entries = std::mem::take(&mut *self.entries.borrow_mut());
        self.tokens.borrow_mut().clear();
        self.layout.borrow_mut().clear();
        self.sizes.borrow_mut().clear();
        self.inputs.borrow_mut().clear();
        // Root is claimed exclusively. Clear ranges even after a failed prefix.
        if self.owned.get() {
            self.root.set_text_content(None);
        } else {
            for entry in entries.values() {
                for node in entry.parts.values() {
                    if let Some(element) = node.dyn_ref::<Element>() {
                        let _ = element.remove_attribute("data-gly-node");
                        let _ = element.remove_attribute("data-gly-live");
                    }
                }
            }
        }
        let _ = self.root.remove_attribute("data-gly-app");
        drop(entries);
    }
}
pub(super) struct Sink(pub Rc<Dom>);
impl glory_html::Sink for Sink {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        if self.0.staging.get() {
            return Ok(());
        }
        match op {
            Op::Create { id, key, shape } => self.0.create(id, key, shape),
            Op::Patch { id, edits } => self.0.patch(id, edits),
            Op::Place { id, parent, before } => self.0.place(id, parent, before),
            Op::Dispose { id, subtree } => self.0.dispose(id, subtree),
            Op::Suspend { id } => {
                let entry = self.0.entry(id)?;
                for node in range(&entry)? {
                    entry.fragment.append_child(&node).map_err(backend)?;
                }
                Ok(())
            }
        }
    }
}
fn range(entry: &Entry) -> Result<Vec<Node>, Error> {
    let mut nodes = Vec::new();
    let mut next = Some(entry.first.clone());
    while let Some(node) = next {
        next = node.next_sibling();
        let last = node.is_same_node(Some(&entry.last));
        nodes.push(node);
        if last {
            return Ok(nodes);
        }
    }
    Err(Error::InvalidPlacement("broken HTML range"))
}
pub(super) fn number(element: &Element, name: &str) -> f64 {
    js_sys::Reflect::get(element, &name.into())
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0)
}
pub(super) fn rect(rect: &web_sys::DomRect) -> Rect {
    Rect {
        x: rect.x(),
        y: rect.y(),
        width: rect.width(),
        height: rect.height(),
    }
}
pub(super) fn focus(element: &Element) -> Result<(), Error> {
    let element = element
        .dyn_ref::<HtmlElement>()
        .ok_or(Error::Unsupported("focus on this element"))?;
    let options = web_sys::FocusOptions::new();
    options.set_prevent_scroll(true);
    element.focus_with_options(&options).map_err(backend)
}
pub(super) fn has_focus(element: &Element) -> Result<bool, Error> {
    if !element.is_connected() {
        return Ok(false);
    }
    // Document.activeElement retargets to a shadow host. Read the control's
    // actual root, which is either Document or ShadowRoot, without walking the
    // UI tree or inferring focus from tag/tabindex/geometry.
    let root = element.get_root_node();
    let active = js_sys::Reflect::get(root.as_ref(), &"activeElement".into()).map_err(backend)?;
    Ok(active.dyn_ref::<Element>() == Some(element))
}
struct Selection {
    active: Option<Element>,
    start: Option<JsValue>,
    end: Option<JsValue>,
    direction: Option<JsValue>,
}
impl Selection {
    fn capture(document: &Document) -> Self {
        let active = document.active_element();
        let read = |name: &str| {
            active
                .as_ref()
                .and_then(|active| js_sys::Reflect::get(active, &name.into()).ok())
        };
        let start = read("selectionStart");
        let end = read("selectionEnd");
        let direction = read("selectionDirection");
        Self {
            active,
            start,
            end,
            direction,
        }
    }
    fn restore(self, document: &Document) {
        let Some(active) = self.active else { return };
        if !active.is_connected() || document.active_element().as_ref() == Some(&active) {
            return;
        }
        let _ = focus(&active);
        for (name, value) in [
            ("selectionStart", self.start),
            ("selectionEnd", self.end),
            ("selectionDirection", self.direction),
        ] {
            if let Some(value) = value
                && !value.is_null()
                && !value.is_undefined()
            {
                let _ = js_sys::Reflect::set(&active, &name.into(), &value);
            }
        }
    }
}
