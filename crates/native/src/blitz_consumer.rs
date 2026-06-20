//! Applies Glory command batches to a blitz-dom document.
//!
//! Mapping notes (vs the reference semantics in
//! `glory_core::renderer::command_dom`):
//! - Glory node ids are u64 allocated by the producer; blitz node ids are
//!   `usize` allocated by the document — [`BlitzConsumer`] keeps the
//!   translation map.
//! - `SetText` maps to `DocumentMutator::set_node_text` (clears children,
//!   browser `textContent` semantics — matching the reference interpreter).
//! - `AddClass`/`RemoveClass` are reconciled into the `class` attribute
//!   (blitz has no classList API).
//! - `SetHtml` would need an HTML parser pass (blitz-html); rejected for
//!   now — widgets on the native backend should compose nodes instead.
//! - Events (`AttachEvent`/`DetachEvent`) are recorded. With the `shell`
//!   feature, `GloryBlitzDocument` forwards matching Blitz DOM events back
//!   into the held Glory `CommandHolder` (NT2). The DOM→Glory translation and
//!   listener routing are pure functions ([`map_dom_event`] /
//!   [`resolve_event_target`]) so they can be unit-tested headlessly; the
//!   `EventDriver` wiring that feeds them only runs under a live winit window.
//!
//! Text input / IME (NT3) — Blitz alpha limits:
//! - Blitz drives form state from a small set of *attributes*: writing the
//!   `value` attribute calls `TextInputData::set_text` (parley editor) and
//!   writing `checked` on an `<input>` toggles `CheckboxInput` state. So
//!   `SetProperty { value | checked }` is mirrored onto those attributes via
//!   [`BlitzConsumer::sync_reflected_property`], routing them into the form
//!   lifecycle rather than leaving them as inert properties.
//! - `checked` is only honoured by blitz-dom once the node `is_in_document()`
//!   (see `DocumentMutator::set_attribute`); a property set *before* insertion
//!   would otherwise be dropped. [`BlitzConsumer`] therefore re-applies tracked
//!   reflected properties when a node is inserted.
//! - IME `preedit`/`commit` and key composition are surfaced as `EventData`
//!   (see [`ime_event_extra`]), so the app layer can drive its own model.
//! - Full selection / caret sync is **not reachable** in this alpha: the
//!   parley `PlainEditor` selection lives behind `TextInputData::editor` with
//!   no consumer-facing setter on the mutator, so we cannot push a selection
//!   range into blitz. We map value/checked correctly and forward IME events;
//!   selection round-tripping waits on a richer Blitz form API.
//!
//! Layout query incrementality (NT4):
//! - blitz-dom exposes no public "layout dirty" signal, so the consumer keeps
//!   its own [`LayoutCache`]: tree/style mutations mark it dirty and a
//!   `BoundingRect` query only calls `doc.resolve` when dirty, reusing the last
//!   layout otherwise.

use std::collections::{BTreeMap, BTreeSet, HashMap};
#[cfg(feature = "shell")]
use std::{
    cell::RefCell,
    ops::{Deref, DerefMut},
    rc::Rc,
};

use blitz_dom::{Attribute, BaseDocument, DocumentConfig, QualName};
use glory_core::renderer::{Command, CommandInsertPosition, NodeQuery, QueryError, QueryResponse, QueryValue};
#[cfg(feature = "shell")]
use glory_core::renderer::{EventData, KeyboardData, PointerData, TargetData};
#[cfg(feature = "shell")]
use glory_core::web::holders::CommandHolder;
#[cfg(feature = "shell")]
use glory_core::{Holder, Widget};

#[cfg(feature = "shell")]
pub type GloryBlitzWindowId = winit::window::WindowId;

pub struct BlitzConsumer {
    doc: BaseDocument,
    /// Glory command-stream id → blitz node id.
    ids: HashMap<u64, usize>,
    /// Blitz node id → Glory command-stream id.
    blitz_to_glory: HashMap<usize, u64>,
    /// Class sets per glory id (blitz exposes only whole attributes).
    classes: HashMap<u64, BTreeSet<String>>,
    /// DOM properties per glory id, separate from attributes.
    properties: HashMap<u64, BTreeMap<String, String>>,
    /// (glory id, event name) listeners — recorded for the event stage.
    listeners: BTreeSet<(u64, String)>,
    /// Answers produced for `Command::Query`, drained by native hosts and
    /// fed back into the holder that issued the request.
    query_responses: Vec<QueryResponse>,
    /// Tracks whether the blitz layout is stale relative to applied commands,
    /// so layout queries only `doc.resolve` when something actually changed.
    layout: LayoutCache,
}

/// Pure "is the layout stale?" state machine (NT4).
///
/// blitz-dom has no public dirty flag, so the consumer owns this: any command
/// that mutates the tree or styling marks the cache dirty, and a layout query
/// only triggers a (costly) `doc.resolve` when [`LayoutCache::needs_resolve`]
/// reports stale state. Kept dependency-free for straightforward unit testing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutCache {
    dirty: bool,
}

impl Default for LayoutCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutCache {
    /// A fresh cache is dirty: nothing has been resolved yet.
    pub const fn new() -> Self {
        Self { dirty: true }
    }

    /// Record that the tree/style changed; the next query must re-resolve.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Whether a layout query must call `doc.resolve` before reading geometry.
    pub const fn needs_resolve(&self) -> bool {
        self.dirty
    }

    /// Record that layout has just been resolved and is now up to date.
    pub fn mark_resolved(&mut self) {
        self.dirty = false;
    }
}

fn qual(name: &str) -> QualName {
    QualName::new(
        None,
        blitz_dom::Namespace::from("http://www.w3.org/1999/xhtml"),
        blitz_dom::LocalName::from(name),
    )
}

impl Default for BlitzConsumer {
    fn default() -> Self {
        Self::new()
    }
}

impl BlitzConsumer {
    pub fn new() -> Self {
        let mut doc = BaseDocument::new(DocumentConfig {
            // Stylo's `Parallel` traversal shares a global rayon pool; two
            // documents resolving concurrently on different threads can panic
            // with "already mutably borrowed" (see blitz `StyleThreading`).
            // A native consumer may resolve from a host thread (layout queries)
            // while another window resolves elsewhere, so use the sequential
            // traversal which is safe to run from many threads at once. This
            // also keeps the layout-query unit tests deterministic when the
            // test harness resolves several documents in parallel.
            style_threading: blitz_dom::StyleThreading::Sequential,
            ..Default::default()
        });
        // A fresh BaseDocument is just the document node (slab id 0);
        // give it the html/body skeleton and host the app in <body>.
        let body = {
            let mut mutator = doc.mutate();
            let html = mutator.create_element(qual("html"), Vec::new());
            let body = mutator.create_element(qual("body"), Vec::new());
            mutator.append_children(0, &[html]);
            mutator.append_children(html, &[body]);
            body
        };
        let mut ids = HashMap::new();
        // Reserved host root (glory id 0) → <body>.
        ids.insert(0, body);
        let mut blitz_to_glory = HashMap::new();
        blitz_to_glory.insert(body, 0);
        Self {
            doc,
            ids,
            blitz_to_glory,
            classes: HashMap::new(),
            properties: HashMap::new(),
            listeners: BTreeSet::new(),
            query_responses: Vec::new(),
            layout: LayoutCache::new(),
        }
    }

    pub fn document(&self) -> &BaseDocument {
        &self.doc
    }

    /// Mutable access to the underlying blitz document.
    ///
    /// Needed by callers that drive the layout/paint pipeline directly —
    /// `doc.set_viewport(...)` / `doc.resolve(...)` and `blitz_paint::paint_scene`
    /// all take `&mut BaseDocument` (NT6 screenshot regression).
    pub fn document_mut(&mut self) -> &mut BaseDocument {
        &mut self.doc
    }

    /// Install Glory's image/sub-resource [`NetProvider`](blitz_traits::net::NetProvider)
    /// (NT5) and point the document's `base_url` at `assets_root`, so relative
    /// and `glory://` `<img src>` values resolve into the asset tree.
    ///
    /// `assets_root` is a filesystem directory. The base URL is built with the
    /// pure [`resolve_asset_url`](crate::resolve_asset_url) helper (a `file://`
    /// URL), and the provider reads local files / decodes `data:` URIs;
    /// `http(s)` images need a host-supplied provider (see the module docs).
    pub fn set_assets_root(&mut self, assets_root: impl AsRef<std::path::Path>) {
        let assets_root = assets_root.as_ref();
        // Build a directory `file://` base URL (trailing slash) so Blitz's
        // relative-URL resolution treats the root as a directory, not a file.
        let mut base = crate::resolve_asset_url("", assets_root);
        if !base.ends_with('/') {
            base.push('/');
        }
        self.doc.set_base_url(&base);
        self.doc
            .set_net_provider(crate::GloryImageProvider::new().with_assets_root(assets_root).shared());
    }

    /// Current layout-cache state (NT4 inspection / test aid).
    pub fn layout_cache(&self) -> LayoutCache {
        self.layout
    }

    pub fn listeners(&self) -> &BTreeSet<(u64, String)> {
        &self.listeners
    }

    pub fn take_query_responses(&mut self) -> Vec<QueryResponse> {
        std::mem::take(&mut self.query_responses)
    }

    pub fn glory_id_for_blitz(&self, blitz_id: usize) -> Option<u64> {
        self.blitz_to_glory.get(&blitz_id).copied()
    }

    fn blitz_id(&self, glory_id: u64) -> Option<usize> {
        self.ids.get(&glory_id).copied()
    }

    pub fn apply_batch(&mut self, batch: &[Command]) {
        for command in batch {
            self.apply(command);
        }
    }

    pub fn apply(&mut self, command: &Command) {
        match command {
            Command::Create { id, name, .. } => {
                let node_id = self.doc.mutate().create_element(qual(name), Vec::new());
                self.ids.insert(*id, node_id);
                self.blitz_to_glory.insert(node_id, *id);
                self.layout.mark_dirty();
            }
            Command::SetAttribute { id, name, value } => {
                if let Some(node) = self.blitz_id(*id) {
                    self.doc.mutate().set_attribute(node, qual(name), value);
                    self.layout.mark_dirty();
                }
            }
            Command::RemoveAttribute { id, name } => {
                if let Some(node) = self.blitz_id(*id) {
                    self.doc.mutate().clear_attribute(node, qual(name));
                    self.layout.mark_dirty();
                }
            }
            Command::SetProperty { id, name, value } => {
                self.properties.entry(*id).or_default().insert(name.clone(), value.clone());
                self.sync_reflected_property(*id, name, Some(value));
            }
            Command::RemoveProperty { id, name } => {
                if let Some(properties) = self.properties.get_mut(id) {
                    properties.remove(name);
                    if properties.is_empty() {
                        self.properties.remove(id);
                    }
                }
                self.sync_reflected_property(*id, name, None);
            }
            Command::AddClass { id, value } => {
                let classes = self.classes.entry(*id).or_default();
                for part in value.split_whitespace() {
                    classes.insert(part.to_owned());
                }
                self.sync_classes(*id);
            }
            Command::RemoveClass { id, value } => {
                let classes = self.classes.entry(*id).or_default();
                for part in value.split_whitespace() {
                    classes.remove(part);
                }
                self.sync_classes(*id);
            }
            Command::SetText { id, value } => {
                if let Some(node) = self.blitz_id(*id) {
                    // textContent semantics: replace children with one text
                    // node (blitz's set_node_text only edits text nodes).
                    let mut mutator = self.doc.mutate();
                    mutator.remove_and_drop_all_children(node);
                    let text = mutator.create_text_node(value);
                    mutator.append_children(node, &[text]);
                    self.layout.mark_dirty();
                }
            }
            Command::SetHtml { id, .. } => {
                let _ = id;
                // SetHtml requires an HTML parser pass (blitz-html); the
                // native backend composes nodes instead.
            }
            Command::Insert { parent, child, position } => {
                let (Some(parent), Some(child_blitz), child_glory) = (self.blitz_id(*parent), self.blitz_id(*child), *child) else {
                    return;
                };
                let child = child_blitz;
                {
                    let mut mutator = self.doc.mutate();
                    match position {
                        CommandInsertPosition::Tail => mutator.append_children(parent, &[child]),
                        CommandInsertPosition::Head => match mutator.child_ids(parent).first().copied() {
                            Some(first) => mutator.insert_nodes_before(first, &[child]),
                            None => mutator.append_children(parent, &[child]),
                        },
                        CommandInsertPosition::Before(anchor) => {
                            drop(mutator);
                            match self.blitz_id(*anchor) {
                                Some(anchor) => self.doc.mutate().insert_nodes_before(anchor, &[child]),
                                None => self.doc.mutate().append_children(parent, &[child]),
                            }
                        }
                        CommandInsertPosition::After(anchor) => {
                            drop(mutator);
                            match self.blitz_id(*anchor) {
                                Some(anchor) => self.doc.mutate().insert_nodes_after(anchor, &[child]),
                                None => self.doc.mutate().append_children(parent, &[child]),
                            }
                        }
                    }
                }
                // blitz-dom only applies `checked` (and finalises form state)
                // once a node `is_in_document()`. A property set before this
                // insertion would have been a no-op, so re-route any tracked
                // reflected properties now that the node is attached.
                self.reapply_reflected_properties(child_glory);
                self.layout.mark_dirty();
            }
            Command::Remove { child, .. } => {
                if let Some(blitz_child) = self.blitz_id(*child) {
                    self.doc.mutate().remove_node(blitz_child);
                    self.ids.remove(child);
                    self.blitz_to_glory.remove(&blitz_child);
                    self.classes.remove(child);
                    self.properties.remove(child);
                    self.layout.mark_dirty();
                }
            }
            Command::AttachEvent { id, name, .. } => {
                self.listeners.insert((*id, name.clone()));
            }
            Command::DetachEvent { id, name } => {
                self.listeners.remove(&(*id, name.clone()));
            }
            Command::Query { id, token, kind } => {
                let result = self.answer_query(*id, *kind);
                self.query_responses.push(QueryResponse { token: *token, result });
            }
        }
    }

    fn answer_query(&mut self, glory_id: u64, kind: NodeQuery) -> Result<QueryValue, QueryError> {
        let blitz_id = self.blitz_id(glory_id).ok_or(QueryError::NodeGone)?;
        if self.doc.get_node(blitz_id).is_none() {
            return Err(QueryError::NodeGone);
        }

        match kind {
            NodeQuery::Value => Ok(QueryValue::Value(
                self.properties
                    .get(&glory_id)
                    .and_then(|properties| properties.get("value"))
                    .cloned()
                    .unwrap_or_default(),
            )),
            NodeQuery::BoundingRect => {
                // NT4: only re-run the (costly) layout pass when a command has
                // marked the tree/style dirty; otherwise reuse the last layout.
                if self.layout.needs_resolve() {
                    self.doc.resolve(0.0);
                    self.layout.mark_resolved();
                }
                let node = self.doc.get_node(blitz_id).ok_or(QueryError::NodeGone)?;
                let position = node.absolute_position(0.0, 0.0);
                Ok(QueryValue::Rect {
                    x: position.x as f64,
                    y: position.y as f64,
                    width: node.final_layout.size.width as f64,
                    height: node.final_layout.size.height as f64,
                })
            }
            NodeQuery::ScrollOffset => {
                let node = self.doc.get_node(blitz_id).ok_or(QueryError::NodeGone)?;
                Ok(QueryValue::ScrollOffset {
                    x: node.scroll_offset.x,
                    y: node.scroll_offset.y,
                })
            }
        }
    }

    fn sync_classes(&mut self, glory_id: u64) {
        let value = self
            .classes
            .get(&glory_id)
            .map(|set| set.iter().cloned().collect::<Vec<_>>().join(" "))
            .unwrap_or_default();
        if let Some(node) = self.blitz_id(glory_id) {
            self.doc.mutate().set_attribute(node, qual("class"), &value);
            self.layout.mark_dirty();
        }
    }

    /// Route a known form property (`value`/`checked`) into blitz form state.
    ///
    /// blitz-dom finalises form state through these *attributes*: writing
    /// `value` updates the parley text editor and writing `checked` toggles the
    /// checkbox. Other properties stay property-only (tracked in `properties`).
    fn sync_reflected_property(&mut self, glory_id: u64, name: &str, value: Option<&String>) {
        if !is_reflected_blitz_property(name) {
            return;
        }

        if let Some(node) = self.blitz_id(glory_id) {
            match value {
                Some(value) => self.doc.mutate().set_attribute(node, qual(name), value),
                None => self.doc.mutate().clear_attribute(node, qual(name)),
            }
            self.layout.mark_dirty();
        }
    }

    /// Re-route all tracked reflected properties for a node into blitz form
    /// state. Used right after insertion, because blitz only honours `checked`
    /// once the node `is_in_document()`.
    fn reapply_reflected_properties(&mut self, glory_id: u64) {
        let Some(reflected) = self.properties.get(&glory_id).map(|props| {
            props
                .iter()
                .filter(|(name, _)| is_reflected_blitz_property(name))
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect::<Vec<_>>()
        }) else {
            return;
        };
        for (name, value) in reflected {
            self.sync_reflected_property(glory_id, &name, Some(&value));
        }
    }

    /// Tag names of `glory_id`'s children, in order (test/inspection aid).
    pub fn child_tags(&mut self, glory_id: u64) -> Vec<String> {
        let Some(node) = self.blitz_id(glory_id) else { return Vec::new() };
        let child_ids = self.doc.mutate().child_ids(node);
        let mutator = self.doc.mutate();
        child_ids
            .iter()
            .filter_map(|id| mutator.element_name(*id).map(|name| name.local.to_string()))
            .collect()
    }

    /// Attribute value lookup by glory id (test/inspection aid).
    pub fn attribute(&self, glory_id: u64, name: &str) -> Option<String> {
        let node = self.blitz_id(glory_id)?;
        let node = self.doc.get_node(node)?;
        let element = node.element_data()?;
        element
            .attrs
            .iter()
            .find(|attr| attr.name.local.as_ref() == name)
            .map(|attr| attr.value.clone())
    }

    /// Property value lookup by glory id (test/inspection aid).
    pub fn property(&self, glory_id: u64, name: &str) -> Option<String> {
        self.properties.get(&glory_id)?.get(name).cloned()
    }

    #[allow(dead_code)]
    fn unused(_: Attribute) {}
}

/// How a `SetProperty`/`RemoveProperty` is routed on the blitz backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyRoute {
    /// Mirrored onto a blitz form attribute (drives the form-state lifecycle)
    /// while still being tracked as a property.
    FormAttribute,
    /// Tracked as a plain property only; not reflected into blitz state.
    PropertyOnly,
}

/// Pure classifier for property routing (NT3) — easy to unit test in isolation.
pub fn property_route(name: &str) -> PropertyRoute {
    if is_reflected_blitz_property(name) {
        PropertyRoute::FormAttribute
    } else {
        PropertyRoute::PropertyOnly
    }
}

/// Form properties blitz-dom understands as attributes: `value` feeds the
/// parley text editor (text input / IME committed text) and `checked` toggles
/// checkbox state. These are mirrored onto attributes so they enter the form
/// lifecycle instead of being inert properties.
fn is_reflected_blitz_property(name: &str) -> bool {
    matches!(name, "value" | "checked")
}

#[cfg(feature = "shell")]
#[derive(Clone, Debug, PartialEq)]
pub struct GloryBlitzWindowConfig {
    pub title: String,
    pub inner_size: Option<(f64, f64)>,
}

#[cfg(feature = "shell")]
impl Default for GloryBlitzWindowConfig {
    fn default() -> Self {
        Self {
            title: "Glory Native".to_owned(),
            inner_size: None,
        }
    }
}

#[cfg(feature = "shell")]
impl GloryBlitzWindowConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn inner_size(mut self, width: f64, height: f64) -> Self {
        self.inner_size = Some((width, height));
        self
    }
}

#[cfg(feature = "shell")]
pub struct GloryBlitzApplication {
    pending_windows: Vec<blitz_shell::WindowConfig<anyrender_vello::VelloWindowRenderer>>,
}

#[cfg(feature = "shell")]
impl Default for GloryBlitzApplication {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "shell")]
impl GloryBlitzApplication {
    pub fn new() -> Self {
        Self { pending_windows: Vec::new() }
    }

    pub fn window<W>(mut self, config: GloryBlitzWindowConfig, widget: W) -> Self
    where
        W: Widget + 'static,
    {
        self.add_window(config, widget);
        self
    }

    pub fn add_window<W>(&mut self, config: GloryBlitzWindowConfig, widget: W)
    where
        W: Widget + 'static,
    {
        self.pending_windows.push(create_blitz_window_config(config, widget));
    }

    pub fn pending_window_count(&self) -> usize {
        self.pending_windows.len()
    }

    pub fn into_blitz_application(
        self,
        proxy: blitz_shell::BlitzShellProxy,
        event_queue: std::sync::mpsc::Receiver<blitz_shell::BlitzShellEvent>,
    ) -> blitz_shell::BlitzApplication<anyrender_vello::VelloWindowRenderer> {
        let mut application = blitz_shell::BlitzApplication::new(proxy, event_queue);
        for window in self.pending_windows {
            application.add_window(window);
        }
        application
    }

    pub fn run(self) -> Result<(), winit::error::EventLoopError> {
        let event_loop = blitz_shell::create_default_event_loop();
        let (proxy, event_queue) = blitz_shell::BlitzShellProxy::new(event_loop.create_proxy());
        let application = self.into_blitz_application(proxy, event_queue);
        event_loop.run_app(application)
    }
}

#[cfg(feature = "shell")]
fn create_blitz_window_config(
    config: GloryBlitzWindowConfig,
    widget: impl Widget + 'static,
) -> blitz_shell::WindowConfig<anyrender_vello::VelloWindowRenderer> {
    let holder = CommandHolder::new().mount(widget);
    let mut consumer = BlitzConsumer::new();
    flush_holder_into_consumer(&holder, &mut consumer);

    let renderer = anyrender_vello::VelloWindowRenderer::new();
    let mut attributes = winit::window::WindowAttributes::default().with_title(config.title);
    if let Some((width, height)) = config.inner_size {
        attributes = attributes.with_surface_size(winit::dpi::LogicalSize::new(width, height));
    }
    blitz_shell::WindowConfig::with_attributes(Box::new(GloryBlitzDocument { holder, consumer }), renderer, attributes)
}

#[cfg(feature = "shell")]
pub fn launch_blitz_window(widget: impl Widget + 'static) {
    launch_blitz_window_with_config(GloryBlitzWindowConfig::default(), widget);
}

#[cfg(feature = "shell")]
pub fn launch_blitz_window_with_config(config: GloryBlitzWindowConfig, widget: impl Widget + 'static) {
    GloryBlitzApplication::new()
        .window(config, widget)
        .run()
        .expect("run Glory Blitz event loop");
}

#[cfg(feature = "shell")]
struct GloryBlitzDocument {
    holder: CommandHolder,
    consumer: BlitzConsumer,
}

#[cfg(feature = "shell")]
impl Deref for GloryBlitzDocument {
    type Target = BaseDocument;

    fn deref(&self) -> &Self::Target {
        &self.consumer.doc
    }
}

#[cfg(feature = "shell")]
impl DerefMut for GloryBlitzDocument {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.consumer.doc
    }
}

#[cfg(feature = "shell")]
impl blitz_dom::Document for GloryBlitzDocument {
    fn inner(&self) -> blitz_dom::DocGuard<'_> {
        blitz_dom::DocGuard::Ref(&self.consumer.doc)
    }

    fn inner_mut(&mut self) -> blitz_dom::DocGuardMut<'_> {
        blitz_dom::DocGuardMut::Ref(&mut self.consumer.doc)
    }

    fn handle_ui_event(&mut self, event: blitz_traits::events::UiEvent) {
        let events = Rc::new(RefCell::new(Vec::new()));
        let handler = GloryEventBridge {
            blitz_to_glory: self.consumer.blitz_to_glory.clone(),
            listeners: self.consumer.listeners.clone(),
            events: events.clone(),
        };
        {
            let mut driver = blitz_dom::EventDriver::new(self, handler);
            driver.handle_ui_event(event);
        }

        let mut changed = false;
        for event in events.borrow_mut().drain(..) {
            if self.holder.dispatch_event(event) {
                changed |= flush_holder_into_consumer(&self.holder, &mut self.consumer);
            }
        }
        if changed {
            self.consumer.doc.shell_provider.request_redraw();
        }
    }
}

#[cfg(feature = "shell")]
fn flush_holder_into_consumer(holder: &CommandHolder, consumer: &mut BlitzConsumer) -> bool {
    let mut changed = false;
    loop {
        let batch = holder.take_batch();
        if batch.is_empty() {
            return changed;
        }
        consumer.apply_batch(&batch);
        changed = true;
        for response in consumer.take_query_responses() {
            changed |= holder.resolve_query(response);
        }
    }
}

#[cfg(feature = "shell")]
struct GloryEventBridge {
    blitz_to_glory: HashMap<usize, u64>,
    listeners: BTreeSet<(u64, String)>,
    events: Rc<RefCell<Vec<EventData>>>,
}

#[cfg(feature = "shell")]
impl blitz_dom::EventHandler for GloryEventBridge {
    fn handle_event(
        &mut self,
        chain: &[usize],
        event: &mut blitz_traits::events::DomEvent,
        _doc: &mut dyn blitz_dom::Document,
        _event_state: &mut blitz_traits::events::EventState,
    ) {
        let name = event.name();
        let Some(glory_id) = resolve_event_target(chain, &self.blitz_to_glory, &self.listeners, name) else {
            return;
        };

        let data = map_dom_event(name, glory_id, &event.data);
        self.events.borrow_mut().push(data);
    }
}

/// Pure listener resolution (NT2): walk the bubble `chain` (target → root) and
/// return the first glory node that has a registered listener for `name`.
///
/// Factored out of [`GloryEventBridge::handle_event`] so dispatch routing is
/// unit-testable without a Blitz `EventDriver` / live event loop.
#[cfg(feature = "shell")]
pub fn resolve_event_target(chain: &[usize], blitz_to_glory: &HashMap<usize, u64>, listeners: &BTreeSet<(u64, String)>, name: &str) -> Option<u64> {
    chain.iter().find_map(|blitz_id| {
        let glory_id = blitz_to_glory.get(blitz_id).copied()?;
        listeners.contains(&(glory_id, name.to_owned())).then_some(glory_id)
    })
}

/// Pure mapping from a Blitz [`DomEventData`](blitz_traits::events::DomEventData)
/// to Glory's [`EventData`] (NT2).
///
/// This is the heart of the native event bridge and is exercised by unit tests
/// across pointer / click / wheel / scroll / keyboard / input / focus / IME
/// families. Keeping it a free function (no document, no event loop) means the
/// translation can be verified headlessly; the surrounding plumbing that feeds
/// it from a real Blitz `EventDriver` is `cargo check`-covered and only runs
/// under a live winit window.
#[cfg(feature = "shell")]
pub fn map_dom_event(name: &str, glory_id: u64, data: &blitz_traits::events::DomEventData) -> EventData {
    use blitz_traits::events::DomEventData as D;

    let mut out = EventData::new(name, glory_id);
    match data {
        D::PointerMove(pointer)
        | D::PointerDown(pointer)
        | D::PointerUp(pointer)
        | D::PointerEnter(pointer)
        | D::PointerLeave(pointer)
        | D::PointerOver(pointer)
        | D::PointerOut(pointer)
        | D::MouseMove(pointer)
        | D::MouseDown(pointer)
        | D::MouseUp(pointer)
        | D::MouseEnter(pointer)
        | D::MouseLeave(pointer)
        | D::MouseOver(pointer)
        | D::MouseOut(pointer)
        | D::Click(pointer)
        | D::ContextMenu(pointer)
        | D::DoubleClick(pointer) => {
            out.pointer = Some(pointer_data(pointer));
            out.extra = Some(pointer_event_extra(pointer));
        }
        D::Wheel(wheel) => {
            out.pointer = Some(wheel_pointer_data(wheel));
            out.extra = Some(wheel_event_extra(wheel));
        }
        D::Scroll(scroll) => {
            out.extra = Some(scroll_event_extra(scroll));
        }
        D::Focus(_) | D::Blur(_) | D::FocusIn(_) | D::FocusOut(_) => {
            out.extra = Some(serde_json::json!({
                "focus": { "type": name }
            }));
        }
        D::Input(input) => {
            out.target = Some(TargetData {
                value: Some(input.value.clone()),
                checked: None,
            });
        }
        D::KeyPress(key) | D::KeyDown(key) | D::KeyUp(key) => {
            out.keyboard = Some(keyboard_data(key));
            out.extra = Some(serde_json::json!({
                "repeat": key.is_auto_repeating,
                "composing": key.is_composing,
                "location": format!("{:?}", key.location),
                "text": key.text.as_ref().map(ToString::to_string),
            }));
        }
        D::Ime(ime) => {
            out.extra = Some(ime_event_extra(ime));
        }
        D::AppleStandardKeybinding(binding) => {
            out.extra = Some(serde_json::json!({
                "apple_standard_keybinding": binding.to_string()
            }));
        }
    }
    out
}

#[cfg(feature = "shell")]
fn pointer_data(pointer: &blitz_traits::events::BlitzPointerEvent) -> PointerData {
    PointerData {
        client_x: pointer.client_x() as f64,
        client_y: pointer.client_y() as f64,
        button: pointer.button as i16,
        buttons: pointer.buttons.bits() as u16,
    }
}

#[cfg(feature = "shell")]
fn wheel_pointer_data(wheel: &blitz_traits::events::BlitzWheelEvent) -> PointerData {
    PointerData {
        client_x: wheel.client_x() as f64,
        client_y: wheel.client_y() as f64,
        button: 0,
        buttons: wheel.buttons.bits() as u16,
    }
}

#[cfg(feature = "shell")]
fn pointer_event_extra(pointer: &blitz_traits::events::BlitzPointerEvent) -> serde_json::Value {
    let pointer_type = match pointer.id {
        blitz_traits::events::BlitzPointerId::Mouse => "mouse".to_owned(),
        blitz_traits::events::BlitzPointerId::Pen => "pen".to_owned(),
        blitz_traits::events::BlitzPointerId::Finger(id) => format!("touch:{id}"),
    };
    serde_json::json!({
        "pointer": {
            "type": pointer_type,
            "primary": pointer.is_primary,
            "page_x": pointer.page_x(),
            "page_y": pointer.page_y(),
            "screen_x": pointer.screen_x(),
            "screen_y": pointer.screen_y(),
            "alt": pointer.mods.alt(),
            "ctrl": pointer.mods.ctrl(),
            "shift": pointer.mods.shift(),
            "meta": pointer.mods.meta(),
            "pressure": pointer.details.pressure,
            "tilt_x": pointer.details.tilt_x,
            "tilt_y": pointer.details.tilt_y,
            "twist": pointer.details.twist,
        }
    })
}

#[cfg(feature = "shell")]
fn wheel_event_extra(wheel: &blitz_traits::events::BlitzWheelEvent) -> serde_json::Value {
    let (delta_x, delta_y, delta_mode) = match wheel.delta {
        blitz_traits::events::BlitzWheelDelta::Lines(x, y) => (x, y, "line"),
        blitz_traits::events::BlitzWheelDelta::Pixels(x, y) => (x, y, "pixel"),
    };
    serde_json::json!({
        "wheel": {
            "delta_x": delta_x,
            "delta_y": delta_y,
            "delta_mode": delta_mode,
            "page_x": wheel.page_x(),
            "page_y": wheel.page_y(),
            "screen_x": wheel.screen_x(),
            "screen_y": wheel.screen_y(),
            "alt": wheel.mods.alt(),
            "ctrl": wheel.mods.ctrl(),
            "shift": wheel.mods.shift(),
            "meta": wheel.mods.meta(),
        }
    })
}

#[cfg(feature = "shell")]
fn scroll_event_extra(scroll: &blitz_traits::events::BlitzScrollEvent) -> serde_json::Value {
    serde_json::json!({
        "scroll": {
            "top": scroll.scroll_top,
            "left": scroll.scroll_left,
            "width": scroll.scroll_width,
            "height": scroll.scroll_height,
            "client_width": scroll.client_width,
            "client_height": scroll.client_height,
        }
    })
}

#[cfg(feature = "shell")]
fn keyboard_data(key: &blitz_traits::events::BlitzKeyEvent) -> KeyboardData {
    KeyboardData {
        key: key.key.to_string(),
        code: key.code.to_string(),
        alt: key.modifiers.alt(),
        ctrl: key.modifiers.ctrl(),
        shift: key.modifiers.shift(),
        meta: key.modifiers.meta(),
    }
}

#[cfg(feature = "shell")]
fn ime_event_extra(ime: &blitz_traits::events::BlitzImeEvent) -> serde_json::Value {
    match ime {
        blitz_traits::events::BlitzImeEvent::Enabled => serde_json::json!({
            "ime": { "state": "enabled" }
        }),
        blitz_traits::events::BlitzImeEvent::Preedit(value, cursor) => serde_json::json!({
            "ime": {
                "state": "preedit",
                "value": value,
                "cursor": cursor,
            }
        }),
        blitz_traits::events::BlitzImeEvent::Commit(value) => serde_json::json!({
            "ime": {
                "state": "commit",
                "value": value,
            }
        }),
        blitz_traits::events::BlitzImeEvent::DeleteSurrounding { before_bytes, after_bytes } => serde_json::json!({
            "ime": {
                "state": "delete_surrounding",
                "before_bytes": before_bytes,
                "after_bytes": after_bytes,
            }
        }),
        blitz_traits::events::BlitzImeEvent::Disabled => serde_json::json!({
            "ime": { "state": "disabled" }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glory_core::Holder;
    use glory_core::reflow::Cage;
    use glory_core::renderer::{NodeQuery, QueryError, QueryResponse, QueryValue};
    use glory_core::web::events;
    use glory_core::web::holders::CommandHolder;
    use glory_core::web::widgets::{button, div, span};
    use glory_core::{Scope, Widget};

    #[derive(Debug)]
    struct Counter {
        value: Cage<i64>,
    }

    impl Widget for Counter {
        fn build(&mut self, ctx: &mut Scope) {
            let value = self.value.clone();
            div()
                .attr("data-app", "counter")
                .fill(button().text("+").on(events::click, move |_| value.revise(|mut v| *v += 1)))
                .fill(span().text(self.value.clone()))
                .show_in(ctx);
        }
    }

    /// The actual spike: a real widget tree drives a real blitz document
    /// through nothing but the command stream, including a reactive patch.
    #[test]
    fn widget_tree_renders_into_blitz_document() {
        let value = Cage::new(0i64);
        let holder = CommandHolder::new().mount(Counter { value: value.clone() });

        let mut consumer = BlitzConsumer::new();
        consumer.apply_batch(&holder.take_batch());

        assert_eq!(consumer.child_tags(0), vec!["div"]);
        // div is glory id 1 (first created); its attribute survived.
        assert_eq!(consumer.attribute(1, "data-app").as_deref(), Some("counter"));
        assert_eq!(consumer.child_tags(1), vec!["button", "span"]);
        assert!(consumer.listeners().iter().any(|(_, name)| name == "click"));

        // Reactive update flows through as an incremental batch.
        holder.update(|| value.revise(|mut v| *v = 42));
        consumer.apply_batch(&holder.take_batch());

        // The span (id 3: div=1, button=2, span=3) now carries "42".
        let span_blitz_id = consumer.blitz_id(3).unwrap();
        let doc = consumer.document();
        let span_node = doc.get_node(span_blitz_id).unwrap();
        assert_eq!(span_node.text_content(), "42");
    }

    #[test]
    fn properties_are_tracked_separately_from_attributes() {
        let mut consumer = BlitzConsumer::new();
        consumer.apply_batch(&[
            Command::Create {
                id: 1,
                name: "input".into(),
                is_void: false,
            },
            Command::Insert {
                parent: 0,
                child: 1,
                position: CommandInsertPosition::Tail,
            },
            Command::SetAttribute {
                id: 1,
                name: "data-attr".into(),
                value: "attribute".into(),
            },
            Command::SetProperty {
                id: 1,
                name: "data-prop".into(),
                value: "property".into(),
            },
            Command::SetProperty {
                id: 1,
                name: "value".into(),
                value: "typed".into(),
            },
        ]);

        assert_eq!(consumer.attribute(1, "data-attr").as_deref(), Some("attribute"));
        assert_eq!(consumer.property(1, "data-prop").as_deref(), Some("property"));
        assert_eq!(consumer.attribute(1, "data-prop"), None);

        // Blitz's form state is still driven through attributes, so known
        // form properties are mirrored while remaining queryable as properties.
        assert_eq!(consumer.property(1, "value").as_deref(), Some("typed"));
        assert_eq!(consumer.attribute(1, "value").as_deref(), Some("typed"));

        consumer.apply(&Command::RemoveProperty {
            id: 1,
            name: "data-prop".into(),
        });
        assert_eq!(consumer.property(1, "data-prop"), None);
        assert_eq!(consumer.attribute(1, "data-prop"), None);
    }

    #[test]
    fn value_queries_answer_from_tracked_properties() {
        let mut consumer = BlitzConsumer::new();
        consumer.apply_batch(&[
            Command::Create {
                id: 1,
                name: "input".into(),
                is_void: false,
            },
            Command::SetAttribute {
                id: 1,
                name: "value".into(),
                value: "attribute".into(),
            },
            Command::SetProperty {
                id: 1,
                name: "value".into(),
                value: "typed".into(),
            },
            Command::Query {
                id: 1,
                token: 7,
                kind: NodeQuery::Value,
            },
        ]);

        assert_eq!(
            consumer.take_query_responses(),
            vec![QueryResponse {
                token: 7,
                result: Ok(QueryValue::Value("typed".into()))
            }]
        );
    }

    #[test]
    fn queries_report_missing_nodes() {
        let mut consumer = BlitzConsumer::new();
        consumer.apply(&Command::Query {
            id: 99,
            token: 3,
            kind: NodeQuery::Value,
        });

        assert_eq!(
            consumer.take_query_responses(),
            vec![QueryResponse {
                token: 3,
                result: Err(QueryError::NodeGone)
            }]
        );
    }

    #[test]
    fn layout_queries_use_blitz_layout_state() {
        let mut consumer = BlitzConsumer::new();
        consumer.apply_batch(&[
            Command::Create {
                id: 1,
                name: "div".into(),
                is_void: false,
            },
            Command::SetAttribute {
                id: 1,
                name: "style".into(),
                value: "width: 123px; height: 45px;".into(),
            },
            Command::Insert {
                parent: 0,
                child: 1,
                position: CommandInsertPosition::Tail,
            },
            Command::Query {
                id: 1,
                token: 1,
                kind: NodeQuery::BoundingRect,
            },
        ]);

        let rect = consumer.take_query_responses().remove(0);
        assert_eq!(rect.token, 1);
        match rect.result {
            Ok(QueryValue::Rect { width, height, .. }) => {
                assert_eq!(width, 123.0);
                assert_eq!(height, 45.0);
            }
            other => panic!("unexpected layout query response: {other:?}"),
        }
    }

    #[test]
    fn scroll_queries_use_blitz_scroll_state() {
        let mut consumer = BlitzConsumer::new();
        consumer.apply(&Command::Create {
            id: 1,
            name: "div".into(),
            is_void: false,
        });
        let blitz_id = consumer.blitz_id(1).unwrap();
        let node = consumer.doc.get_node_mut(blitz_id).unwrap();
        node.scroll_offset.x = 12.0;
        node.scroll_offset.y = 34.0;

        consumer.apply(&Command::Query {
            id: 1,
            token: 2,
            kind: NodeQuery::ScrollOffset,
        });

        assert_eq!(
            consumer.take_query_responses(),
            vec![QueryResponse {
                token: 2,
                result: Ok(QueryValue::ScrollOffset { x: 12.0, y: 34.0 })
            }]
        );
    }

    #[test]
    #[cfg(feature = "shell")]
    fn blitz_shell_application_tracks_pending_windows() {
        let config = GloryBlitzWindowConfig::new().title("Native Counter").inner_size(640.0, 480.0);
        let app = GloryBlitzApplication::new().window(config, Counter { value: Cage::new(0) });

        assert_eq!(app.pending_window_count(), 1);
    }

    #[test]
    #[cfg(feature = "shell")]
    fn blitz_window_config_builds_initial_document() {
        let config = GloryBlitzWindowConfig::default();
        let _window = create_blitz_window_config(config, Counter { value: Cage::new(0) });
    }

    #[test]
    #[cfg(feature = "shell")]
    fn pointer_event_payload_preserves_touch_metadata() {
        let pointer = sample_pointer_event();

        let data = pointer_data(&pointer);
        assert_eq!(data.client_x, 33.0);
        assert_eq!(data.client_y, 44.0);
        assert_eq!(data.button, blitz_traits::events::MouseEventButton::Secondary as i16);
        assert_eq!(data.buttons, blitz_traits::events::MouseEventButtons::Primary.bits() as u16);

        let extra = pointer_event_extra(&pointer);
        assert_eq!(extra["pointer"]["type"], serde_json::json!("touch:42"));
        assert_eq!(extra["pointer"]["primary"], serde_json::json!(true));
        assert_eq!(extra["pointer"]["pressure"], serde_json::json!(0.75));
        assert_eq!(extra["pointer"]["tilt_x"], serde_json::json!(2));
        assert_eq!(extra["pointer"]["tilt_y"], serde_json::json!(-3));
        assert_eq!(extra["pointer"]["twist"], serde_json::json!(91));
    }

    #[test]
    #[cfg(feature = "shell")]
    fn wheel_scroll_and_ime_payloads_are_serialized() {
        let wheel = blitz_traits::events::BlitzWheelEvent {
            delta: blitz_traits::events::BlitzWheelDelta::Lines(1.5, -2.0),
            coords: sample_pointer_coords(),
            buttons: blitz_traits::events::MouseEventButtons::Auxiliary,
            mods: Default::default(),
        };

        let data = wheel_pointer_data(&wheel);
        assert_eq!(data.client_x, 33.0);
        assert_eq!(data.button, 0);
        assert_eq!(data.buttons, blitz_traits::events::MouseEventButtons::Auxiliary.bits() as u16);

        let wheel_extra = wheel_event_extra(&wheel);
        assert_eq!(wheel_extra["wheel"]["delta_x"], serde_json::json!(1.5));
        assert_eq!(wheel_extra["wheel"]["delta_y"], serde_json::json!(-2.0));
        assert_eq!(wheel_extra["wheel"]["delta_mode"], serde_json::json!("line"));

        let scroll_extra = scroll_event_extra(&blitz_traits::events::BlitzScrollEvent {
            scroll_top: 12.0,
            scroll_left: 34.0,
            scroll_width: 1200,
            scroll_height: 900,
            client_width: 600,
            client_height: 450,
        });
        assert_eq!(scroll_extra["scroll"]["top"], serde_json::json!(12.0));
        assert_eq!(scroll_extra["scroll"]["client_height"], serde_json::json!(450));

        let ime_extra = ime_event_extra(&blitz_traits::events::BlitzImeEvent::DeleteSurrounding {
            before_bytes: 3,
            after_bytes: 5,
        });
        assert_eq!(
            ime_extra["ime"],
            serde_json::json!({
                "state": "delete_surrounding",
                "before_bytes": 3,
                "after_bytes": 5
            })
        );
    }

    #[cfg(feature = "shell")]
    fn sample_pointer_coords() -> blitz_traits::events::PointerCoords {
        blitz_traits::events::PointerCoords {
            page_x: 11.0,
            page_y: 22.0,
            screen_x: 55.0,
            screen_y: 66.0,
            client_x: 33.0,
            client_y: 44.0,
        }
    }

    #[test]
    fn layout_cache_is_dirty_until_resolved() {
        let mut cache = LayoutCache::new();
        // Fresh cache starts dirty: nothing resolved yet.
        assert!(cache.needs_resolve());

        cache.mark_resolved();
        assert!(!cache.needs_resolve());

        cache.mark_dirty();
        assert!(cache.needs_resolve());

        // Re-resolving clears it again; idempotent resolves stay clean.
        cache.mark_resolved();
        cache.mark_resolved();
        assert!(!cache.needs_resolve());
    }

    #[test]
    fn property_route_classifies_form_vs_plain_properties() {
        assert_eq!(property_route("value"), PropertyRoute::FormAttribute);
        assert_eq!(property_route("checked"), PropertyRoute::FormAttribute);
        assert_eq!(property_route("data-prop"), PropertyRoute::PropertyOnly);
        assert_eq!(property_route("placeholder"), PropertyRoute::PropertyOnly);
    }

    #[test]
    fn mutations_mark_layout_dirty_and_queries_resolve_once() {
        let mut consumer = BlitzConsumer::new();
        consumer.apply_batch(&[
            Command::Create {
                id: 1,
                name: "div".into(),
                is_void: false,
            },
            Command::SetAttribute {
                id: 1,
                name: "style".into(),
                value: "width: 80px; height: 20px;".into(),
            },
            Command::Insert {
                parent: 0,
                child: 1,
                position: CommandInsertPosition::Tail,
            },
        ]);
        // Pending mutations leave the cache dirty.
        assert!(consumer.layout_cache().needs_resolve());

        consumer.apply(&Command::Query {
            id: 1,
            token: 1,
            kind: NodeQuery::BoundingRect,
        });
        // The query resolved layout and cleared the dirty flag.
        assert!(!consumer.layout_cache().needs_resolve());
        let _ = consumer.take_query_responses();

        // A second query with no intervening mutation reuses the layout.
        consumer.apply(&Command::Query {
            id: 1,
            token: 2,
            kind: NodeQuery::BoundingRect,
        });
        assert!(!consumer.layout_cache().needs_resolve());
        let _ = consumer.take_query_responses();

        // A new mutation re-dirties the cache.
        consumer.apply(&Command::SetAttribute {
            id: 1,
            name: "style".into(),
            value: "width: 90px; height: 30px;".into(),
        });
        assert!(consumer.layout_cache().needs_resolve());
    }

    #[test]
    fn scroll_query_does_not_clear_layout_dirty_flag() {
        let mut consumer = BlitzConsumer::new();
        consumer.apply(&Command::Create {
            id: 1,
            name: "div".into(),
            is_void: false,
        });
        assert!(consumer.layout_cache().needs_resolve());

        // ScrollOffset reads cached node state and must not pretend layout is
        // resolved (it never runs the layout pass).
        consumer.apply(&Command::Query {
            id: 1,
            token: 9,
            kind: NodeQuery::ScrollOffset,
        });
        assert!(consumer.layout_cache().needs_resolve());
        let _ = consumer.take_query_responses();
    }

    #[test]
    fn value_property_is_routed_into_form_attribute() {
        let mut consumer = BlitzConsumer::new();
        consumer.apply_batch(&[
            Command::Create {
                id: 1,
                name: "input".into(),
                is_void: false,
            },
            Command::Insert {
                parent: 0,
                child: 1,
                position: CommandInsertPosition::Tail,
            },
            Command::SetProperty {
                id: 1,
                name: "value".into(),
                value: "hello".into(),
            },
        ]);

        // Tracked as a property AND mirrored into the form attribute.
        assert_eq!(consumer.property(1, "value").as_deref(), Some("hello"));
        assert_eq!(consumer.attribute(1, "value").as_deref(), Some("hello"));
    }

    #[test]
    fn checked_property_set_before_insert_is_reapplied_on_insert() {
        let mut consumer = BlitzConsumer::new();
        // Property arrives before the node is in the document — blitz would
        // drop the `checked` state at this point, but we track it.
        consumer.apply_batch(&[
            Command::Create {
                id: 1,
                name: "input".into(),
                is_void: false,
            },
            Command::SetAttribute {
                id: 1,
                name: "type".into(),
                value: "checkbox".into(),
            },
            Command::SetProperty {
                id: 1,
                name: "checked".into(),
                value: "true".into(),
            },
        ]);
        assert_eq!(consumer.property(1, "checked").as_deref(), Some("true"));

        // Insertion re-applies the reflected property so it lands on the
        // element's `checked` *attribute* while the node is in the document
        // (blitz only honours form mutations once attached).
        consumer.apply(&Command::Insert {
            parent: 0,
            child: 1,
            position: CommandInsertPosition::Tail,
        });
        let blitz_id = consumer.blitz_id(1).unwrap();
        assert_eq!(consumer.attribute(1, "checked").as_deref(), Some("true"));

        // blitz constructs the `CheckboxInput` special data during the layout
        // pass, seeding its state from the `checked` attribute. After a resolve
        // the form state reflects the property we routed before insertion.
        consumer.doc.resolve(0.0);
        let node = consumer.document().get_node(blitz_id).unwrap();
        assert_eq!(node.element_data().and_then(|el| el.checkbox_input_checked()), Some(true));
    }

    #[cfg(feature = "shell")]
    fn sample_pointer_event() -> blitz_traits::events::BlitzPointerEvent {
        blitz_traits::events::BlitzPointerEvent {
            id: blitz_traits::events::BlitzPointerId::Finger(42),
            is_primary: true,
            coords: sample_pointer_coords(),
            button: blitz_traits::events::MouseEventButton::Secondary,
            buttons: blitz_traits::events::MouseEventButtons::Primary,
            mods: Default::default(),
            details: blitz_traits::events::PointerDetails {
                pressure: 0.75,
                tilt_x: 2,
                tilt_y: -3,
                twist: 91,
                ..Default::default()
            },
        }
    }

    // ----------------------------------------------------------------------
    // NT2: pure event-dispatch mapping / routing.
    // ----------------------------------------------------------------------

    #[cfg(feature = "shell")]
    fn sample_key_event(text: Option<&str>, repeating: bool) -> blitz_traits::events::BlitzKeyEvent {
        blitz_traits::events::BlitzKeyEvent {
            key: Default::default(),
            code: Default::default(),
            modifiers: Default::default(),
            location: Default::default(),
            is_auto_repeating: repeating,
            is_composing: false,
            state: blitz_traits::events::KeyState::Pressed,
            text: text.map(Into::into),
        }
    }

    #[test]
    #[cfg(feature = "shell")]
    fn resolve_event_target_walks_chain_to_first_listener() {
        let mut blitz_to_glory = HashMap::new();
        blitz_to_glory.insert(10, 1u64); // target
        blitz_to_glory.insert(20, 2u64); // ancestor with the listener
        let mut listeners = BTreeSet::new();
        listeners.insert((2u64, "click".to_owned()));

        // chain is target → ancestor; only the ancestor listens, so dispatch
        // bubbles up to it.
        let hit = resolve_event_target(&[10, 20], &blitz_to_glory, &listeners, "click");
        assert_eq!(hit, Some(2));

        // No listener for a different event name.
        assert_eq!(resolve_event_target(&[10, 20], &blitz_to_glory, &listeners, "keydown"), None);

        // Unmapped blitz ids are skipped.
        assert_eq!(resolve_event_target(&[99], &blitz_to_glory, &listeners, "click"), None);
    }

    #[test]
    #[cfg(feature = "shell")]
    fn map_dom_event_click_carries_pointer_payload() {
        let data = blitz_traits::events::DomEventData::Click(sample_pointer_event());
        let event = map_dom_event("click", 7, &data);

        assert_eq!(event.name, "click");
        assert_eq!(event.node_id, 7);
        let pointer = event.pointer.expect("pointer payload");
        assert_eq!(pointer.client_x, 33.0);
        assert_eq!(pointer.client_y, 44.0);
        assert_eq!(event.extra.unwrap()["pointer"]["type"], serde_json::json!("touch:42"));
    }

    #[test]
    #[cfg(feature = "shell")]
    fn map_dom_event_pointerdown_and_contextmenu_share_pointer_mapping() {
        for (name, data) in [
            ("pointerdown", blitz_traits::events::DomEventData::PointerDown(sample_pointer_event())),
            ("contextmenu", blitz_traits::events::DomEventData::ContextMenu(sample_pointer_event())),
        ] {
            let event = map_dom_event(name, 1, &data);
            assert_eq!(event.name, name);
            assert!(event.pointer.is_some(), "{name} should carry pointer data");
            assert!(event.extra.is_some());
        }
    }

    #[test]
    #[cfg(feature = "shell")]
    fn map_dom_event_keyboard_sets_key_and_extra() {
        let data = blitz_traits::events::DomEventData::KeyDown(sample_key_event(Some("a"), true));
        let event = map_dom_event("keydown", 3, &data);

        let keyboard = event.keyboard.expect("keyboard payload");
        // Modifiers default to off.
        assert!(!keyboard.alt && !keyboard.ctrl && !keyboard.shift && !keyboard.meta);
        let extra = event.extra.unwrap();
        assert_eq!(extra["repeat"], serde_json::json!(true));
        assert_eq!(extra["composing"], serde_json::json!(false));
        assert_eq!(extra["text"], serde_json::json!("a"));
    }

    #[test]
    #[cfg(feature = "shell")]
    fn map_dom_event_input_extracts_target_value() {
        let data = blitz_traits::events::DomEventData::Input(blitz_traits::events::BlitzInputEvent {
            value: "typed text".to_owned(),
        });
        let event = map_dom_event("input", 4, &data);

        assert_eq!(event.target_value(), "typed text");
        assert_eq!(event.target.unwrap().checked, None);
    }

    #[test]
    #[cfg(feature = "shell")]
    fn map_dom_event_wheel_serializes_delta() {
        let data = blitz_traits::events::DomEventData::Wheel(blitz_traits::events::BlitzWheelEvent {
            delta: blitz_traits::events::BlitzWheelDelta::Pixels(3.0, -7.5),
            coords: sample_pointer_coords(),
            buttons: blitz_traits::events::MouseEventButtons::default(),
            mods: Default::default(),
        });
        let event = map_dom_event("wheel", 5, &data);

        // Wheel surfaces a synthetic pointer (position) plus the delta in extra.
        assert!(event.pointer.is_some());
        let extra = event.extra.unwrap();
        assert_eq!(extra["wheel"]["delta_x"], serde_json::json!(3.0));
        assert_eq!(extra["wheel"]["delta_y"], serde_json::json!(-7.5));
        assert_eq!(extra["wheel"]["delta_mode"], serde_json::json!("pixel"));
    }

    #[test]
    #[cfg(feature = "shell")]
    fn map_dom_event_focus_family_records_focus_type() {
        for name in ["focus", "blur", "focusin", "focusout"] {
            let data = match name {
                "focus" => blitz_traits::events::DomEventData::Focus(blitz_traits::events::BlitzFocusEvent),
                "blur" => blitz_traits::events::DomEventData::Blur(blitz_traits::events::BlitzFocusEvent),
                "focusin" => blitz_traits::events::DomEventData::FocusIn(blitz_traits::events::BlitzFocusEvent),
                _ => blitz_traits::events::DomEventData::FocusOut(blitz_traits::events::BlitzFocusEvent),
            };
            let event = map_dom_event(name, 6, &data);
            assert_eq!(event.extra.unwrap()["focus"]["type"], serde_json::json!(name));
        }
    }

    #[test]
    #[cfg(feature = "shell")]
    fn map_dom_event_scroll_serializes_geometry() {
        let data = blitz_traits::events::DomEventData::Scroll(blitz_traits::events::BlitzScrollEvent {
            scroll_top: 10.0,
            scroll_left: 5.0,
            scroll_width: 800,
            scroll_height: 600,
            client_width: 400,
            client_height: 300,
        });
        let event = map_dom_event("scroll", 8, &data);
        let extra = event.extra.unwrap();
        assert_eq!(extra["scroll"]["top"], serde_json::json!(10.0));
        assert_eq!(extra["scroll"]["client_width"], serde_json::json!(400));
    }
}
