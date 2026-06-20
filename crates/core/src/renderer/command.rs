//! Canonical serializable command stream shared by every non-browser backend.
//!
//! This module is the single authoritative definition of Glory's renderer
//! wire protocol. Desktop (wry webview IPC), future native (Blitz), TUI and
//! liveview backends all consume the same [`Command`] enum; the in-memory
//! [`CommandDom`](crate::renderer::command_dom::CommandDom) test interpreter
//! is the reference implementation of its semantics.
//!
//! # Wire format (semi-stable)
//!
//! [`Command`] serializes with serde's default externally-tagged enum
//! representation, e.g. `{"SetText":{"id":3,"value":"hi"}}`. The JS side
//! (`crates/desktop/src/wry_interpreter.js`) decodes exactly this shape.
//! Renaming variants or fields is a wire-protocol change: update the JS
//! interpreter, `_todos.md` §12 and AGENTS.md together.
//!
//! # Buffering and flush
//!
//! [`CommandRenderer`] never sends commands eagerly. Every operation is
//! buffered in a [`CommandQueue`]; the host (a holder such as
//! `CommandHolder`, or the desktop window host) drains the buffer with
//! [`CommandRenderer::take_batch`] at transaction boundaries — after the
//! initial mount, and after each [`CommandRenderer::dispatch_event`] once
//! the reactive scheduler has settled. This is what makes a batch an atomic
//! unit over IPC.
//!
//! # Node id 0
//!
//! Node id `0` is reserved for the host root (`document.body` on webview
//! backends). [`CommandQueue`] starts allocating ids at `1`; consumers must
//! pre-register their root container under id `0`.

use std::any::Any;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use super::{EventPayload, InsertPosition, Renderer};

/// Child placement in the serializable command stream. Anchors are node ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandInsertPosition {
    Head,
    Tail,
    Before(u64),
    After(u64),
}

/// One renderer instruction. See the module docs for the wire format.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Create {
        id: u64,
        name: String,
        is_void: bool,
    },
    SetAttribute {
        id: u64,
        name: String,
        value: String,
    },
    RemoveAttribute {
        id: u64,
        name: String,
    },
    SetProperty {
        id: u64,
        name: String,
        value: String,
    },
    RemoveProperty {
        id: u64,
        name: String,
    },
    AddClass {
        id: u64,
        value: String,
    },
    RemoveClass {
        id: u64,
        value: String,
    },
    SetText {
        id: u64,
        value: String,
    },
    SetHtml {
        id: u64,
        value: String,
    },
    Insert {
        parent: u64,
        child: u64,
        position: CommandInsertPosition,
    },
    Remove {
        parent: u64,
        child: u64,
    },
    AttachEvent {
        id: u64,
        name: String,
        bubbles: bool,
    },
    DetachEvent {
        id: u64,
        name: String,
    },
    /// Read request against the live node. The consumer answers
    /// asynchronously with a [`QueryResponse`] carrying the same `token`.
    Query {
        id: u64,
        token: u64,
        kind: NodeQuery,
    },
}

impl Command {
    /// Structural commands are coalescing barriers: content writes are never
    /// merged across them because node ids may be created/destroyed/moved.
    fn is_structural(&self) -> bool {
        matches!(
            self,
            Command::Create { .. } | Command::Insert { .. } | Command::Remove { .. } | Command::AttachEvent { .. } | Command::DetachEvent { .. }
        )
    }
}

/// What to read from a live node.
///
/// This is the formal shape of Glory's sync-read discipline: business code
/// never reads platform state synchronously. Remote backends (webview IPC,
/// liveview) answer later; in-process consumers may answer immediately —
/// either way the caller gets a future.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeQuery {
    BoundingRect,
    Value,
    ScrollOffset,
}

/// Successful [`NodeQuery`] payloads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum QueryValue {
    Rect { x: f64, y: f64, width: f64, height: f64 },
    Value(String),
    ScrollOffset { x: f64, y: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundingRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScrollOffset {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryError {
    /// The node no longer exists on the consumer side — a normal race.
    NodeGone,
    /// The consumer cannot answer this query kind.
    Unsupported,
    /// The host dropped the pending query (e.g. window closed).
    HostShutdown,
}

/// Consumer's answer to a [`Command::Query`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryResponse {
    pub token: u64,
    pub result: Result<QueryValue, QueryError>,
}

/// Pointer (mouse / touch / pen) portion of [`EventData`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PointerData {
    pub client_x: f64,
    pub client_y: f64,
    pub button: i16,
    pub buttons: u16,
}

/// Keyboard portion of [`EventData`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KeyboardData {
    pub key: String,
    pub code: String,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub meta: bool,
}

/// Snapshot of the event target's form state. This is how `input` /
/// `change` handlers read the current value across the IPC boundary —
/// remote backends cannot synchronously query the live node.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TargetData {
    pub value: Option<String>,
    pub checked: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClipboardData {
    pub text: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SelectionData {
    pub start: Option<u32>,
    pub end: Option<u32>,
    pub direction: Option<String>,
    pub text: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResizeData {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaData {
    pub current_time: f64,
    pub duration: Option<f64>,
    pub paused: Option<bool>,
    pub volume: Option<f64>,
}

/// Serializable event payload produced by command-stream backends.
///
/// This is the cross-platform event model: desktop (wry IPC), mobile and
/// liveview all deliver events as `EventData`. Under the `backend-command`
/// feature, every [`EventDescriptor`](crate::web::events::EventDescriptor)'s
/// `EventType` is `EventData`, so user handlers receive it directly. Fields
/// are optional because different hosts can supply different event families.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EventData {
    pub name: String,
    pub node_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<PointerData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<KeyboardData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<TargetData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clipboard: Option<ClipboardData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<SelectionData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll: Option<ScrollOffset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resize: Option<ResizeData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

impl EventData {
    pub fn new(name: impl Into<String>, node_id: u64) -> Self {
        Self {
            name: name.into(),
            node_id,
            ..Default::default()
        }
    }

    /// The target's `value` (empty string when absent) — the command-stream
    /// counterpart of reading `event.target.value` in the browser.
    pub fn target_value(&self) -> String {
        self.target.as_ref().and_then(|t| t.value.clone()).unwrap_or_default()
    }

    /// The target's `checked` flag (false when absent).
    pub fn target_checked(&self) -> bool {
        self.target.as_ref().and_then(|t| t.checked).unwrap_or(false)
    }

    pub fn clipboard_text(&self) -> Option<&str> {
        self.clipboard.as_ref().and_then(|data| data.text.as_deref())
    }

    pub fn selection_range(&self) -> Option<(u32, u32)> {
        let selection = self.selection.as_ref()?;
        Some((selection.start?, selection.end?))
    }
}

impl EventPayload for EventData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.name)
    }
}

type EventHandler = Box<dyn FnMut(EventData)>;
type QueryWaiter = futures::channel::oneshot::Sender<Result<QueryValue, QueryError>>;

#[derive(Default)]
struct QueueState {
    next_id: u64,
    buffer: Vec<Command>,
    coalesce: bool,
    next_query_token: u64,
    pending_queries: HashMap<u64, QueryWaiter>,
    pending_synthetic_events: VecDeque<EventData>,
}

/// Shared command buffer + event handler registry behind a [`CommandRenderer`].
///
/// Cheap to clone (`Rc` internally). Handlers live in a separate cell from
/// the buffer so dispatching an event — whose handler will synchronously
/// push new commands through the reactive scheduler — never re-enters a
/// held borrow.
#[derive(Clone, Default)]
pub struct CommandQueue {
    state: Rc<RefCell<QueueState>>,
    handlers: Rc<RefCell<HashMap<(u64, String), EventHandler>>>,
}

thread_local! {
    static CURRENT_QUEUE: RefCell<Option<CommandQueue>> = const { RefCell::new(None) };
}

#[cfg(not(feature = "single-app"))]
thread_local! {
    /// Holder-id → queue registry. The reactive scheduler installs the
    /// owning holder's queue around patch runs so widgets created during
    /// any `revise` — not just ones inside `mount`/`dispatch_event`
    /// guards — allocate node ids from the right stream.
    static HOLDER_QUEUES: RefCell<HashMap<crate::HolderId, CommandQueue>> = RefCell::new(HashMap::new());
}

#[cfg(all(not(feature = "single-app"), any(feature = "web-ssr", feature = "backend-command")))]
pub(crate) fn register_holder_queue(holder_id: crate::HolderId, queue: CommandQueue) {
    HOLDER_QUEUES.with_borrow_mut(|queues| {
        queues.insert(holder_id, queue);
    });
}

#[cfg(all(not(feature = "single-app"), any(feature = "web-ssr", feature = "backend-command")))]
pub(crate) fn unregister_holder_queue(holder_id: crate::HolderId) {
    HOLDER_QUEUES.with_borrow_mut(|queues| {
        queues.remove(&holder_id);
    });
}

/// Installs the queue registered for `holder_id` (when any) as
/// thread-current. Called by the reactive scheduler around patch runs.
#[cfg(not(feature = "single-app"))]
pub(crate) fn make_holder_queue_current(holder_id: crate::HolderId) -> Option<CurrentQueueGuard> {
    let queue = HOLDER_QUEUES.with_borrow(|queues| queues.get(&holder_id).cloned())?;
    Some(queue.make_current())
}

/// Restores the previously-current queue on drop. See [`CommandQueue::make_current`].
pub struct CurrentQueueGuard {
    prev: Option<CommandQueue>,
}

impl Drop for CurrentQueueGuard {
    fn drop(&mut self) {
        CURRENT_QUEUE.with_borrow_mut(|current| *current = self.prev.take());
    }
}

impl CommandQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Installs this queue as the thread-current one so that nodes created
    /// via `Node::new` (e.g. by element factories like `div()`) allocate
    /// from it. Holders call this around mount and event dispatch.
    pub fn make_current(&self) -> CurrentQueueGuard {
        let prev = CURRENT_QUEUE.with_borrow_mut(|current| current.replace(self.clone()));
        CurrentQueueGuard { prev }
    }

    /// The thread-current queue, if a holder installed one.
    pub fn current() -> Option<CommandQueue> {
        CURRENT_QUEUE.with_borrow(|current| current.clone())
    }

    fn shares_state_with(&self, other: &CommandQueue) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }

    pub(crate) fn next_id(&self) -> u64 {
        let mut state = self.state.borrow_mut();
        state.next_id += 1;
        state.next_id
    }

    pub(crate) fn push(&self, command: Command) {
        self.state.borrow_mut().buffer.push(command);
    }

    /// Enables the optional coalescing pass applied by [`Self::take_batch`].
    /// Off by default: with fine-grained reactivity redundant writes are
    /// rare, and the pass only pays for itself on IPC/remote sinks.
    pub fn set_coalesce(&self, enabled: bool) {
        self.state.borrow_mut().coalesce = enabled;
    }

    /// Non-draining copy of the buffered commands (test/inspection helper).
    pub fn commands(&self) -> Vec<Command> {
        self.state.borrow().buffer.clone()
    }

    /// Read-only diagnostic snapshot for devtools.
    pub fn devtools_snapshot(&self) -> crate::devtools::CommandQueueSnapshot {
        let state = self.state.borrow();
        crate::devtools::CommandQueueSnapshot {
            buffered_command_count: state.buffer.len(),
            handler_count: self.handlers.borrow().len(),
            pending_query_count: state.pending_queries.len(),
            next_node_id: state.next_id,
            next_query_token: state.next_query_token,
            coalesce: state.coalesce,
        }
    }

    /// Drains the buffer, applying the coalescing pass when enabled.
    pub fn take_batch(&self) -> Vec<Command> {
        self.flush_pending_synthetic_events();
        let (batch, coalesce_enabled) = {
            let mut state = self.state.borrow_mut();
            let coalesce = state.coalesce;
            (std::mem::take(&mut state.buffer), coalesce)
        };
        if coalesce_enabled { coalesce(batch) } else { batch }
    }

    fn enqueue_synthetic_event(&self, data: EventData) {
        self.state.borrow_mut().pending_synthetic_events.push_back(data);
    }

    fn flush_pending_synthetic_events(&self) {
        loop {
            let data = { self.state.borrow_mut().pending_synthetic_events.pop_front() };
            let Some(data) = data else {
                break;
            };
            let _ = self.dispatch(data);
        }
    }

    /// Issues a read request against `node_id`. The returned future
    /// resolves when the consumer answers via [`Self::resolve_query`].
    ///
    /// Do not block the host event-loop thread on this future — the answer
    /// arrives through that same loop. Await it from a task the loop can
    /// keep pumping, or poll it after the response was delivered.
    pub fn query(&self, node_id: u64, kind: NodeQuery) -> impl std::future::Future<Output = Result<QueryValue, QueryError>> + use<> {
        let (sender, receiver) = futures::channel::oneshot::channel();
        let token = {
            let mut state = self.state.borrow_mut();
            state.next_query_token += 1;
            let token = state.next_query_token;
            state.pending_queries.insert(token, sender);
            state.buffer.push(Command::Query { id: node_id, token, kind });
            token
        };
        let _ = token;
        async move {
            match receiver.await {
                Ok(result) => result,
                Err(_) => Err(QueryError::HostShutdown),
            }
        }
    }

    /// Delivers a consumer answer to the matching pending query. Returns
    /// `false` when the token is unknown (already resolved or dropped).
    pub fn resolve_query(&self, response: QueryResponse) -> bool {
        let waiter = self.state.borrow_mut().pending_queries.remove(&response.token);
        match waiter {
            Some(sender) => sender.send(response.result).is_ok(),
            None => false,
        }
    }

    pub(crate) fn register_handler(&self, id: u64, name: impl Into<String>, handler: EventHandler) {
        let name = name.into();
        self.handlers.borrow_mut().insert((id, name.clone()), handler);
        if name == "mounted" {
            self.enqueue_synthetic_event(EventData::new("mounted", id));
        }
    }

    pub(crate) fn remove_handler(&self, id: u64, name: &str) {
        self.handlers.borrow_mut().remove(&(id, name.to_owned()));
    }

    /// Looks up and invokes the handler registered for `(data.node_id,
    /// data.name)`. Returns `false` when no handler is registered (e.g. the
    /// node was removed while the event was in flight — a normal race on
    /// IPC backends, not an error).
    ///
    /// Re-entrancy safe: the handler is moved out of the registry during the
    /// call, so it can freely trigger reactive updates that push commands or
    /// attach further handlers.
    pub fn dispatch(&self, data: EventData) -> bool {
        let key = (data.node_id, data.name.clone());
        let Some(mut handler) = self.handlers.borrow_mut().remove(&key) else {
            return false;
        };
        handler(data);
        // The handler may have been replaced while it ran (e.g. re-attach);
        // only restore it if the slot is still empty.
        let mut handlers = self.handlers.borrow_mut();
        handlers.entry(key).or_insert(handler);
        true
    }
}

impl fmt::Debug for CommandQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.borrow();
        f.debug_struct("CommandQueue")
            .field("buffered", &state.buffer.len())
            .field("next_id", &state.next_id)
            .field("handlers", &self.handlers.borrow().len())
            .finish()
    }
}

/// Merge redundant content writes within one batch.
///
/// Reverse scan keeping the *last* write per `(node, slot)`; structural
/// commands ([`Command::is_structural`]) are barriers — nothing merges
/// across them, because ids may be created, moved or removed there.
/// Class commands are set operations and are never merged.
pub fn coalesce(batch: Vec<Command>) -> Vec<Command> {
    use std::collections::HashSet;

    #[derive(Hash, PartialEq, Eq)]
    enum Slot {
        Attr(u64, String),
        Prop(u64, String),
        Text(u64),
        Html(u64),
    }

    let mut seen: HashSet<Slot> = HashSet::new();
    let mut kept_rev: Vec<Command> = Vec::with_capacity(batch.len());
    for command in batch.into_iter().rev() {
        if command.is_structural() {
            seen.clear();
            kept_rev.push(command);
            continue;
        }
        let slot = match &command {
            Command::SetAttribute { id, name, .. } | Command::RemoveAttribute { id, name } => Some(Slot::Attr(*id, name.clone())),
            Command::SetProperty { id, name, .. } | Command::RemoveProperty { id, name } => Some(Slot::Prop(*id, name.clone())),
            Command::SetText { id, .. } => Some(Slot::Text(*id)),
            Command::SetHtml { id, .. } => Some(Slot::Html(*id)),
            _ => None,
        };
        match slot {
            Some(slot) => {
                if seen.insert(slot) {
                    kept_rev.push(command);
                }
            }
            None => kept_rev.push(command),
        }
    }
    kept_rev.reverse();
    kept_rev
}

/// Id-based node handle used by command-stream backends.
///
/// Mirrors the method surface of the SSR in-memory `Node` (set_attribute,
/// add_class, append_with_node, …) so the shared widget/attr/prop/class
/// layers work unchanged — each method simply records a [`Command`] into
/// the shared queue instead of mutating a local tree.
#[derive(Clone)]
pub struct CommandNode {
    id: u64,
    name: Rc<str>,
    is_void: bool,
    queue: CommandQueue,
}

impl PartialEq for CommandNode {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for CommandNode {}

impl fmt::Debug for CommandNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CommandNode(#{} <{}>)", self.id, self.name)
    }
}

impl CommandNode {
    /// Creates a node allocated from the thread-current queue (installed by
    /// the holder via [`CommandQueue::make_current`]). Falls back to an
    /// isolated queue when none is current — such a node's commands never
    /// reach a host batch, which is only acceptable in unit tests.
    pub fn new(name: impl Into<Cow<'static, str>>, is_void: bool) -> Self {
        let queue = CommandQueue::current().unwrap_or_else(|| {
            crate::debug_warn!("CommandNode::new outside a current CommandQueue; commands will go to an isolated buffer");
            CommandQueue::new()
        });
        Self::create_in(queue, name, is_void)
    }

    pub(crate) fn create_in(queue: CommandQueue, name: impl Into<Cow<'static, str>>, is_void: bool) -> Self {
        let name = name.into();
        let id = queue.next_id();
        queue.push(Command::Create {
            id,
            name: name.to_string(),
            is_void,
        });
        Self {
            id,
            name: Rc::from(&*name),
            is_void,
            queue,
        }
    }

    /// The reserved host root node (id 0). Consumers pre-register their root
    /// container (e.g. `document.body`) under this id; no `Create` command
    /// is emitted for it.
    pub fn host(queue: CommandQueue) -> Self {
        Self {
            id: 0,
            name: Rc::from("body"),
            is_void: false,
            queue,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn tag_name(&self) -> &str {
        &self.name
    }
    pub fn is_void(&self) -> bool {
        self.is_void
    }
    pub(crate) fn queue(&self) -> &CommandQueue {
        &self.queue
    }

    /// Identity comparison (same node id). Named after the SSR `Node`
    /// method so shared code compiles against both backends.
    pub fn ptr_eq(&self, other: &CommandNode) -> bool {
        self.id == other.id
    }

    /// Sets an attribute. The SSR-convention pseudo attributes
    /// `inner_text` / `inner_html` are translated to [`Command::SetText`] /
    /// [`Command::SetHtml`] so remote DOM interpreters apply real text/HTML
    /// content instead of literal attributes.
    pub fn set_attribute(&self, key: impl Into<Cow<'static, str>>, value: impl Into<Cow<'static, str>>) {
        let key = key.into();
        let value = value.into().into_owned();
        match &*key {
            "inner_text" => self.queue.push(Command::SetText { id: self.id, value }),
            "inner_html" => self.queue.push(Command::SetHtml { id: self.id, value }),
            _ => self.queue.push(Command::SetAttribute {
                id: self.id,
                name: key.into_owned(),
                value,
            }),
        }
    }

    pub fn remove_attribute(&self, key: &str) {
        match key {
            "inner_text" => self.queue.push(Command::SetText {
                id: self.id,
                value: String::new(),
            }),
            "inner_html" => self.queue.push(Command::SetHtml {
                id: self.id,
                value: String::new(),
            }),
            _ => self.queue.push(Command::RemoveAttribute {
                id: self.id,
                name: key.to_owned(),
            }),
        }
    }

    /// Sets a property. The SSR-convention `text` property is translated to
    /// [`Command::SetText`]. A `None` value maps to [`Command::RemoveProperty`]
    /// (closest wire equivalent of a valueless property).
    pub fn set_property(&self, key: impl Into<Cow<'static, str>>, value: impl Into<Option<Cow<'static, str>>>) {
        let key = key.into();
        let value = value.into();
        match (&*key, value) {
            ("text", value) => self.queue.push(Command::SetText {
                id: self.id,
                value: value.map(Cow::into_owned).unwrap_or_default(),
            }),
            (_, Some(value)) => self.queue.push(Command::SetProperty {
                id: self.id,
                name: key.into_owned(),
                value: value.into_owned(),
            }),
            (_, None) => self.queue.push(Command::RemoveProperty {
                id: self.id,
                name: key.into_owned(),
            }),
        }
    }

    pub fn remove_property(&self, key: &str) {
        self.queue.push(Command::RemoveProperty {
            id: self.id,
            name: key.to_owned(),
        });
    }

    pub fn add_class(&self, value: impl Into<Cow<'static, str>>) {
        self.queue.push(Command::AddClass {
            id: self.id,
            value: value.into().into_owned(),
        });
    }

    pub fn remove_class(&self, key: &str) {
        self.queue.push(Command::RemoveClass {
            id: self.id,
            value: key.to_owned(),
        });
    }

    pub fn prepend_with_node(&self, node: &CommandNode) {
        self.queue.push(Command::Insert {
            parent: self.id,
            child: node.id,
            position: CommandInsertPosition::Head,
        });
    }

    pub fn append_with_node(&self, node: &CommandNode) {
        self.queue.push(Command::Insert {
            parent: self.id,
            child: node.id,
            position: CommandInsertPosition::Tail,
        });
    }

    pub fn insert_before(&self, anchor: &CommandNode, new_node: &CommandNode) {
        self.queue.push(Command::Insert {
            parent: self.id,
            child: new_node.id,
            position: CommandInsertPosition::Before(anchor.id),
        });
    }

    pub fn insert_after(&self, anchor: &CommandNode, new_node: &CommandNode) {
        self.queue.push(Command::Insert {
            parent: self.id,
            child: new_node.id,
            position: CommandInsertPosition::After(anchor.id),
        });
    }

    pub fn remove_child(&self, node: &CommandNode) {
        self.queue.push(Command::Remove {
            parent: self.id,
            child: node.id,
        });
    }

    /// Unregisters this node's handler for `name` and tells the consumer to
    /// drop its listener. Called when the owning widget is dropped.
    pub fn detach_event(&self, name: &str) {
        self.queue.remove_handler(self.id, name);
        self.queue.push(Command::DetachEvent {
            id: self.id,
            name: name.to_owned(),
        });
    }

    /// Reads the live value of this node through the command-stream query
    /// channel. Remote hosts answer asynchronously via [`QueryResponse`].
    pub fn value(&self) -> impl std::future::Future<Output = Result<String, QueryError>> + use<> {
        let query = self.queue.query(self.id, NodeQuery::Value);
        async move {
            match query.await? {
                QueryValue::Value(value) => Ok(value),
                _ => Err(QueryError::Unsupported),
            }
        }
    }

    /// Reads the live bounding rectangle of this node.
    pub fn bounding_rect(&self) -> impl std::future::Future<Output = Result<BoundingRect, QueryError>> + use<> {
        let query = self.queue.query(self.id, NodeQuery::BoundingRect);
        async move {
            match query.await? {
                QueryValue::Rect { x, y, width, height } => Ok(BoundingRect { x, y, width, height }),
                _ => Err(QueryError::Unsupported),
            }
        }
    }

    /// Reads the live scroll offset of this node.
    pub fn scroll_offset(&self) -> impl std::future::Future<Output = Result<ScrollOffset, QueryError>> + use<> {
        let query = self.queue.query(self.id, NodeQuery::ScrollOffset);
        async move {
            match query.await? {
                QueryValue::ScrollOffset { x, y } => Ok(ScrollOffset { x, y }),
                _ => Err(QueryError::Unsupported),
            }
        }
    }
}

/// [`Renderer`] implementation that records [`Command`]s into a shared
/// [`CommandQueue`] instead of touching a real UI tree.
#[derive(Clone)]
pub struct CommandRenderer {
    queue: CommandQueue,
}

impl CommandRenderer {
    /// A renderer over a fresh, isolated queue.
    pub fn new() -> Self {
        Self::from_queue(CommandQueue::new())
    }

    pub fn from_queue(queue: CommandQueue) -> Self {
        Self { queue }
    }

    pub fn queue(&self) -> &CommandQueue {
        &self.queue
    }

    /// Non-draining copy of buffered commands (test/inspection helper).
    pub fn commands(&self) -> Vec<Command> {
        self.queue.commands()
    }

    /// Drains the buffered batch. See [`CommandQueue::take_batch`].
    pub fn take_batch(&self) -> Vec<Command> {
        self.queue.take_batch()
    }

    /// Delivers a deserialized backend event to the registered handler.
    /// Hosts should wrap this in `reflow::batch` so all signal writes the
    /// handler performs settle as one transaction before the next
    /// [`Self::take_batch`].
    pub fn dispatch_event(&self, data: EventData) -> bool {
        self.queue.dispatch(data)
    }

    /// Async read against a live node. See [`CommandQueue::query`].
    pub fn query(&self, node: &CommandNode, kind: NodeQuery) -> impl std::future::Future<Output = Result<QueryValue, QueryError>> + use<> {
        self.queue.query(node.id(), kind)
    }

    /// Delivers a consumer's [`QueryResponse`]. See [`CommandQueue::resolve_query`].
    pub fn resolve_query(&self, response: QueryResponse) -> bool {
        self.queue.resolve_query(response)
    }
}

/// `Default` resolves to the thread-current queue (when a holder installed
/// one) so that independently-constructed widgets share the host buffer;
/// otherwise an isolated queue is created.
impl Default for CommandRenderer {
    fn default() -> Self {
        match CommandQueue::current() {
            Some(queue) => Self::from_queue(queue),
            None => Self::new(),
        }
    }
}

impl fmt::Debug for CommandRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandRenderer").field("queue", &self.queue).finish()
    }
}

impl Renderer for CommandRenderer {
    type Event = EventData;
    type Node = CommandNode;

    fn create_element(&self, name: Cow<'static, str>, is_void: bool) -> Self::Node {
        CommandNode::create_in(self.queue.clone(), name, is_void)
    }

    fn set_attribute(&self, node: &Self::Node, name: Cow<'static, str>, value: Cow<'static, str>) {
        node.set_attribute(name, value);
    }

    fn remove_attribute(&self, node: &Self::Node, name: &str) {
        node.remove_attribute(name);
    }

    fn set_property(&self, node: &Self::Node, name: Cow<'static, str>, value: Cow<'static, str>) {
        node.set_property(name, Some(value));
    }

    fn remove_property(&self, node: &Self::Node, name: &str) {
        node.remove_property(name);
    }

    fn add_class(&self, node: &Self::Node, value: Cow<'static, str>) {
        node.add_class(value);
    }

    fn remove_class(&self, node: &Self::Node, value: &str) {
        node.remove_class(value);
    }

    fn set_text(&self, node: &Self::Node, value: Cow<'static, str>) {
        node.queue().push(Command::SetText {
            id: node.id(),
            value: value.into_owned(),
        });
    }

    fn set_html(&self, node: &Self::Node, value: Cow<'static, str>) {
        node.queue().push(Command::SetHtml {
            id: node.id(),
            value: value.into_owned(),
        });
    }

    fn insert_child(&self, parent: &Self::Node, child: &Self::Node, position: InsertPosition<'_, Self::Node>) {
        match position {
            InsertPosition::Head => parent.prepend_with_node(child),
            InsertPosition::Tail => parent.append_with_node(child),
            InsertPosition::Before(anchor) => parent.insert_before(anchor, child),
            InsertPosition::After(anchor) => parent.insert_after(anchor, child),
        }
    }

    fn remove_child(&self, parent: &Self::Node, child: &Self::Node) {
        parent.remove_child(child);
    }

    fn node_identity_eq(&self, left: &Self::Node, right: &Self::Node) -> bool {
        left.ptr_eq(right)
    }

    fn attach_event(&self, node: &Self::Node, name: Cow<'static, str>, bubbles: bool, handler: Box<dyn FnMut(Self::Event)>) {
        debug_assert!(
            node.queue().shares_state_with(&self.queue),
            "attach_event: node belongs to a different CommandQueue"
        );
        node.queue().register_handler(node.id(), name.clone().into_owned(), handler);
        node.queue().push(Command::AttachEvent {
            id: node.id(),
            name: name.into_owned(),
            bubbles,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_buffers_and_drains_batches() {
        let renderer = CommandRenderer::new();
        let root = renderer.create_element("main".into(), false);
        let child = renderer.create_element("button".into(), false);
        renderer.set_text(&child, "Click".into());
        renderer.insert_child(&root, &child, InsertPosition::Tail);

        let batch = renderer.take_batch();
        assert_eq!(
            batch,
            vec![
                Command::Create {
                    id: 1,
                    name: "main".into(),
                    is_void: false
                },
                Command::Create {
                    id: 2,
                    name: "button".into(),
                    is_void: false
                },
                Command::SetText {
                    id: 2,
                    value: "Click".into()
                },
                Command::Insert {
                    parent: 1,
                    child: 2,
                    position: CommandInsertPosition::Tail
                },
            ]
        );
        assert!(renderer.take_batch().is_empty(), "buffer must drain");
    }

    #[test]
    fn command_queue_devtools_snapshot_reports_counts() {
        let renderer = CommandRenderer::new();
        let node = renderer.create_element("button".into(), false);
        renderer.attach_event(&node, "click".into(), true, Box::new(|_| {}));

        let snapshot = renderer.queue.devtools_snapshot();
        assert_eq!(snapshot.buffered_command_count, 2);
        assert_eq!(snapshot.handler_count, 1);
        assert_eq!(snapshot.pending_query_count, 0);
        assert_eq!(snapshot.next_node_id, 1);
        assert_eq!(snapshot.next_query_token, 0);
        assert!(!snapshot.coalesce);
    }

    #[test]
    fn node_translates_ssr_pseudo_attributes() {
        let renderer = CommandRenderer::new();
        let node = renderer.create_element("p".into(), false);
        node.set_attribute("inner_text", "hello");
        node.set_attribute("inner_html", "<b>raw</b>");
        node.set_attribute("title", "t");
        node.set_property("text", Some(Cow::from("prop-text")));
        node.remove_attribute("inner_text");

        let batch = renderer.take_batch();
        assert_eq!(
            &batch[1..],
            &[
                Command::SetText {
                    id: 1,
                    value: "hello".into()
                },
                Command::SetHtml {
                    id: 1,
                    value: "<b>raw</b>".into()
                },
                Command::SetAttribute {
                    id: 1,
                    name: "title".into(),
                    value: "t".into()
                },
                Command::SetText {
                    id: 1,
                    value: "prop-text".into()
                },
                Command::SetText { id: 1, value: String::new() },
            ]
        );
    }

    #[test]
    fn coalesce_keeps_last_write_per_slot() {
        let renderer = CommandRenderer::new();
        renderer.queue().set_coalesce(true);
        let node = renderer.create_element("p".into(), false);
        renderer.set_text(&node, "a".into());
        renderer.set_text(&node, "b".into());
        node.set_attribute("title", "x");
        node.set_attribute("title", "y");

        let batch = renderer.take_batch();
        assert_eq!(
            batch,
            vec![
                Command::Create {
                    id: 1,
                    name: "p".into(),
                    is_void: false
                },
                Command::SetText { id: 1, value: "b".into() },
                Command::SetAttribute {
                    id: 1,
                    name: "title".into(),
                    value: "y".into()
                },
            ]
        );
    }

    #[test]
    fn coalesce_never_merges_across_structural_barriers() {
        let parent = CommandNode::host(CommandQueue::new());
        let queue = parent.queue().clone();
        queue.set_coalesce(true);
        let renderer = CommandRenderer::from_queue(queue.clone());
        let node = renderer.create_element("p".into(), false);

        renderer.set_text(&node, "before".into());
        renderer.remove_child(&parent, &node);
        renderer.set_text(&node, "after".into());

        let batch = renderer.take_batch();
        assert_eq!(
            batch,
            vec![
                Command::Create {
                    id: 1,
                    name: "p".into(),
                    is_void: false
                },
                Command::SetText {
                    id: 1,
                    value: "before".into()
                },
                Command::Remove { parent: 0, child: 1 },
                Command::SetText {
                    id: 1,
                    value: "after".into()
                },
            ]
        );
    }

    #[test]
    fn dispatch_event_reaches_registered_handler_and_supports_reentrancy() {
        use std::cell::Cell;
        use std::rc::Rc;

        let renderer = CommandRenderer::new();
        let node = renderer.create_element("button".into(), false);
        let hits = Rc::new(Cell::new(0));
        let hits2 = hits.clone();
        let reentrant_node = node.clone();
        renderer.attach_event(
            &node,
            "click".into(),
            true,
            Box::new(move |data: EventData| {
                hits2.set(hits2.get() + 1);
                assert_eq!(data.node_id, reentrant_node.id());
                // Re-entrant command push must not panic.
                reentrant_node.set_attribute("data-clicked", "true");
            }),
        );

        assert!(renderer.dispatch_event(EventData::new("click", node.id())));
        assert!(renderer.dispatch_event(EventData::new("click", node.id())), "handler must be restored");
        assert_eq!(hits.get(), 2);
        assert!(!renderer.dispatch_event(EventData::new("click", 999)), "unknown node returns false");

        let batch = renderer.take_batch();
        assert!(batch.contains(&Command::SetAttribute {
            id: node.id(),
            name: "data-clicked".into(),
            value: "true".into()
        }));
    }

    #[test]
    fn event_data_round_trips_through_json() {
        let mut data = EventData::new("input", 7);
        data.target = Some(TargetData {
            value: Some("typed".into()),
            checked: None,
        });
        data.clipboard = Some(ClipboardData { text: Some("copied".into()) });
        data.selection = Some(SelectionData {
            start: Some(1),
            end: Some(4),
            direction: Some("forward".into()),
            text: Some("ype".into()),
        });
        data.scroll = Some(ScrollOffset { x: 3.0, y: 5.0 });
        data.resize = Some(ResizeData { width: 640.0, height: 480.0 });
        data.media = Some(MediaData {
            current_time: 1.5,
            duration: Some(30.0),
            paused: Some(false),
            volume: Some(0.75),
        });
        let json = serde_json::to_string(&data).unwrap();
        let back: EventData = serde_json::from_str(&json).unwrap();
        assert_eq!(back, data);
        assert_eq!(back.target_value(), "typed");
        assert_eq!(back.clipboard_text(), Some("copied"));
        assert_eq!(back.selection_range(), Some((1, 4)));
    }

    #[test]
    fn command_node_value_helper_resolves_query() {
        let renderer = CommandRenderer::new();
        let node = renderer.create_element("input".into(), false);
        renderer.set_property(&node, "value".into(), "typed".into());
        let value = node.value();

        let mut dom = crate::renderer::command_dom::CommandDom::new();
        dom.apply_batch(&renderer.take_batch());
        for response in dom.take_query_responses() {
            assert!(renderer.resolve_query(response));
        }

        assert_eq!(futures::executor::block_on(value).unwrap(), "typed");
    }

    #[test]
    fn command_node_bounding_rect_helper_maps_response() {
        let renderer = CommandRenderer::new();
        let node = renderer.create_element("div".into(), false);
        let rect = node.bounding_rect();
        let token = renderer
            .take_batch()
            .into_iter()
            .find_map(|command| match command {
                Command::Query { token, .. } => Some(token),
                _ => None,
            })
            .expect("query command emitted");

        assert!(renderer.resolve_query(QueryResponse {
            token,
            result: Ok(QueryValue::Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            }),
        }));

        assert_eq!(
            futures::executor::block_on(rect).unwrap(),
            BoundingRect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0
            }
        );
    }

    #[test]
    fn command_wire_format_is_externally_tagged() {
        let json = serde_json::to_string(&Command::SetText { id: 3, value: "hi".into() }).unwrap();
        assert_eq!(json, r#"{"SetText":{"id":3,"value":"hi"}}"#);
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Command::SetText { id: 3, value: "hi".into() });
    }

    #[test]
    fn current_queue_guard_restores_previous() {
        let outer = CommandQueue::new();
        let inner = CommandQueue::new();
        let _outer_guard = outer.make_current();
        {
            let _inner_guard = inner.make_current();
            let node = CommandNode::new("div", false);
            assert!(node.queue().shares_state_with(&inner));
        }
        let node = CommandNode::new("div", false);
        assert!(node.queue().shares_state_with(&outer));
    }
}
