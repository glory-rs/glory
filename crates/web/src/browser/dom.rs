use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

use glory_html::{Edit, EditKind, Kind, Op, Part, Property, Shape};
use glory_render::{Error, Event, EventKind, Input, NodeId, Rect, ScrollAck};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, DocumentFragment, Element, HtmlElement, Node, ResizeObserver};

use super::{backend, frame::Frame};

#[path = "templates.rs"]
mod templates;
use templates::Templates;
#[path = "plans.rs"]
mod plans;
#[path = "submission.rs"]
pub(super) mod submission;

// HTML has exactly four typed parts. Explicit plans keep their original
// physical nodes together and only cross the bridge for parts actually used.
// The cache never searches a live DOM path (editing/moves cannot retarget it).
#[derive(Default)]
pub(super) struct Parts {
    nodes: [OnceCell<Node>; 4],
    planned: Option<(Rc<plans::Instance>, usize)>,
}
impl Parts {
    fn index(part: Part) -> usize {
        plans::part_index(part)
    }
    fn planned(instance: Rc<plans::Instance>, member: usize) -> Self {
        Self {
            nodes: Default::default(),
            planned: Some((instance, member)),
        }
    }
    fn contains(&self, part: Part) -> bool {
        self.nodes[Self::index(part)].get().is_some()
            || self
                .planned
                .as_ref()
                .is_some_and(|(instance, member)| instance.address(*member, part).is_some())
    }
    fn target(&self, part: Part) -> Option<plans::Target<'_>> {
        let (instance, member) = self.planned.as_ref()?;
        instance.target(*member, part)
    }
    pub fn get(&self, part: &Part) -> Option<&Node> {
        let slot = &self.nodes[Self::index(*part)];
        if let Some(node) = slot.get() {
            return Some(node);
        }
        let (instance, member) = self.planned.as_ref()?;
        let address = instance.address(*member, *part)?;
        Some(slot.get_or_init(|| instance.node(address)))
    }
    fn insert(&mut self, part: Part, node: Node) {
        self.nodes[Self::index(part)] = OnceCell::from(node);
    }
    pub fn values(&self) -> impl Iterator<Item = &Node> {
        [Part::Main, Part::Control, Part::Content, Part::Picker]
            .into_iter()
            .filter_map(|part| self.get(&part))
    }
    fn is_empty(&self) -> bool {
        [Part::Main, Part::Control, Part::Content, Part::Picker]
            .iter()
            .all(|part| !self.contains(*part))
    }
    fn retire(&self) {
        if let Some((instance, member)) = &self.planned {
            instance.retire(*member);
        }
    }
    fn flush_retired(&self) -> Result<(), Error> {
        if let Some((instance, _)) = &self.planned {
            instance.flush_retired()?;
        }
        Ok(())
    }
}
#[cfg(feature = "hydrate")]
impl From<BTreeMap<Part, Node>> for Parts {
    fn from(parts: BTreeMap<Part, Node>) -> Self {
        let mut result = Self::default();
        for (part, node) in parts {
            result.insert(part, node);
        }
        result
    }
}

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
    pub input_before: Option<(
        u64,
        glory_render::TextSelection,
        glory_render::InputEditIntent,
    )>,
    pub composing: bool,
    pub revision: u64,
    pub(super) requested: u64,
    pending: Option<String>,
}
// Boxed nodes already own their physical root in Parts. Only boxless Groups
// need separate boundary handles; retaining aliases costs externref slots and
// clone/drop bookkeeping for every instance.
enum Boundary {
    Part(Part),
    Range(Node, Node),
}
pub(super) struct Entry {
    pub display: RefCell<Option<super::display::Instance>>,
    pub placement: Option<glory_html::display::Placement>,
    pub(super) rich: RefCell<Option<Box<super::rich::Editing>>>,
    pub(super) collapse: RefCell<Option<Box<super::collapse::Observer>>>,
    pub(super) image: RefCell<Option<Box<super::images::Observer>>>,
    pub parent: Cell<Option<NodeId>>,
    #[cfg(feature = "hydrate")]
    is_virtual: bool,
    pub parts: Parts,
    boundary: Boundary,
    pub fragment: RefCell<Option<DocumentFragment>>,
    pub children: Option<Part>,
    control: Option<Part>,
    pub edit: EditKind,
    pub editing: RefCell<Editing>,
    pub interests: RefCell<BTreeSet<EventKind>>,
    pub scroll_request: Cell<Option<ScrollAck>>,
    pub prevent_default: bool,
}
impl Entry {
    /// Shared Rust state only. Creation replays new-control properties later;
    /// adoption supplies observed composition without rewriting the live input.
    fn new(
        shape: &Shape,
        parts: Parts,
        boundary: Boundary,
        parent: Option<NodeId>,
        fragment: Option<DocumentFragment>,
        composing: bool,
    ) -> Rc<Self> {
        Rc::new(Self {
            display: RefCell::new(None),
            placement: shape.display.clone(),
            rich: RefCell::new(None),
            collapse: RefCell::new(None),
            image: RefCell::new(None),
            parent: Cell::new(parent),
            #[cfg(feature = "hydrate")]
            is_virtual: virtual_shape(shape),
            parts,
            boundary,
            fragment: RefCell::new(fragment),
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
        })
    }

    pub(super) fn first(&self) -> &Node {
        match &self.boundary {
            Boundary::Part(part) => self.parts.get(part).expect("physical root part"),
            Boundary::Range(first, _) => first,
        }
    }
    fn last(&self) -> &Node {
        match &self.boundary {
            Boundary::Part(part) => self.parts.get(part).expect("physical root part"),
            Boundary::Range(_, last) => last,
        }
    }
    pub(super) fn image_revision(&self) -> u64 {
        self.editing.borrow().requested
    }
    pub fn element(&self) -> Result<Element, Error> {
        self.part(Part::Main)?
            .dyn_into()
            .map_err(|_| Error::Unsupported("Group has no element"))
    }
    pub fn part(&self, part: Part) -> Result<Node, Error> {
        self.node(part).cloned()
    }
    fn node(&self, part: Part) -> Result<&Node, Error> {
        self.parts.get(&part).ok_or(Error::Unsupported("HTML part"))
    }
    fn element_part(&self, part: Part) -> Result<&Element, Error> {
        let node = self.node(part)?;
        node.dyn_ref().ok_or_else(|| backend(node.clone().into()))
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
        let node = self.node(part)?;
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
                js_sys::Reflect::set(node, &name.into(), &value.into()).map_err(backend)?;
            }
            edit.revision = edit.requested;
            edit.pending = None;
            return Ok(());
        }
        if let Property::SelectedKeys(keys) = value {
            super::select::write(self.element_part(part)?, keys)?;
            let mut editing = self.editing.borrow_mut();
            editing.revision = editing.requested;
            return Ok(());
        }
        if let Property::RichDocument(document) = value {
            return super::rich::property(self, document.clone());
        }
        let value: JsValue = match value {
            Property::Text(value) => value.into(),
            Property::Bool(value) => (*value).into(),
            Property::SelectedKeys(_) => unreachable!(),
            Property::RichDocument(_) => unreachable!(),
        };
        js_sys::Reflect::set(node, &name.into(), &value).map_err(backend)?;
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

pub(super) struct DragSession {
    pub from: NodeId,
    pub pointer: i32,
    pub x: f64,
    pub y: f64,
    pub started: bool,
    pub captured: bool,
}
pub(super) struct Dom {
    pub(super) rich_pending: RefCell<Vec<NodeId>>,
    pub(super) rich_live: RefCell<BTreeSet<NodeId>>,
    pub(super) files: RefCell<super::files::Files>,
    pub(super) clipboards: RefCell<super::clipboard::Writes>,
    pub(super) translations: RefCell<super::translation::Translations>,
    pub(super) collapses: RefCell<super::collapse::Collapses>,
    pub(super) images: RefCell<super::images::Images>,
    pub layers: RefCell<super::layers::Layers>,
    pub document_layers: Rc<super::layers::DocumentLayers>,
    pub(super) sheets: RefCell<super::sheet::Sheets>,
    pub layer_layout: Cell<bool>,
    pub drag: RefCell<Option<DragSession>>,
    pub(super) swipe: RefCell<Option<super::swipe::Session>>,
    pub suppress_click: Cell<bool>,
    pub document: Document,
    pub root: Element,
    pub frame: Rc<Frame>,
    pub entries: RefCell<glory_render::NodeMap<Rc<Entry>>>,
    pub display_queue: Rc<glory_html::display::Queue>,
    templates: Templates,
    plans: plans::Plans,
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
    pub fn new(document: Document, root: Element, frame: Rc<Frame>) -> Result<Rc<Self>, Error> {
        let document_layers = super::layers::DocumentLayers::acquire(&document)?;
        let error = root.set_attribute("data-gly-app", "").err().map(backend);
        let files = super::files::Files::new(&document)?;
        let dom = Rc::new(Self {
            rich_pending: RefCell::new(Vec::new()),
            rich_live: RefCell::new(BTreeSet::new()),
            files: RefCell::new(files),
            translations: RefCell::new(super::translation::Translations::default()),
            collapses: RefCell::new(super::collapse::Collapses::default()),
            images: RefCell::new(super::images::Images::default()),
            clipboards: RefCell::new(super::clipboard::Writes::default()),
            layers: RefCell::new(super::layers::Layers::default()),
            document_layers: document_layers.clone(),
            sheets: RefCell::new(super::sheet::Sheets::default()),
            layer_layout: Cell::new(false),
            drag: RefCell::new(None),
            swipe: RefCell::new(None),
            suppress_click: Cell::new(false),
            document,
            root,
            frame,
            entries: RefCell::new(glory_render::NodeMap::default()),
            display_queue: Rc::new(glory_html::display::Queue::default()),
            templates: Templates::default(),
            plans: plans::Plans::default(),
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
        });
        document_layers.register(&dom);
        Ok(dom)
    }
    pub fn entry(&self, id: NodeId) -> Result<Rc<Entry>, Error> {
        self.entries
            .borrow()
            .get(&id)
            .cloned()
            .ok_or(Error::NodeGone(id))
    }
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.entries.borrow().get(&id)?.parent.get()
    }
    pub fn id_for_token(&self, token: &str) -> Option<NodeId> {
        self.entries.borrow().id_for_token(token)
    }
    #[cfg(feature = "hydrate")]
    pub fn adopt(
        &self,
        id: NodeId,
        key: u64,
        shape: &Shape,
        parent: Option<NodeId>,
        parts: BTreeMap<Part, Node>,
        first: Node,
        last: Node,
        display: Option<super::display::Instance>,
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
        let boundary = if shape.pieces.is_empty() {
            Boundary::Range(first, last)
        } else {
            Boundary::Part(shape.root_part()?)
        };
        let entry = Entry::new(shape, parts.into(), boundary, parent, None, composing);
        if let Some(display) = display {
            display.bind(self, id)?;
            *entry.display.borrow_mut() = Some(display);
        }
        if entry.edit == EditKind::Text {
            entry.value()?;
        }
        if entry.edit == EditKind::Check {
            // HTML cannot encode the native indeterminate property. Restore it
            // only after whole-subtree validation, preserving a pre-hydration
            // user toggle; controlled widgets reconcile through the queued event.
            if let Some(Kind::Element { properties, .. }) = shape
                .pieces
                .iter()
                .find(|piece| Some(piece.part) == shape.control)
                .map(|piece| &piece.kind)
            {
                let control = entry.control()?;
                let checked = js_sys::Reflect::get(&control, &"checked".into())
                    .map_err(backend)?
                    .as_bool()
                    .unwrap_or(false);
                if properties.get("checked") == Some(&Property::Bool(checked))
                    && let Some(value) = properties.get("indeterminate")
                {
                    entry.property(
                        shape.control.expect("check control"),
                        "indeterminate",
                        value,
                    )?;
                }
            }
        }
        if entry.edit == EditKind::Rich {
            self.rich_live.borrow_mut().insert(id);
        }
        self.entries.borrow_mut().insert(id, entry);
        Ok(())
    }
    #[cfg(feature = "hydrate")]
    pub fn activate(&self) -> Result<(), Error> {
        let entries: Vec<_> = self.entries.borrow().values().cloned().collect();
        for entry in entries {
            if !entry.is_virtual {
                continue;
            }
            for node in entry.parts.values() {
                if let Some(element) = node.dyn_ref::<Element>() {
                    element
                        .set_attribute("data-gly-live", "")
                        .map_err(backend)?;
                }
            }
        }
        self.live.set(true);
        super::layers::sync(self)?;
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
        if kind == EventKind::Swipe
            && !enabled
            && self
                .swipe
                .borrow()
                .as_ref()
                .is_some_and(|current| current.node == id)
        {
            super::swipe::cancel(self);
        }
        if kind == EventKind::ImageStatus {
            self.queue_image(id, &entry, enabled);
        }
        if kind == EventKind::CollapseComplete {
            self.queue_collapse(id, &entry, enabled);
        }
        if enabled
            && kind == EventKind::ColorSchemeChanged
            && let Some(media) = self.document.default_view().and_then(|window| {
                window
                    .match_media("(prefers-color-scheme: dark)")
                    .ok()
                    .flatten()
            })
        {
            self.queue(
                id,
                Event::ColorSchemeChanged {
                    dark: media.matches(),
                },
            );
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
        if matches!(
            event,
            Event::Swipe {
                phase: glory_render::DragPhase::Move,
                ..
            } | Event::Drag {
                phase: glory_render::DragPhase::Move,
                ..
            }
        ) && let Some(Input::Event {
            node,
            event: previous,
        }) = inputs.last_mut()
            && *node == id
            && previous.kind() == event.kind()
            && matches!(
                previous,
                Event::Swipe {
                    phase: glory_render::DragPhase::Move,
                    ..
                } | Event::Drag {
                    phase: glory_render::DragPhase::Move,
                    ..
                }
            )
        {
            *previous = event;
            drop(inputs);
            self.frame.input();
            return;
        }
        inputs.push(Input::Event { node: id, event });
        drop(inputs);
        self.frame.input();
    }
    pub fn read_layout(&self) {
        let sheet_writes = super::sheet::read(self);
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
        drop(inputs);
        if self.layer_layout.replace(false)
            && let Err(error) = super::layers::layout(self)
        {
            self.error.borrow_mut().get_or_insert(error);
        }
        if let Err(error) = sheet_writes.and_then(|writes| super::sheet::write(self, writes)) {
            self.error.borrow_mut().get_or_insert(error);
        }
    }
    pub fn create(&self, id: NodeId, key: u64, shape: Shape) -> Result<(), Error> {
        let token = format!("{}.{}.{}", id.runtime().value(), id.slot(), id.generation());
        let mut parts = Parts::default();
        let is_virtual = virtual_shape(&shape);
        // Parsed HTML needs key/part attributes and text markers to recover
        // identity. Fresh DOM already has typed part references; only Groups
        // need a physical range. Keep the serialized/adopted wire shape intact.
        let fragment = shape
            .pieces
            .is_empty()
            .then(|| self.document.create_document_fragment());
        for piece in &shape.pieces {
            let node: Node = match &piece.kind {
                Kind::Text(value) => self.document.create_text_node(value).into(),
                Kind::Element {
                    tag, attributes, ..
                } => {
                    let element = self.templates.element(
                        &self.document,
                        tag,
                        attributes,
                        &token,
                        self.live.get() && is_virtual,
                    )?;
                    element.into()
                }
            };
            if let Some(parent) = piece.parent.and_then(|part| parts.get(&part)) {
                parent.append_child(&node).map_err(backend)?;
            }
            parts.insert(piece.part, node);
        }
        if let Some(fragment) = &fragment {
            fragment
                .append_child(&self.document.create_comment(&format!("gly:s:{key}")))
                .map_err(backend)?;
            fragment
                .append_child(&self.document.create_comment(&format!("gly:e:{key}")))
                .map_err(backend)?;
        }
        let entry = self.make_entry(&shape, parts, fragment)?;
        if let Some(glory_html::display::Placement::Root(frame)) = &shape.display {
            let display = super::display::Instance::create(
                &self.document,
                frame.clone(),
                key,
                &entry
                    .first()
                    .parent_node()
                    .ok_or(Error::InvalidPlacement("display range"))?,
                Some(entry.last()),
            )?;
            display.bind(self, id)?;
            *entry.display.borrow_mut() = Some(display);
        }
        if entry.edit == EditKind::Rich {
            self.rich_live.borrow_mut().insert(id);
        }
        self.entries.borrow_mut().insert(id, entry);
        Ok(())
    }
    fn make_entry(
        &self,
        shape: &Shape,
        parts: Parts,
        fragment: Option<DocumentFragment>,
    ) -> Result<Rc<Entry>, Error> {
        let boundary = if shape.pieces.is_empty() {
            let range = fragment.as_ref().ok_or(Error::Unsupported("Group range"))?;
            Boundary::Range(
                range
                    .first_child()
                    .ok_or(Error::Unsupported("Group start"))?,
                range.last_child().ok_or(Error::Unsupported("Group end"))?,
            )
        } else {
            let root = shape.root_part()?;
            if !parts.contains(root) {
                return Err(Error::Unsupported("HTML physical root"));
            }
            Boundary::Part(root)
        };
        let entry = Entry::new(shape, parts, boundary, None, fragment, false);
        if let Some(options) = &shape.select_options {
            let control = entry.control()?;
            let keys = shape
                .pieces
                .iter()
                .find_map(|piece| match &piece.kind {
                    Kind::Element { properties, .. } => match properties.get("selectedKeys") {
                        Some(Property::SelectedKeys(keys)) => Some(keys.as_slice()),
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap_or_default();
            super::select::create(&self.document, &control, options, keys)?;
        }
        for piece in &shape.pieces {
            if let Kind::Element {
                attributes,
                properties,
                ..
            } = &piece.kind
            {
                for (name, value) in properties {
                    // On a fresh native control, disabled is reflected by its
                    // attribute already. Patches/adoption retain their property
                    // barriers; never skip value/checked/composition writes.
                    if *name == "disabled"
                        && matches!(value, Property::Bool(disabled) if *disabled == attributes.contains_key("disabled"))
                    {
                        continue;
                    }
                    entry.property(piece.part, name, value)?;
                }
            }
        }
        Ok(entry)
    }
    fn place(
        &self,
        id: NodeId,
        parent: Option<NodeId>,
        before: Option<NodeId>,
    ) -> Result<(), Error> {
        let entry = self.entry(id)?;
        if self.place_detached_plan(&entry, parent, before)? {
            entry.parent.set(parent);
            return Ok(());
        }
        let semantic_parent = parent;
        let (parent, end) = if let Some(parent) = parent {
            let parent = self.entry(parent)?;
            if let Some(glory_html::display::Placement::Outlet { frame, slot }) = &entry.placement {
                let display = parent.display.borrow();
                let display = display
                    .as_ref()
                    .ok_or(Error::InvalidPlacement("display parent"))?;
                if !Rc::ptr_eq(frame, &display.frame) {
                    return Err(Error::InvalidPlacement("foreign display outlet"));
                }
                display.destination(*slot)?
            } else if let Some(part) = parent.children {
                (parent.part(part)?, None)
            } else if parent.parts.is_empty() {
                (
                    parent
                        .last()
                        .parent_node()
                        .ok_or(Error::InvalidPlacement("detached Group end"))?,
                    Some(parent.last().clone()),
                )
            } else {
                return Err(Error::InvalidProps("node has no HTML child slot"));
            }
        } else {
            (self.root.clone().into(), self.root_end.borrow().clone())
        };
        let before = if let Some(id) = before {
            Some(self.entry(id)?.first().clone())
        } else {
            end
        };
        // A detached new/resumed range cannot own the document's focus. Avoid
        // selection property reads and moveBefore reflection for these inserts.
        let connected = entry.first().is_connected();
        let selection = connected.then(|| Selection::capture(&self.document));
        // moveBefore keeps focus, IME and element state on supporting engines.
        let mover = (connected && parent.is_connected())
            .then(|| {
                js_sys::Reflect::get(&parent, &"moveBefore".into())
                    .ok()
                    .and_then(|value| value.dyn_into::<js_sys::Function>().ok())
            })
            .flatten();
        let place = |node: &Node| -> Result<(), Error> {
            let used = mover.as_ref().is_some_and(|mover| {
                mover
                    .call2(
                        &parent,
                        node,
                        before.as_ref().map_or(&JsValue::NULL, |node| node.as_ref()),
                    )
                    .is_ok()
            });
            if !used {
                parent
                    .insert_before(node, before.as_ref())
                    .map_err(backend)?;
            }
            Ok(())
        };
        if let Boundary::Part(_) = entry.boundary {
            place(entry.first())?;
        } else {
            for node in range(&entry)? {
                place(&node)?;
            }
        }
        if let Some(selection) = selection {
            selection.restore(&self.document);
        }
        entry.parent.set(semantic_parent);
        Ok(())
    }
    fn patch(&self, id: NodeId, edits: Vec<Edit>) -> Result<(), Error> {
        let entry = self.entry(id)?;
        let mut edits = edits.into_iter();
        loop {
            if let Some((instance, member)) = &entry.parts.planned {
                let consumed = instance.fields(*member, edits.as_slice())?;
                if consumed != 0 {
                    edits.nth(consumed - 1);
                    continue;
                }
            }
            let Some(edit) = edits.next() else { break };
            match edit {
                Edit::Revision(revision) => {
                    entry.editing.borrow_mut().requested = revision;
                    if entry.interests.borrow().contains(&EventKind::ImageStatus) {
                        self.queue_image(id, &entry, true);
                    }
                }
                Edit::Text { part, value } => entry.node(part)?.set_node_value(Some(&value)),
                Edit::Property { part, name, value } => entry.property(part, name, &value)?,
                Edit::Attribute { part, name, value } => {
                    let element = entry.element_part(part)?;
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
        self.dispose_confirmed(id, subtree, false)
    }
    fn dispose_confirmed(
        &self,
        id: NodeId,
        subtree: Vec<NodeId>,
        confirmed: bool,
    ) -> Result<(), Error> {
        let entry = self.entry(id)?;
        // Remove lookup/queued state before native focus events from removal.
        let removed: Vec<_> = {
            let mut entries = self.entries.borrow_mut();
            subtree.iter().filter_map(|id| entries.remove(id)).collect()
        };
        // Membership is already represented by the generation-checked live
        // table after removal; no per-disposal HashSet is necessary.
        let entries = self.entries.borrow();
        self.rich_pending
            .borrow_mut()
            .retain(|id| entries.contains_key(id));
        for id in &subtree {
            self.rich_live.borrow_mut().remove(id);
        }
        if self
            .drag
            .borrow()
            .as_ref()
            .is_some_and(|drag| !entries.contains_key(&drag.from))
        {
            self.drag.borrow_mut().take();
        }
        // The mapper supplies the complete semantic subtree, including parked
        // descendants. Remove those keys directly: retaining the whole parent
        // table for each row makes bulk teardown repeatedly scan unrelated rows.
        {
            let mut sizes = self.sizes.borrow_mut();
            for id in &subtree {
                sizes.remove(id);
                self.images.borrow_mut().retire(*id);
                self.clipboards.borrow_mut().retire(*id);
                self.collapses.borrow_mut().retire(*id);
                self.translations.borrow_mut().retire(*id);
            }
        }
        self.layout
            .borrow_mut()
            .retain(|(id, _)| entries.contains_key(id));
        self.inputs.borrow_mut().retain(
            |input| !matches!(input, Input::Event { node, .. } if !entries.contains_key(node)),
        );
        drop(entries);
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
        let remove = |node: &Node| -> Result<(), Error> {
            if let Some(parent) = node.parent_node() {
                parent.remove_child(node).map_err(backend)?;
            }
            Ok(())
        };
        let result = if confirmed {
            Ok(())
        } else if let Boundary::Part(part) = entry.boundary {
            if let Some(target) = entry.parts.target(part) {
                target.remove()
            } else {
                remove(entry.first())
            }
        } else {
            range(&entry).and_then(|nodes| nodes.iter().try_for_each(remove))
        };
        // Mark the whole semantic retirement before clearing any shared Array.
        // Full instances need no clearing bridge; partial instances clear their
        // retired slots once, even while unrelated members remain mounted.
        for entry in &removed {
            entry.parts.retire();
        }
        let retired = removed
            .iter()
            .try_for_each(|entry| entry.parts.flush_retired());
        result.and(retired)
    }
    pub fn clear(&self) {
        self.rich_pending.borrow_mut().clear();
        self.rich_live.borrow_mut().clear();
        self.swipe.borrow_mut().take();
        self.close_files();
        *self.collapses.borrow_mut() = super::collapse::Collapses::default();
        *self.translations.borrow_mut() = super::translation::Translations::default();
        *self.sheets.borrow_mut() = super::sheet::Sheets::default();
        if self.cleared.replace(true) {
            return;
        }
        if let Some(observer) = self.observer.borrow_mut().take() {
            observer.disconnect();
        }
        if let Some(observer) = self.viewport_observer.borrow_mut().take() {
            observer.disconnect();
        }
        super::layers::clear(self);
        let entries = std::mem::take(&mut *self.entries.borrow_mut());
        self.templates.clear();
        self.plans.clear();
        self.drag.borrow_mut().take();
        self.suppress_click.set(false);
        self.layout.borrow_mut().clear();
        self.sizes.borrow_mut().clear();
        self.inputs.borrow_mut().clear();
        self.images.borrow_mut().clear();
        self.clipboards.borrow_mut().clear();
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
        // Clearing this root may physically disconnect another independently
        // owned App. Its detached top layers must release document leases now.
        let _ = self.document_layers.sync(None);
    }
}
pub(super) struct Sink(pub Rc<Dom>);
impl glory_html::Sink for Sink {
    fn create_plan(
        &mut self,
        plan: &glory_render::Plan,
        ids: &[NodeId],
        keys: &[u64],
        shape: &glory_html::PlanShape,
    ) -> Result<(), Error> {
        if self.0.staging.get() {
            return Ok(());
        }
        self.0.create_plan(plan, ids, keys, shape)
    }

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
                let mut parking = entry.fragment.borrow_mut();
                let fragment =
                    parking.get_or_insert_with(|| self.0.document.create_document_fragment());
                if let Boundary::Part(_) = entry.boundary {
                    fragment.append_child(entry.first()).map_err(backend)?;
                } else {
                    for node in range(&entry)? {
                        fragment.append_child(&node).map_err(backend)?;
                    }
                }
                Ok(())
            }
        }
    }
}
fn virtual_shape(shape: &Shape) -> bool {
    shape.pieces.iter().any(|piece| {
        matches!(&piece.kind, Kind::Element { attributes, .. } if attributes.contains_key("data-gly-virtual"))
    })
}
fn range(entry: &Entry) -> Result<Vec<Node>, Error> {
    if let Boundary::Part(_) = entry.boundary {
        return Ok(vec![entry.first().clone()]);
    }
    let mut nodes = Vec::new();
    let mut next = Some(entry.first().clone());
    while let Some(node) = next {
        next = node.next_sibling();
        let last = node.is_same_node(Some(entry.last()));
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
    let options = web_sys::FocusOptions::new();
    options.set_prevent_scroll(true);
    if let Some(element) = element.dyn_ref::<HtmlElement>() {
        element.focus_with_options(&options).map_err(backend)
    } else if let Some(element) = element.dyn_ref::<web_sys::SvgElement>() {
        element.focus_with_options(&options).map_err(backend)
    } else {
        Err(Error::Unsupported("focus on this element"))
    }
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
