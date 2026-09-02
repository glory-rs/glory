//! LiveView protocol primitives for Glory.
//!
//! This crate does not choose a web framework. It owns the command-stream
//! session model that a Salvo/Axum/Actix WebSocket adapter can wrap:
//!
//! 1. The server mounts a widget into a [`LiveViewSession`].
//! 2. The initial [`Command`](glory_core::renderer::Command) batch is sent to
//!    the browser.
//! 3. The browser applies commands, sends [`EventData`] and [`QueryResponse`]
//!    messages back, and receives patch batches.

use glory_core::renderer::{Command, EventData, QueryResponse};
use glory_core::web::holders::CommandHolder;
use glory_core::{Holder, Widget};
use serde::{Deserialize, Serialize};

pub const LIVEVIEW_PROTOCOL_VERSION: u32 = 1;
pub const LIVEVIEW_DEFAULT_PATH: &str = "/__glory/liveview";
pub const LIVEVIEW_CLIENT_JS: &str = include_str!("liveview_client.js");

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum LiveViewMessage {
    Hello {
        protocol_version: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resume_token: Option<String>,
    },
    Session {
        token: String,
    },
    Resumed {
        token: String,
    },
    Mount {
        commands: Vec<Command>,
    },
    Event(Box<EventData>),
    Query(Box<QueryResponse>),
    Patch {
        commands: Vec<Command>,
    },
    Error {
        message: String,
    },
    Ping,
    Pong,
    Close {
        reason: String,
    },
}

impl LiveViewMessage {
    pub fn hello() -> Self {
        Self::Hello {
            protocol_version: LIVEVIEW_PROTOCOL_VERSION,
            resume_token: None,
        }
    }

    pub fn hello_with_resume(token: impl Into<String>) -> Self {
        Self::Hello {
            protocol_version: LIVEVIEW_PROTOCOL_VERSION,
            resume_token: Some(token.into()),
        }
    }

    pub fn patch(commands: Vec<Command>) -> Self {
        Self::Patch { commands }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(input: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(input)
    }
}

pub struct LiveViewSession {
    holder: CommandHolder,
}

impl LiveViewSession {
    pub fn mount(widget: impl Widget) -> (Self, LiveViewMessage) {
        let holder = CommandHolder::new().mount(widget);
        let commands = holder.take_batch();
        (Self { holder }, LiveViewMessage::Mount { commands })
    }

    pub fn holder(&self) -> &CommandHolder {
        &self.holder
    }

    pub fn dispatch_event(&self, event: EventData) -> LiveViewMessage {
        self.holder.dispatch_event(event);
        LiveViewMessage::patch(self.holder.take_batch())
    }

    pub fn resolve_query(&self, response: QueryResponse) -> LiveViewMessage {
        self.holder.resolve_query(response);
        LiveViewMessage::patch(self.holder.take_batch())
    }

    pub fn handle_message(&self, message: LiveViewMessage) -> Option<LiveViewMessage> {
        match message {
            LiveViewMessage::Event(event) => Some(self.dispatch_event(*event)),
            LiveViewMessage::Query(response) => Some(self.resolve_query(*response)),
            LiveViewMessage::Ping => Some(LiveViewMessage::Pong),
            // Protocol negotiation: a client whose protocol version does not
            // match the server's is told so, rather than being silently driven
            // with a possibly-incompatible wire format.
            LiveViewMessage::Hello { protocol_version, .. } => {
                if protocol_version != LIVEVIEW_PROTOCOL_VERSION {
                    Some(LiveViewMessage::Error {
                        message: format!("liveview protocol mismatch: client {protocol_version}, server {LIVEVIEW_PROTOCOL_VERSION}"),
                    })
                } else {
                    None
                }
            }
            LiveViewMessage::Close { .. } => None,
            LiveViewMessage::Session { .. }
            | LiveViewMessage::Resumed { .. }
            | LiveViewMessage::Mount { .. }
            | LiveViewMessage::Patch { .. }
            | LiveViewMessage::Error { .. }
            | LiveViewMessage::Pong => None,
        }
    }

    /// Snapshot the holder's pending command batch into an [`OutboundBuffer`]
    /// as a single patch, applying its backpressure policy. Lets adapters
    /// coalesce output when a client is consuming slowly (LV2).
    pub fn enqueue_patch(&self, buffer: &mut OutboundBuffer) {
        buffer.push(self.holder.take_batch());
    }
}

/// Outbound command-batch buffer with a coalescing backpressure policy.
///
/// Each `push` appends a patch batch. While the number of pending batches is
/// within `max_pending` they are kept separate; once the cap is exceeded, new
/// batches are folded into the tail batch (commands are concatenated, which is
/// a faithful sequential replay) so the queue cannot grow without bound when a
/// client is slow. `max_pending == 0` disables the cap (unbounded).
#[derive(Debug, Default)]
pub struct OutboundBuffer {
    max_pending: usize,
    batches: std::collections::VecDeque<Vec<Command>>,
}

impl OutboundBuffer {
    pub fn new(max_pending: usize) -> Self {
        Self {
            max_pending,
            batches: std::collections::VecDeque::new(),
        }
    }

    pub fn from_config(config: &LiveViewConfig) -> Self {
        Self::new(config.max_pending_patches)
    }

    /// Enqueue a patch batch (empty batches are dropped). Folds into the tail
    /// when over the pending cap.
    pub fn push(&mut self, batch: Vec<Command>) {
        if batch.is_empty() {
            return;
        }
        if self.max_pending != 0
            && self.batches.len() >= self.max_pending
            && let Some(tail) = self.batches.back_mut()
        {
            tail.extend(batch);
            return;
        }
        self.batches.push_back(batch);
    }

    /// Pop the oldest pending batch (FIFO).
    pub fn drain_one(&mut self) -> Option<Vec<Command>> {
        self.batches.pop_front()
    }

    /// Drain every pending batch in order.
    pub fn take(&mut self) -> Vec<Vec<Command>> {
        self.batches.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.batches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.batches.is_empty()
    }

    pub fn max_pending(&self) -> usize {
        self.max_pending
    }

    /// Total number of commands across all pending batches.
    pub fn command_count(&self) -> usize {
        self.batches.iter().map(Vec::len).sum()
    }
}

/// Lifecycle policy for LiveView sessions held in a [`SessionRegistry`].
///
/// A value of `0` for either timeout field disables that particular check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveViewConfig {
    /// Reap a session after this many seconds without any client activity
    /// (no event/query/ping touched it).
    pub idle_timeout_secs: u64,
    /// Hard cap on a session's total lifetime, regardless of activity.
    pub max_lifetime_secs: u64,
    /// Maximum number of pending outbound patch batches before an
    /// [`OutboundBuffer`] starts coalescing. `0` = unbounded.
    pub max_pending_patches: usize,
    /// Origins allowed to open a LiveView socket, in addition to the request's
    /// own host. Empty (the default) means same-origin only.
    ///
    /// Browsers do **not** apply the same-origin policy to WebSocket
    /// handshakes: any page on the internet can open a socket to this endpoint
    /// and it will carry the user's cookies. Checking `Origin` is the standard
    /// defence (cross-site WebSocket hijacking).
    pub allowed_origins: &'static [&'static str],
    /// Largest inbound websocket message accepted, in bytes. `0` = the
    /// framework default (64 MiB for the tungstenite-based adapters), which is
    /// far more than any legitimate LiveView frame and lets one client force a
    /// very large allocation and parse.
    pub max_message_bytes: usize,
    /// Hard cap on concurrently registered sessions. `0` = unbounded.
    ///
    /// Each session owns a live widget tree and a task on a worker thread, and
    /// a session outlives the socket that created it (that is what makes
    /// resume possible). Without a cap, repeatedly opening and dropping
    /// connections piles up trees until the idle timeout finally collects
    /// them.
    pub max_sessions: usize,
}

impl Default for LiveViewConfig {
    fn default() -> Self {
        // 5 minutes idle, 1 hour absolute, 64 pending patches — conservative
        // defaults a host app can override.
        Self {
            idle_timeout_secs: 300,
            max_lifetime_secs: 3600,
            max_pending_patches: 64,
            allowed_origins: &[],
            max_message_bytes: 256 * 1024,
            max_sessions: 10_000,
        }
    }
}

/// Opaque token a client presents to resume a previously registered session.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResumeToken(String);

impl ResumeToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for ResumeToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

struct RegistryEntry<T> {
    value: T,
    created_at: u64,
    last_active_at: u64,
}

impl<T> RegistryEntry<T> {
    fn expired(&self, now: u64, config: &LiveViewConfig) -> bool {
        let idle_expired = config.idle_timeout_secs > 0 && now.saturating_sub(self.last_active_at) >= config.idle_timeout_secs;
        let lifetime_expired = config.max_lifetime_secs > 0 && now.saturating_sub(self.created_at) >= config.max_lifetime_secs;
        idle_expired || lifetime_expired
    }
}

/// Registry of live sessions keyed by a [`ResumeToken`], with idle/TTL reaping.
///
/// Framework- and clock-agnostic: every method needing the current time takes
/// `now` (unix seconds) explicitly, so reaping/resume semantics are
/// deterministic and unit-testable. Adapters pass [`current_unix_secs`]. `T` is
/// the per-session state; no `Send` bound is imposed so the `!Send` reactive
/// tree can live inside it.
pub struct SessionRegistry<T> {
    config: LiveViewConfig,
    next_id: u64,
    entries: std::collections::HashMap<String, RegistryEntry<T>>,
}

impl<T> SessionRegistry<T> {
    pub fn new(config: LiveViewConfig) -> Self {
        Self {
            config,
            next_id: 0,
            entries: std::collections::HashMap::new(),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(LiveViewConfig::default())
    }

    pub fn config(&self) -> LiveViewConfig {
        self.config
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Register a new session, returning a monotonic resume token. For
    /// unguessable tokens (recommended in production) supply your own via
    /// [`SessionRegistry::insert_with_token`].
    pub fn insert(&mut self, now: u64, value: T) -> ResumeToken {
        let id = self.next_id;
        self.next_id += 1;
        self.insert_with_token(now, format!("lv-{id}"), value)
    }

    /// Whether the registry is at [`LiveViewConfig::max_sessions`].
    ///
    /// Callers should reap first and refuse the connection if this still holds;
    /// admitting anyway would let a client with no valid token pin memory.
    pub fn is_full(&self) -> bool {
        self.config.max_sessions > 0 && self.entries.len() >= self.config.max_sessions
    }

    /// Register a session under a caller-supplied token. Overwrites any existing
    /// entry with the same token.
    pub fn insert_with_token(&mut self, now: u64, token: impl Into<String>, value: T) -> ResumeToken {
        let token = token.into();
        self.entries.insert(
            token.clone(),
            RegistryEntry {
                value,
                created_at: now,
                last_active_at: now,
            },
        );
        ResumeToken(token)
    }

    /// Resume a session by token if it is still alive at `now`, refreshing its
    /// activity timestamp. Expired entries are removed and `None` is returned.
    pub fn resume(&mut self, now: u64, token: &str) -> Option<&mut T> {
        match self.entries.get(token) {
            Some(entry) if entry.expired(now, &self.config) => {
                self.entries.remove(token);
                None
            }
            Some(_) => {
                let entry = self.entries.get_mut(token).expect("entry present");
                entry.last_active_at = now;
                Some(&mut entry.value)
            }
            None => None,
        }
    }

    /// Refresh a session's activity timestamp. Returns `false` if unknown.
    pub fn touch(&mut self, now: u64, token: &str) -> bool {
        match self.entries.get_mut(token) {
            Some(entry) => {
                entry.last_active_at = now;
                true
            }
            None => false,
        }
    }

    /// Remove a session, returning its state if present.
    pub fn remove(&mut self, token: &str) -> Option<T> {
        self.entries.remove(token).map(|entry| entry.value)
    }

    /// Remove and return every session that is idle or past its max lifetime
    /// at `now`. Adapters should call this periodically.
    pub fn reap(&mut self, now: u64) -> Vec<(ResumeToken, T)> {
        let config = self.config;
        let expired: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.expired(now, &config))
            .map(|(token, _)| token.clone())
            .collect();
        expired
            .into_iter()
            .filter_map(|token| self.entries.remove(&token).map(|entry| (ResumeToken(token), entry.value)))
            .collect()
    }
}

/// Current wall-clock time in unix seconds, for feeding the time-injected
/// [`SessionRegistry`] methods from adapter code.
pub fn current_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Router abstraction for frameworks that can mount a Glory LiveView websocket
/// endpoint.
pub trait LiveviewRouter {
    fn create_default_liveview_router() -> Self;

    fn with_liveview<W>(self, path: &str, widget: impl Fn() -> W + Send + Sync + 'static) -> Self
    where
        Self: Sized,
        W: Widget + 'static;
}

#[cfg(any(feature = "axum", feature = "actix"))]
fn normalize_liveview_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        LIVEVIEW_DEFAULT_PATH.to_owned()
    } else if trimmed.starts_with('/') {
        trimmed.to_owned()
    } else {
        format!("/{trimmed}")
    }
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
fn negotiate_hello(message: LiveViewMessage) -> Result<Option<String>, LiveViewMessage> {
    match message {
        LiveViewMessage::Hello {
            protocol_version,
            resume_token,
        } if protocol_version == LIVEVIEW_PROTOCOL_VERSION => Ok(resume_token.filter(|token| !token.is_empty())),
        LiveViewMessage::Hello { protocol_version, .. } => Err(LiveViewMessage::Error {
            message: format!("liveview protocol mismatch: client {protocol_version}, server {LIVEVIEW_PROTOCOL_VERSION}"),
        }),
        _ => Err(LiveViewMessage::Error {
            message: "first liveview message must be hello".to_owned(),
        }),
    }
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
#[derive(Clone)]
struct SessionManager {
    registry: std::sync::Arc<std::sync::Mutex<SessionRegistry<SessionWorker>>>,
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
struct SessionConnection {
    worker: SessionWorker,
    token: ResumeToken,
    mount: Option<LiveViewMessage>,
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
impl SessionManager {
    fn new(config: LiveViewConfig) -> Self {
        Self {
            registry: std::sync::Arc::new(std::sync::Mutex::new(SessionRegistry::new(config))),
        }
    }

    fn with_default_config() -> Self {
        Self::new(LiveViewConfig::default())
    }

    async fn connect<W>(
        &self,
        factory: std::sync::Arc<dyn Fn() -> W + Send + Sync + 'static>,
        resume_token: Option<String>,
    ) -> Result<SessionConnection, ()>
    where
        W: Widget + 'static,
    {
        let now = current_unix_secs();
        if let Some(token) = resume_token {
            let worker = {
                let mut registry = self.registry.lock().expect("liveview session registry poisoned");
                registry.reap(now);
                registry.resume(now, &token).cloned()
            };
            if let Some(worker) = worker {
                return Ok(SessionConnection {
                    worker,
                    token: ResumeToken(token),
                    mount: None,
                });
            }
        } else {
            self.reap();
        }

        // Refuse rather than admit past the cap: a new session allocates a
        // widget tree that survives this socket.
        {
            let mut registry = self.registry.lock().expect("liveview session registry poisoned");
            registry.reap(now);
            if registry.is_full() {
                return Err(());
            }
        }

        let (worker, mount) = SessionWorker::spawn(factory).await?;
        let token = self.insert_worker(worker.clone());
        Ok(SessionConnection {
            worker,
            token,
            mount: Some(mount),
        })
    }

    /// Drop a session outright, used when the client says goodbye.
    fn remove(&self, token: &str) {
        self.registry.lock().expect("liveview session registry poisoned").remove(token);
    }

    fn config(&self) -> LiveViewConfig {
        self.registry.lock().expect("liveview session registry poisoned").config
    }

    fn insert_worker(&self, worker: SessionWorker) -> ResumeToken {
        let now = current_unix_secs();
        let token = new_resume_token();
        self.registry
            .lock()
            .expect("liveview session registry poisoned")
            .insert_with_token(now, token, worker)
    }

    fn touch(&self, token: &str) -> bool {
        let now = current_unix_secs();
        let mut registry = self.registry.lock().expect("liveview session registry poisoned");
        let touched = registry.touch(now, token);
        let expired = registry.reap(now).into_iter().any(|(reaped, _)| reaped.as_str() == token);
        touched && !expired
    }

    fn reap(&self) {
        let now = current_unix_secs();
        self.registry.lock().expect("liveview session registry poisoned").reap(now);
    }
}

/// Whether a websocket handshake carrying `origin` (with the request's own
/// `host`) may open a LiveView session.
///
/// A handshake without an `Origin` header is allowed: that is a non-browser
/// client, which has no ambient cookies to abuse.
pub fn origin_is_allowed(config: &LiveViewConfig, origin: Option<&str>, host: Option<&str>) -> bool {
    let Some(origin) = origin.map(str::trim) else {
        return true;
    };
    if origin.eq_ignore_ascii_case("null") {
        return false;
    }
    if config.allowed_origins.iter().any(|allowed| allowed.eq_ignore_ascii_case(origin)) {
        return true;
    }
    match (origin.split_once("://"), host) {
        (Some((_, authority)), Some(host)) => authority.eq_ignore_ascii_case(host.trim()),
        _ => false,
    }
}

/// Generates a resume token from the OS CSPRNG.
///
/// A resume token is a bearer credential: whoever presents it takes over the
/// session, including everything its widget tree can read and every handler it
/// can run. It was previously built from the clock, the pid and a counter, all
/// of which an attacker can narrow down, making tokens guessable. 128 bits of
/// `getrandom` entropy removes that.
#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
fn new_resume_token() -> String {
    let mut bytes = [0u8; 16];
    if getrandom::fill(&mut bytes).is_err() {
        // The OS entropy source is unavailable. Refusing to hand out a
        // predictable token is the only safe answer, so make it obvious.
        panic!("liveview: OS randomness is unavailable; cannot mint a resume token");
    }
    let mut token = String::with_capacity(3 + bytes.len() * 2);
    token.push_str("lv-");
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut token, "{byte:02x}");
    }
    token
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
#[derive(Clone)]
struct SessionWorker {
    sender: futures::channel::mpsc::Sender<SessionRequest>,
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
enum SessionRequest {
    Message {
        message: LiveViewMessage,
        reply: futures::channel::oneshot::Sender<Option<LiveViewMessage>>,
    },
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
type LocalTask = Box<dyn FnOnce(futures::executor::LocalSpawner) + Send + 'static>;

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
struct LocalWorkerPool {
    senders: Vec<futures::channel::mpsc::UnboundedSender<LocalTask>>,
    next: std::sync::atomic::AtomicUsize,
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
impl LocalWorkerPool {
    fn new() -> Self {
        let count = liveview_worker_count();
        let mut senders = Vec::with_capacity(count);
        for index in 0..count {
            let (sender, receiver) = futures::channel::mpsc::unbounded::<LocalTask>();
            std::thread::Builder::new()
                .name(format!("glory-liveview-local-{index}"))
                .spawn(move || run_local_worker(receiver))
                .expect("glory-liveview: failed to spawn local worker");
            senders.push(sender);
        }
        Self {
            senders,
            next: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn spawn(&self, task: LocalTask) -> Result<(), ()> {
        let index = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % self.senders.len();
        self.senders[index].unbounded_send(task).map_err(|_| ())
    }
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
fn liveview_worker_pool() -> &'static LocalWorkerPool {
    static POOL: std::sync::OnceLock<LocalWorkerPool> = std::sync::OnceLock::new();
    POOL.get_or_init(LocalWorkerPool::new)
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
fn liveview_worker_count() -> usize {
    std::env::var("GLORY_LIVEVIEW_WORKERS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map(|value| value.get()).unwrap_or(1).min(4))
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
fn run_local_worker(mut receiver: futures::channel::mpsc::UnboundedReceiver<LocalTask>) {
    use futures::StreamExt;
    use futures::task::LocalSpawnExt;

    let mut pool = futures::executor::LocalPool::new();
    let spawner = pool.spawner();
    let dispatch_spawner = spawner.clone();
    spawner
        .spawn_local(async move {
            while let Some(task) = receiver.next().await {
                task(dispatch_spawner.clone());
            }
        })
        .expect("glory-liveview: failed to spawn local dispatcher");
    pool.run();
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
impl SessionWorker {
    async fn spawn<W>(factory: std::sync::Arc<dyn Fn() -> W + Send + Sync + 'static>) -> Result<(Self, LiveViewMessage), ()>
    where
        W: Widget + 'static,
    {
        let (sender, receiver) = futures::channel::mpsc::channel(32);
        let (mount_sender, mount_receiver) = futures::channel::oneshot::channel();
        liveview_worker_pool().spawn(Box::new(move |spawner| {
            use futures::task::LocalSpawnExt;

            spawner
                .spawn_local(run_session(factory, receiver, mount_sender))
                .expect("glory-liveview: failed to spawn session task");
        }))?;
        let mount = mount_receiver.await.map_err(|_| ())?;
        Ok((Self { sender }, mount))
    }

    async fn handle_message(&self, message: LiveViewMessage) -> Option<LiveViewMessage> {
        use futures::SinkExt;

        let (reply, receiver) = futures::channel::oneshot::channel();
        let mut sender = self.sender.clone();
        if sender.send(SessionRequest::Message { message, reply }).await.is_err() {
            return Some(LiveViewMessage::Error {
                message: "liveview session worker stopped".to_owned(),
            });
        }
        receiver.await.unwrap_or_else(|_| {
            Some(LiveViewMessage::Error {
                message: "liveview session worker dropped a response".to_owned(),
            })
        })
    }
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
async fn run_session<W>(
    factory: std::sync::Arc<dyn Fn() -> W + Send + Sync + 'static>,
    mut receiver: futures::channel::mpsc::Receiver<SessionRequest>,
    mount_sender: futures::channel::oneshot::Sender<LiveViewMessage>,
) where
    W: Widget + 'static,
{
    use futures::StreamExt;

    let (session, mount) = LiveViewSession::mount(factory());
    if mount_sender.send(mount).is_err() {
        return;
    }
    while let Some(request) = receiver.next().await {
        match request {
            SessionRequest::Message { message, reply } => {
                let _ = reply.send(session.handle_message(message));
            }
        }
    }
}

#[cfg(feature = "salvo")]
pub mod salvo_mount {
    use std::sync::Arc;

    use futures::StreamExt;
    use glory_core::Widget;
    use salvo::prelude::{Depot, FlowCtrl, Request, Response, Router};
    use salvo::websocket::{Message, WebSocket, WebSocketUpgrade};
    use salvo::{Handler, async_trait};

    use crate::{LIVEVIEW_DEFAULT_PATH, LiveViewConfig, LiveViewMessage, SessionManager, negotiate_hello};

    pub fn router<W>(widget: impl Fn() -> W + Send + Sync + 'static) -> Router
    where
        W: Widget + 'static,
    {
        router_with_config(widget, LiveViewConfig::default())
    }

    pub fn router_with_config<W>(widget: impl Fn() -> W + Send + Sync + 'static, config: LiveViewConfig) -> Router
    where
        W: Widget + 'static,
    {
        let factory: Arc<dyn Fn() -> W + Send + Sync + 'static> = Arc::new(widget);
        let sessions = SessionManager::new(config);
        Router::with_path(LIVEVIEW_DEFAULT_PATH.trim_start_matches('/')).get(LiveViewHandler { factory, sessions })
    }

    struct LiveViewHandler<W>
    where
        W: Widget + 'static,
    {
        factory: Arc<dyn Fn() -> W + Send + Sync + 'static>,
        sessions: SessionManager,
    }

    #[async_trait]
    impl<W> Handler for LiveViewHandler<W>
    where
        W: Widget + 'static,
    {
        async fn handle(&self, req: &mut Request, _depot: &mut Depot, res: &mut Response, _ctrl: &mut FlowCtrl) {
            let factory = self.factory.clone();
            let sessions = self.sessions.clone();
            let config = self.sessions.config();
            let header = |name: &str| req.headers().get(name).and_then(|value| value.to_str().ok());
            if !crate::origin_is_allowed(&config, header("origin"), header("host")) {
                res.status_code(salvo::http::StatusCode::FORBIDDEN);
                res.render("cross-site liveview connections are not allowed");
                return;
            }
            let mut upgrade = WebSocketUpgrade::new();
            let max_message_bytes = config.max_message_bytes;
            if max_message_bytes > 0 {
                // Otherwise tungstenite's 64 MiB default applies, and a single
                // client frame can force that much allocation and parsing.
                upgrade = upgrade.max_message_size(max_message_bytes);
            }
            if let Err(err) = upgrade.upgrade(req, res, move |socket| handle_socket(socket, factory, sessions)).await {
                res.render(err);
            }
        }
    }

    async fn handle_socket<W>(mut socket: WebSocket, factory: Arc<dyn Fn() -> W + Send + Sync + 'static>, sessions: SessionManager)
    where
        W: Widget + 'static,
    {
        let Some(resume_token) = read_hello(&mut socket).await else {
            return;
        };
        let Ok(connection) = sessions.connect(factory, resume_token).await else {
            let _ = send_error(&mut socket, "liveview session worker failed to mount").await;
            return;
        };
        let worker = connection.worker;
        let token = connection.token.to_string();
        if let Some(mount) = connection.mount {
            if send(&mut socket, LiveViewMessage::Session { token: token.clone() }).await.is_err() || send(&mut socket, mount).await.is_err() {
                return;
            }
        } else if send(&mut socket, LiveViewMessage::Resumed { token: token.clone() }).await.is_err() {
            return;
        }

        while let Some(message) = socket.next().await {
            let Ok(message) = message else {
                break;
            };
            if !message.is_text() {
                continue;
            }
            let Ok(text) = message.as_str() else {
                let _ = send_error(&mut socket, "invalid UTF-8 websocket message").await;
                continue;
            };
            match LiveViewMessage::from_json(text) {
                Ok(message) => {
                    if !sessions.touch(&token) {
                        let _ = send(
                            &mut socket,
                            LiveViewMessage::Close {
                                reason: "liveview session expired".to_owned(),
                            },
                        )
                        .await;
                        break;
                    }
                    // A client saying goodbye releases its session now,
                    // rather than pinning a widget tree until the idle
                    // timeout expires.
                    let closing = matches!(message, LiveViewMessage::Close { .. });
                    if let Some(reply) = worker.handle_message(message).await
                        && send(&mut socket, reply).await.is_err()
                    {
                        break;
                    }
                    if closing {
                        sessions.remove(&token);
                        break;
                    }
                }
                Err(err) => {
                    if send_error(&mut socket, format!("invalid liveview message: {err}")).await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    async fn read_hello(socket: &mut WebSocket) -> Option<Option<String>> {
        while let Some(message) = socket.next().await {
            let Ok(message) = message else {
                return None;
            };
            if !message.is_text() {
                continue;
            }
            let Ok(text) = message.as_str() else {
                let _ = send_error(socket, "invalid UTF-8 websocket message").await;
                return None;
            };
            let message = match LiveViewMessage::from_json(text) {
                Ok(message) => message,
                Err(err) => {
                    let _ = send_error(socket, format!("invalid liveview message: {err}")).await;
                    return None;
                }
            };
            return match negotiate_hello(message) {
                Ok(token) => Some(token),
                Err(message) => {
                    let _ = send(socket, message).await;
                    None
                }
            };
        }
        None
    }

    async fn send(socket: &mut WebSocket, message: LiveViewMessage) -> Result<(), salvo::Error> {
        socket.send(Message::text(message.to_json().expect("liveview messages serialize"))).await
    }

    async fn send_error(socket: &mut WebSocket, message: impl Into<String>) -> Result<(), salvo::Error> {
        send(socket, LiveViewMessage::Error { message: message.into() }).await
    }
}

#[cfg(feature = "axum")]
pub mod axum_mount {
    use std::sync::Arc;

    use axum::Router;
    use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
    use axum::response::IntoResponse;
    use axum::routing::get;
    use futures::StreamExt;
    use glory_core::Widget;

    use crate::{LIVEVIEW_DEFAULT_PATH, LiveViewConfig, LiveViewMessage, LiveviewRouter, SessionManager, negotiate_hello, normalize_liveview_path};

    pub fn router<W>(widget: impl Fn() -> W + Send + Sync + 'static) -> Router
    where
        W: Widget + 'static,
    {
        Router::new().with_liveview(LIVEVIEW_DEFAULT_PATH, widget)
    }

    pub fn router_with_config<W>(widget: impl Fn() -> W + Send + Sync + 'static, config: LiveViewConfig) -> Router
    where
        W: Widget + 'static,
    {
        route_with_config(Router::new(), LIVEVIEW_DEFAULT_PATH, widget, config)
    }

    pub fn route<S, W>(router: Router<S>, path: &str, widget: impl Fn() -> W + Send + Sync + 'static) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
        W: Widget + 'static,
    {
        router.with_liveview(path, widget)
    }

    pub fn route_with_config<S, W>(router: Router<S>, path: &str, widget: impl Fn() -> W + Send + Sync + 'static, config: LiveViewConfig) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
        W: Widget + 'static,
    {
        let factory: Arc<dyn Fn() -> W + Send + Sync + 'static> = Arc::new(widget);
        let sessions = SessionManager::new(config);
        router.route(
            &normalize_liveview_path(path),
            get(move |headers: axum::http::HeaderMap, ws: WebSocketUpgrade| {
                let factory = factory.clone();
                let sessions = sessions.clone();
                async move {
                    let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
                    // Browsers do not apply the same-origin policy to
                    // websocket handshakes, so without this any site could
                    // open a session with the user's cookies.
                    if !crate::origin_is_allowed(&sessions.config(), header("origin"), header("host")) {
                        return IntoResponse::into_response(axum::http::StatusCode::FORBIDDEN);
                    }
                    IntoResponse::into_response(ws.on_upgrade(move |socket| handle_socket(socket, factory, sessions)))
                }
            }),
        )
    }

    impl<S> LiveviewRouter for Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        fn create_default_liveview_router() -> Self {
            Router::new()
        }

        fn with_liveview<W>(self, path: &str, widget: impl Fn() -> W + Send + Sync + 'static) -> Self
        where
            W: Widget + 'static,
        {
            route_with_config(self, path, widget, LiveViewConfig::default())
        }
    }

    async fn handle_socket<W>(mut socket: WebSocket, factory: Arc<dyn Fn() -> W + Send + Sync + 'static>, sessions: SessionManager)
    where
        W: Widget + 'static,
    {
        let Some(resume_token) = read_hello(&mut socket).await else {
            return;
        };
        let Ok(connection) = sessions.connect(factory, resume_token).await else {
            let _ = send_error(&mut socket, "liveview session worker failed to mount").await;
            return;
        };
        let worker = connection.worker;
        let token = connection.token.to_string();
        if let Some(mount) = connection.mount {
            if send(&mut socket, LiveViewMessage::Session { token: token.clone() }).await.is_err() || send(&mut socket, mount).await.is_err() {
                return;
            }
        } else if send(&mut socket, LiveViewMessage::Resumed { token: token.clone() }).await.is_err() {
            return;
        }

        while let Some(message) = socket.next().await {
            let Ok(message) = message else {
                break;
            };
            let Message::Text(text) = message else {
                continue;
            };
            match LiveViewMessage::from_json(text.as_str()) {
                Ok(message) => {
                    if !sessions.touch(&token) {
                        let _ = send(
                            &mut socket,
                            LiveViewMessage::Close {
                                reason: "liveview session expired".to_owned(),
                            },
                        )
                        .await;
                        break;
                    }
                    // A client saying goodbye releases its session now,
                    // rather than pinning a widget tree until the idle
                    // timeout expires.
                    let closing = matches!(message, LiveViewMessage::Close { .. });
                    if let Some(reply) = worker.handle_message(message).await
                        && send(&mut socket, reply).await.is_err()
                    {
                        break;
                    }
                    if closing {
                        sessions.remove(&token);
                        break;
                    }
                }
                Err(err) => {
                    if send_error(&mut socket, format!("invalid liveview message: {err}")).await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    async fn read_hello(socket: &mut WebSocket) -> Option<Option<String>> {
        while let Some(message) = socket.next().await {
            let Ok(message) = message else {
                return None;
            };
            let Message::Text(text) = message else {
                continue;
            };
            let message = match LiveViewMessage::from_json(text.as_str()) {
                Ok(message) => message,
                Err(err) => {
                    let _ = send_error(socket, format!("invalid liveview message: {err}")).await;
                    return None;
                }
            };
            return match negotiate_hello(message) {
                Ok(token) => Some(token),
                Err(message) => {
                    let _ = send(socket, message).await;
                    None
                }
            };
        }
        None
    }

    async fn send(socket: &mut WebSocket, message: LiveViewMessage) -> Result<(), axum::Error> {
        socket.send(Message::text(message.to_json().expect("liveview messages serialize"))).await
    }

    async fn send_error(socket: &mut WebSocket, message: impl Into<String>) -> Result<(), axum::Error> {
        send(socket, LiveViewMessage::Error { message: message.into() }).await
    }
}

#[cfg(feature = "actix")]
pub mod actix_mount {
    use std::sync::Arc;

    use actix_web::{HttpRequest, HttpResponse, Scope, web};
    use glory_core::Widget;

    use crate::{LIVEVIEW_DEFAULT_PATH, LiveViewConfig, LiveViewMessage, LiveviewRouter, SessionManager, negotiate_hello, normalize_liveview_path};

    pub fn scope<W>(widget: impl Fn() -> W + Send + Sync + 'static) -> Scope
    where
        W: Widget + 'static,
    {
        web::scope("").with_liveview(LIVEVIEW_DEFAULT_PATH, widget)
    }

    pub fn scope_with_config<W>(widget: impl Fn() -> W + Send + Sync + 'static, config: LiveViewConfig) -> Scope
    where
        W: Widget + 'static,
    {
        let factory: Arc<dyn Fn() -> W + Send + Sync + 'static> = Arc::new(widget);
        let sessions = SessionManager::new(config);
        web::scope("").route(
            LIVEVIEW_DEFAULT_PATH,
            web::get().to(move |req: HttpRequest, body: web::Payload| {
                let factory = factory.clone();
                let sessions = sessions.clone();
                async move { handler(req, body, factory, sessions).await }
            }),
        )
    }

    pub fn configure<W>(cfg: &mut web::ServiceConfig, widget: impl Fn() -> W + Send + Sync + 'static)
    where
        W: Widget + 'static,
    {
        configure_with_config(cfg, widget, LiveViewConfig::default());
    }

    pub fn configure_with_config<W>(cfg: &mut web::ServiceConfig, widget: impl Fn() -> W + Send + Sync + 'static, config: LiveViewConfig)
    where
        W: Widget + 'static,
    {
        let factory: Arc<dyn Fn() -> W + Send + Sync + 'static> = Arc::new(widget);
        let sessions = SessionManager::new(config);
        cfg.route(
            LIVEVIEW_DEFAULT_PATH,
            web::get().to(move |req: HttpRequest, body: web::Payload| {
                let factory = factory.clone();
                let sessions = sessions.clone();
                async move { handler(req, body, factory, sessions).await }
            }),
        );
    }

    impl LiveviewRouter for Scope {
        fn create_default_liveview_router() -> Self {
            web::scope("")
        }

        fn with_liveview<W>(self, path: &str, widget: impl Fn() -> W + Send + Sync + 'static) -> Self
        where
            W: Widget + 'static,
        {
            let factory: Arc<dyn Fn() -> W + Send + Sync + 'static> = Arc::new(widget);
            let sessions = SessionManager::with_default_config();
            self.route(
                &normalize_liveview_path(path),
                web::get().to(move |req: HttpRequest, body: web::Payload| {
                    let factory = factory.clone();
                    let sessions = sessions.clone();
                    async move { handler(req, body, factory, sessions).await }
                }),
            )
        }
    }

    async fn handler<W>(
        req: HttpRequest,
        body: web::Payload,
        factory: Arc<dyn Fn() -> W + Send + Sync + 'static>,
        sessions: SessionManager,
    ) -> actix_web::Result<HttpResponse>
    where
        W: Widget + 'static,
    {
        let header = |name: &str| req.headers().get(name).and_then(|value| value.to_str().ok());
        // See the other adapters: websocket handshakes are not same-origin
        // protected by the browser.
        if !crate::origin_is_allowed(&sessions.config(), header("origin"), header("host")) {
            return Ok(HttpResponse::Forbidden().body("cross-site liveview connections are not allowed"));
        }
        let (response, session, stream) = actix_ws::handle(&req, body)?;
        actix_web::rt::spawn(handle_socket(session, stream, factory, sessions));
        Ok(response)
    }

    async fn handle_socket<W>(
        mut session: actix_ws::Session,
        mut stream: actix_ws::MessageStream,
        factory: Arc<dyn Fn() -> W + Send + Sync + 'static>,
        sessions: SessionManager,
    ) where
        W: Widget + 'static,
    {
        let Some(resume_token) = read_hello(&mut session, &mut stream).await else {
            let _ = session.close(None).await;
            return;
        };
        let Ok(connection) = sessions.connect(factory, resume_token).await else {
            let _ = send_error(&mut session, "liveview session worker failed to mount").await;
            return;
        };
        let worker = connection.worker;
        let token = connection.token.to_string();
        if let Some(mount) = connection.mount {
            if send(&mut session, LiveViewMessage::Session { token: token.clone() }).await.is_err() || send(&mut session, mount).await.is_err() {
                return;
            }
        } else if send(&mut session, LiveViewMessage::Resumed { token: token.clone() }).await.is_err() {
            return;
        }

        while let Some(message) = stream.recv().await {
            let Ok(message) = message else {
                break;
            };
            match message {
                actix_ws::Message::Text(text) => match LiveViewMessage::from_json(text.as_ref()) {
                    Ok(message) => {
                        if !sessions.touch(&token) {
                            let _ = send(
                                &mut session,
                                LiveViewMessage::Close {
                                    reason: "liveview session expired".to_owned(),
                                },
                            )
                            .await;
                            break;
                        }
                        // See the other adapters: a client `Close` releases
                        // the session immediately.
                        let closing = matches!(message, LiveViewMessage::Close { .. });
                        if let Some(reply) = worker.handle_message(message).await
                            && send(&mut session, reply).await.is_err()
                        {
                            break;
                        }
                        if closing {
                            sessions.remove(&token);
                            break;
                        }
                    }
                    Err(err) => {
                        if send_error(&mut session, format!("invalid liveview message: {err}")).await.is_err() {
                            break;
                        }
                    }
                },
                actix_ws::Message::Ping(bytes) => {
                    if session.pong(&bytes).await.is_err() {
                        break;
                    }
                }
                actix_ws::Message::Close(_) => break,
                _ => {}
            }
        }

        let _ = session.close(None).await;
    }

    async fn read_hello(session: &mut actix_ws::Session, stream: &mut actix_ws::MessageStream) -> Option<Option<String>> {
        while let Some(message) = stream.recv().await {
            let Ok(message) = message else {
                return None;
            };
            match message {
                actix_ws::Message::Text(text) => {
                    let message = match LiveViewMessage::from_json(text.as_ref()) {
                        Ok(message) => message,
                        Err(err) => {
                            let _ = send_error(session, format!("invalid liveview message: {err}")).await;
                            return None;
                        }
                    };
                    return match negotiate_hello(message) {
                        Ok(token) => Some(token),
                        Err(message) => {
                            let _ = send(session, message).await;
                            None
                        }
                    };
                }
                actix_ws::Message::Ping(bytes) => {
                    if session.pong(&bytes).await.is_err() {
                        return None;
                    }
                }
                actix_ws::Message::Close(_) => return None,
                _ => {}
            }
        }
        None
    }

    async fn send(session: &mut actix_ws::Session, message: LiveViewMessage) -> Result<(), actix_ws::Closed> {
        session.text(message.to_json().expect("liveview messages serialize")).await
    }

    async fn send_error(session: &mut actix_ws::Session, message: impl Into<String>) -> Result<(), actix_ws::Closed> {
        send(session, LiveViewMessage::Error { message: message.into() }).await
    }
}

#[cfg(test)]
mod tests {
    use glory_core::reflow::Cage;
    use glory_core::renderer::{Command, NodeQuery, QueryResponse, QueryValue};
    use glory_core::web::events;
    use glory_core::web::widgets::{button, div};
    use glory_core::{Scope, Widget};

    use super::*;

    #[derive(Debug)]
    struct Counter {
        value: Cage<i32>,
    }

    impl Widget for Counter {
        fn build(&mut self, ctx: &mut Scope) {
            let value = self.value;
            let increment = move |_| value.revise(|mut value| *value += 1);

            div()
                .fill(button().text("count").on(events::click, increment))
                .fill(div().text(self.value))
                .show_in(ctx);
        }
    }

    #[test]
    fn hello_version_negotiation() {
        let (session, _mount) = LiveViewSession::mount(Counter { value: Cage::new(0) });
        assert!(session.handle_message(LiveViewMessage::hello()).is_none());
        let reply = session.handle_message(LiveViewMessage::Hello {
            protocol_version: 999,
            resume_token: None,
        });
        assert!(matches!(reply, Some(LiveViewMessage::Error { .. })));
    }

    #[test]
    fn registry_resume_refreshes_activity_and_survives_idle() {
        let config = LiveViewConfig {
            idle_timeout_secs: 100,
            max_lifetime_secs: 0,
            ..LiveViewConfig::default()
        };
        let mut registry: SessionRegistry<i32> = SessionRegistry::new(config);
        let token = registry.insert(0, 7);

        assert_eq!(registry.resume(90, token.as_str()).copied(), Some(7));
        assert_eq!(registry.resume(150, token.as_str()).copied(), Some(7));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn registry_reaps_idle_and_expired_sessions() {
        let config = LiveViewConfig {
            idle_timeout_secs: 50,
            max_lifetime_secs: 200,
            ..LiveViewConfig::default()
        };
        let mut registry: SessionRegistry<&str> = SessionRegistry::new(config);
        let idle = registry.insert(0, "idle");
        let active = registry.insert(0, "active");

        assert!(registry.touch(40, active.as_str()));

        let reaped = registry.reap(60);
        assert_eq!(reaped.len(), 1);
        assert_eq!(reaped[0].0, idle);
        assert_eq!(reaped[0].1, "idle");
        assert_eq!(registry.len(), 1);

        assert!(registry.resume(60, idle.as_str()).is_none());

        assert!(registry.touch(199, active.as_str()));
        let reaped = registry.reap(200);
        assert_eq!(reaped.len(), 1);
        assert_eq!(reaped[0].1, "active");
        assert!(registry.is_empty());
    }

    #[test]
    fn registry_resume_drops_expired_entry() {
        let mut registry: SessionRegistry<u8> = SessionRegistry::new(LiveViewConfig {
            idle_timeout_secs: 10,
            max_lifetime_secs: 0,
            ..LiveViewConfig::default()
        });
        let token = registry.insert(0, 1);
        assert!(registry.resume(50, token.as_str()).is_none());
        assert!(registry.is_empty(), "expired entry removed on failed resume");
    }

    #[test]
    fn outbound_buffer_keeps_batches_below_cap() {
        let mut buf = OutboundBuffer::new(3);
        buf.push(vec![Command::Remove { parent: 0, child: 1 }]);
        buf.push(vec![Command::Remove { parent: 0, child: 2 }]);
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.command_count(), 2);
        assert_eq!(buf.drain_one().unwrap().len(), 1);
        assert_eq!(buf.len(), 1);
    }

    #[test]
    fn outbound_buffer_coalesces_over_cap_preserving_commands() {
        let mut buf = OutboundBuffer::new(2);
        buf.push(vec![Command::Remove { parent: 0, child: 1 }]);
        buf.push(vec![Command::Remove { parent: 0, child: 2 }]);
        // Third push exceeds the cap of 2 → folds into the tail batch.
        buf.push(vec![Command::Remove { parent: 0, child: 3 }, Command::Remove { parent: 0, child: 4 }]);
        assert_eq!(buf.len(), 2, "still capped at 2 batches");
        assert_eq!(buf.command_count(), 4, "no commands dropped");
        let batches = buf.take();
        assert_eq!(batches[0].len(), 1);
        assert_eq!(batches[1].len(), 3, "tail absorbed the overflow batch");
    }

    #[test]
    fn outbound_buffer_unbounded_and_empty_behavior() {
        let mut buf = OutboundBuffer::new(0);
        for id in 0..10 {
            buf.push(vec![Command::Remove { parent: 0, child: id }]);
        }
        assert_eq!(buf.len(), 10, "max_pending 0 = unbounded");
        buf.push(Vec::new()); // empty batch ignored
        assert_eq!(buf.len(), 10);
        let mut empty = OutboundBuffer::new(4);
        assert!(empty.is_empty());
        assert!(empty.drain_one().is_none());
    }

    #[test]
    fn liveview_message_round_trips_json() {
        let message = LiveViewMessage::hello();
        let json = message.to_json().unwrap();
        assert_eq!(LiveViewMessage::from_json(&json).unwrap(), message);

        let message = LiveViewMessage::hello_with_resume("session-1");
        let json = message.to_json().unwrap();
        assert_eq!(LiveViewMessage::from_json(&json).unwrap(), message);
    }

    #[test]
    fn client_script_exposes_reconnect_entrypoint() {
        assert!(LIVEVIEW_CLIENT_JS.contains("__gloryLiveViewConnect"));
        assert!(LIVEVIEW_CLIENT_JS.contains("__gloryWryEvent"));
        assert!(LIVEVIEW_CLIENT_JS.contains("__gloryWryQuery"));
        assert!(LIVEVIEW_CLIENT_JS.contains("resumeToken"));
        assert!(LIVEVIEW_CLIENT_JS.contains("onSessionToken"));
        assert!(LIVEVIEW_CLIENT_JS.contains("setTimeout(connect"));
    }

    #[test]
    fn session_mount_and_event_emit_command_patches() {
        let (session, mount) = LiveViewSession::mount(Counter { value: Cage::new(0) });
        let LiveViewMessage::Mount { commands } = mount else {
            panic!("expected mount message");
        };
        let button_id = commands
            .iter()
            .find_map(|command| match command {
                Command::Create { id, name, .. } if name == "button" => Some(*id),
                _ => None,
            })
            .expect("button command");
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, Command::AttachEvent { name, .. } if name == "click"))
        );

        let patch = session.dispatch_event(EventData::new("click", button_id));
        let LiveViewMessage::Patch { commands } = patch else {
            panic!("expected patch message");
        };
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, Command::SetText { value, .. } if value == "1"))
        );
    }

    #[test]
    fn session_handles_ping() {
        let (session, _) = LiveViewSession::mount(Counter { value: Cage::new(0) });
        assert_eq!(session.handle_message(LiveViewMessage::Ping), Some(LiveViewMessage::Pong));
    }

    #[test]
    fn session_resolves_query_message() {
        let (session, _) = LiveViewSession::mount(Counter { value: Cage::new(0) });
        let query = session.holder().renderer().query(session.holder().host_node(), NodeQuery::Value);
        let token = session
            .holder()
            .take_batch()
            .into_iter()
            .find_map(|command| match command {
                Command::Query {
                    token,
                    kind: NodeQuery::Value,
                    ..
                } => Some(token),
                _ => None,
            })
            .expect("query command emitted");

        let reply = session.handle_message(LiveViewMessage::Query(Box::new(QueryResponse {
            token,
            result: Ok(QueryValue::Value("live".to_owned())),
        })));
        assert_eq!(reply, Some(LiveViewMessage::Patch { commands: Vec::new() }));
        assert_eq!(futures::executor::block_on(query).unwrap(), QueryValue::Value("live".to_owned()));
    }

    #[test]
    #[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
    fn session_worker_runs_on_local_pool() {
        let (worker, mount) = futures::executor::block_on(SessionWorker::spawn(std::sync::Arc::new(|| Counter { value: Cage::new(0) })))
            .expect("session worker mounts");
        let LiveViewMessage::Mount { commands } = mount else {
            panic!("expected mount message");
        };
        let button_id = commands
            .iter()
            .find_map(|command| match command {
                Command::Create { id, name, .. } if name == "button" => Some(*id),
                _ => None,
            })
            .expect("button command");

        assert_eq!(
            futures::executor::block_on(worker.handle_message(LiveViewMessage::Ping)),
            Some(LiveViewMessage::Pong)
        );

        let reply = futures::executor::block_on(worker.handle_message(LiveViewMessage::Event(Box::new(EventData::new("click", button_id)))));
        let Some(LiveViewMessage::Patch { commands }) = reply else {
            panic!("expected patch message");
        };
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, Command::SetText { value, .. } if value == "1"))
        );
    }

    #[test]
    #[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
    fn session_manager_resumes_existing_worker() {
        let manager = SessionManager::with_default_config();
        let factory = std::sync::Arc::new(|| Counter { value: Cage::new(0) });

        let connection = futures::executor::block_on(manager.connect(factory.clone(), None)).expect("session connects");
        let token = connection.token.to_string();
        let LiveViewMessage::Mount { commands } = connection.mount.as_ref().expect("new session mounts") else {
            panic!("expected mount message");
        };
        let button_id = commands
            .iter()
            .find_map(|command| match command {
                Command::Create { id, name, .. } if name == "button" => Some(*id),
                _ => None,
            })
            .expect("button command");

        let reply = futures::executor::block_on(
            connection
                .worker
                .handle_message(LiveViewMessage::Event(Box::new(EventData::new("click", button_id)))),
        );
        let Some(LiveViewMessage::Patch { commands }) = reply else {
            panic!("expected patch message");
        };
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, Command::SetText { value, .. } if value == "1"))
        );

        let resumed = futures::executor::block_on(manager.connect(factory, Some(token.clone()))).expect("session resumes");
        assert_eq!(resumed.token.as_str(), token);
        assert!(resumed.mount.is_none(), "resume keeps the existing DOM mounted");

        let reply = futures::executor::block_on(
            resumed
                .worker
                .handle_message(LiveViewMessage::Event(Box::new(EventData::new("click", button_id)))),
        );
        let Some(LiveViewMessage::Patch { commands }) = reply else {
            panic!("expected patch message");
        };
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, Command::SetText { value, .. } if value == "2")),
            "resumed worker preserves reactive state"
        );
    }
    #[test]
    #[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
    fn resume_tokens_are_unpredictable() {
        // A resume token is a bearer credential. It used to be
        // `clock + pid + counter`, every part of which an attacker can narrow
        // down.
        let tokens: std::collections::HashSet<String> = (0..256).map(|_| new_resume_token()).collect();
        assert_eq!(tokens.len(), 256, "tokens must not repeat");
        for token in &tokens {
            let hex = token.strip_prefix("lv-").expect("tokens are prefixed");
            assert_eq!(hex.len(), 32, "128 bits of entropy: {token}");
            assert!(hex.chars().all(|c| c.is_ascii_hexdigit()), "{token}");
        }
        // Consecutive tokens must not be a counter apart.
        let a = new_resume_token();
        let b = new_resume_token();
        let differing = a.bytes().zip(b.bytes()).filter(|(x, y)| x != y).count();
        assert!(differing > 4, "consecutive tokens look sequential: {a} / {b}");
    }

    #[test]
    fn cross_site_websocket_handshakes_are_rejected() {
        // Browsers do not apply the same-origin policy to websocket
        // handshakes, so any page could previously open a LiveView session
        // carrying the user's cookies.
        let config = LiveViewConfig::default();
        assert!(!origin_is_allowed(&config, Some("https://evil.example"), Some("app.example.com")));
        assert!(!origin_is_allowed(&config, Some("null"), Some("app.example.com")));

        // Same-origin and non-browser clients are fine.
        assert!(origin_is_allowed(&config, Some("https://app.example.com"), Some("app.example.com")));
        assert!(origin_is_allowed(&config, None, Some("app.example.com")));

        // Explicitly configured origins are allowed across sites.
        let config = LiveViewConfig {
            allowed_origins: &["https://trusted.example"],
            ..LiveViewConfig::default()
        };
        assert!(origin_is_allowed(&config, Some("https://trusted.example"), Some("app.example.com")));
        assert!(!origin_is_allowed(&config, Some("https://evil.example"), Some("app.example.com")));
    }

    #[test]
    fn registry_refuses_sessions_past_the_cap() {
        // Sessions outlive the socket that created them, so an uncapped
        // registry lets repeated connect/disconnect pin widget trees until the
        // idle timeout finally collects them.
        let config = LiveViewConfig {
            max_sessions: 2,
            ..LiveViewConfig::default()
        };
        let mut registry = SessionRegistry::new(config);
        assert!(!registry.is_full());
        registry.insert_with_token(0, "a", 1);
        assert!(!registry.is_full());
        registry.insert_with_token(0, "b", 2);
        assert!(registry.is_full(), "registry must report the cap");

        registry.remove("a");
        assert!(!registry.is_full(), "removing a session frees a slot");
    }

    #[test]
    fn zero_max_sessions_means_unbounded() {
        let config = LiveViewConfig {
            max_sessions: 0,
            ..LiveViewConfig::default()
        };
        let mut registry = SessionRegistry::new(config);
        for index in 0..64 {
            registry.insert_with_token(0, format!("t{index}"), index);
        }
        assert!(!registry.is_full());
    }

    #[test]
    fn client_script_keeps_the_session_alive_and_resets_on_a_fresh_mount() {
        // The server drops an idle session after `idle_timeout_secs`, so the
        // client has to send something. Nothing did: `ping` existed in the
        // protocol but neither side ever originated one, and a user who read
        // the page for five minutes was disconnected on their next click.
        const CLIENT: &str = include_str!("liveview_client.js");
        assert!(CLIENT.contains("setInterval"), "client must send a heartbeat");
        assert!(CLIENT.contains(r#"send({ type: "ping" })"#), "heartbeat must be a ping");

        // A fresh `session` (as opposed to `resumed`) replays the tree from
        // node 0, so the previous session's DOM has to go first or the page
        // ends up with two copies of the UI.
        assert!(CLIENT.contains("__gloryResetWryTree"), "client must reset the tree on a fresh session");
        assert!(CLIENT.contains(r#"if (message.type === "session") resetTree();"#));
    }
}
