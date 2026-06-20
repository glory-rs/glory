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
    Hello { protocol_version: u32 },
    Mount { commands: Vec<Command> },
    Event(Box<EventData>),
    Query(Box<QueryResponse>),
    Patch { commands: Vec<Command> },
    Error { message: String },
    Ping,
    Pong,
    Close { reason: String },
}

impl LiveViewMessage {
    pub fn hello() -> Self {
        Self::Hello {
            protocol_version: LIVEVIEW_PROTOCOL_VERSION,
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
            LiveViewMessage::Hello { protocol_version } => {
                if protocol_version != LIVEVIEW_PROTOCOL_VERSION {
                    Some(LiveViewMessage::Error {
                        message: format!("liveview protocol mismatch: client {protocol_version}, server {LIVEVIEW_PROTOCOL_VERSION}"),
                    })
                } else {
                    None
                }
            }
            LiveViewMessage::Close { .. } => None,
            LiveViewMessage::Mount { .. } | LiveViewMessage::Patch { .. } | LiveViewMessage::Error { .. } | LiveViewMessage::Pong => None,
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
}

impl Default for LiveViewConfig {
    fn default() -> Self {
        // 5 minutes idle, 1 hour absolute, 64 pending patches — conservative
        // defaults a host app can override.
        Self {
            idle_timeout_secs: 300,
            max_lifetime_secs: 3600,
            max_pending_patches: 64,
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
struct SessionWorker {
    sender: futures::channel::mpsc::Sender<SessionRequest>,
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
enum SessionRequest {
    Message {
        message: LiveViewMessage,
        reply: futures::channel::oneshot::Sender<Option<LiveViewMessage>>,
    },
    Close,
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
            SessionRequest::Close => break,
        }
    }
}

#[cfg(any(feature = "salvo", feature = "axum", feature = "actix"))]
impl Drop for SessionWorker {
    fn drop(&mut self) {
        let _ = self.sender.try_send(SessionRequest::Close);
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

    use crate::{LIVEVIEW_DEFAULT_PATH, LiveViewMessage, SessionWorker};

    pub fn router<W>(widget: impl Fn() -> W + Send + Sync + 'static) -> Router
    where
        W: Widget + 'static,
    {
        let factory: Arc<dyn Fn() -> W + Send + Sync + 'static> = Arc::new(widget);
        Router::with_path(LIVEVIEW_DEFAULT_PATH.trim_start_matches('/')).get(LiveViewHandler { factory })
    }

    struct LiveViewHandler<W>
    where
        W: Widget + 'static,
    {
        factory: Arc<dyn Fn() -> W + Send + Sync + 'static>,
    }

    #[async_trait]
    impl<W> Handler for LiveViewHandler<W>
    where
        W: Widget + 'static,
    {
        async fn handle(&self, req: &mut Request, _depot: &mut Depot, res: &mut Response, _ctrl: &mut FlowCtrl) {
            let factory = self.factory.clone();
            if let Err(err) = WebSocketUpgrade::new()
                .upgrade(req, res, move |socket| handle_socket(socket, factory))
                .await
            {
                res.render(err);
            }
        }
    }

    async fn handle_socket<W>(mut socket: WebSocket, factory: Arc<dyn Fn() -> W + Send + Sync + 'static>)
    where
        W: Widget + 'static,
    {
        let Ok((worker, mount)) = SessionWorker::spawn(factory).await else {
            let _ = send_error(&mut socket, "liveview session worker failed to mount").await;
            return;
        };
        if send(&mut socket, mount).await.is_err() {
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
                    if let Some(reply) = worker.handle_message(message).await
                        && send(&mut socket, reply).await.is_err()
                    {
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
    use axum::routing::get;
    use futures::StreamExt;
    use glory_core::Widget;

    use crate::{LIVEVIEW_DEFAULT_PATH, LiveViewMessage, LiveviewRouter, SessionWorker, normalize_liveview_path};

    pub fn router<W>(widget: impl Fn() -> W + Send + Sync + 'static) -> Router
    where
        W: Widget + 'static,
    {
        Router::new().with_liveview(LIVEVIEW_DEFAULT_PATH, widget)
    }

    pub fn route<S, W>(router: Router<S>, path: &str, widget: impl Fn() -> W + Send + Sync + 'static) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
        W: Widget + 'static,
    {
        router.with_liveview(path, widget)
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
            let factory: Arc<dyn Fn() -> W + Send + Sync + 'static> = Arc::new(widget);
            self.route(
                &normalize_liveview_path(path),
                get(move |ws: WebSocketUpgrade| {
                    let factory = factory.clone();
                    async move { ws.on_upgrade(move |socket| handle_socket(socket, factory)) }
                }),
            )
        }
    }

    async fn handle_socket<W>(mut socket: WebSocket, factory: Arc<dyn Fn() -> W + Send + Sync + 'static>)
    where
        W: Widget + 'static,
    {
        let Ok((worker, mount)) = SessionWorker::spawn(factory).await else {
            let _ = send_error(&mut socket, "liveview session worker failed to mount").await;
            return;
        };
        if send(&mut socket, mount).await.is_err() {
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
                    if let Some(reply) = worker.handle_message(message).await
                        && send(&mut socket, reply).await.is_err()
                    {
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

    use crate::{LIVEVIEW_DEFAULT_PATH, LiveViewMessage, LiveviewRouter, SessionWorker, normalize_liveview_path};

    pub fn scope<W>(widget: impl Fn() -> W + Send + Sync + 'static) -> Scope
    where
        W: Widget + 'static,
    {
        web::scope("").with_liveview(LIVEVIEW_DEFAULT_PATH, widget)
    }

    pub fn configure<W>(cfg: &mut web::ServiceConfig, widget: impl Fn() -> W + Send + Sync + 'static)
    where
        W: Widget + 'static,
    {
        let factory: Arc<dyn Fn() -> W + Send + Sync + 'static> = Arc::new(widget);
        cfg.route(
            LIVEVIEW_DEFAULT_PATH,
            web::get().to(move |req: HttpRequest, body: web::Payload| {
                let factory = factory.clone();
                async move { handler(req, body, factory).await }
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
            self.route(
                &normalize_liveview_path(path),
                web::get().to(move |req: HttpRequest, body: web::Payload| {
                    let factory = factory.clone();
                    async move { handler(req, body, factory).await }
                }),
            )
        }
    }

    async fn handler<W>(req: HttpRequest, body: web::Payload, factory: Arc<dyn Fn() -> W + Send + Sync + 'static>) -> actix_web::Result<HttpResponse>
    where
        W: Widget + 'static,
    {
        let (response, session, stream) = actix_ws::handle(&req, body)?;
        actix_web::rt::spawn(handle_socket(session, stream, factory));
        Ok(response)
    }

    async fn handle_socket<W>(
        mut session: actix_ws::Session,
        mut stream: actix_ws::MessageStream,
        factory: Arc<dyn Fn() -> W + Send + Sync + 'static>,
    ) where
        W: Widget + 'static,
    {
        let Ok((worker, mount)) = SessionWorker::spawn(factory).await else {
            let _ = send_error(&mut session, "liveview session worker failed to mount").await;
            return;
        };
        if send(&mut session, mount).await.is_err() {
            return;
        }

        while let Some(message) = stream.recv().await {
            let Ok(message) = message else {
                break;
            };
            match message {
                actix_ws::Message::Text(text) => match LiveViewMessage::from_json(text.as_ref()) {
                    Ok(message) => {
                        if let Some(reply) = worker.handle_message(message).await
                            && send(&mut session, reply).await.is_err()
                        {
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
        let reply = session.handle_message(LiveViewMessage::Hello { protocol_version: 999 });
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
    }

    #[test]
    fn client_script_exposes_reconnect_entrypoint() {
        assert!(LIVEVIEW_CLIENT_JS.contains("__gloryLiveViewConnect"));
        assert!(LIVEVIEW_CLIENT_JS.contains("__gloryWryEvent"));
        assert!(LIVEVIEW_CLIENT_JS.contains("__gloryWryQuery"));
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
}
