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
            LiveViewMessage::Close { .. } => None,
            LiveViewMessage::Hello { .. }
            | LiveViewMessage::Mount { .. }
            | LiveViewMessage::Patch { .. }
            | LiveViewMessage::Error { .. }
            | LiveViewMessage::Pong => None,
        }
    }
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
    sender: std::sync::mpsc::Sender<SessionRequest>,
    thread: Option<std::thread::JoinHandle<()>>,
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
impl SessionWorker {
    fn spawn<W>(factory: std::sync::Arc<dyn Fn() -> W + Send + Sync + 'static>) -> Result<(Self, LiveViewMessage), std::sync::mpsc::RecvError>
    where
        W: Widget + 'static,
    {
        let (sender, receiver) = std::sync::mpsc::channel();
        let (mount_sender, mount_receiver) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let (session, mount) = LiveViewSession::mount(factory());
            if mount_sender.send(mount).is_err() {
                return;
            }
            while let Ok(request) = receiver.recv() {
                match request {
                    SessionRequest::Message { message, reply } => {
                        let _ = reply.send(session.handle_message(message));
                    }
                    SessionRequest::Close => break,
                }
            }
        });
        let mount = mount_receiver.recv()?;
        Ok((
            Self {
                sender,
                thread: Some(thread),
            },
            mount,
        ))
    }

    async fn handle_message(&self, message: LiveViewMessage) -> Option<LiveViewMessage> {
        let (reply, receiver) = futures::channel::oneshot::channel();
        if self.sender.send(SessionRequest::Message { message, reply }).is_err() {
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
impl Drop for SessionWorker {
    fn drop(&mut self) {
        let _ = self.sender.send(SessionRequest::Close);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
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
        let Ok((worker, mount)) = SessionWorker::spawn(factory) else {
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
        let Ok((worker, mount)) = SessionWorker::spawn(factory) else {
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
        let Ok((worker, mount)) = SessionWorker::spawn(factory) else {
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
}
