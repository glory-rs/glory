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

#[cfg(feature = "salvo")]
pub mod salvo_mount {
    use std::sync::Arc;
    use std::sync::mpsc::{self, Sender};
    use std::thread::{self, JoinHandle};

    use futures::StreamExt;
    use futures::channel::oneshot;
    use glory_core::Widget;
    use salvo::prelude::{Depot, FlowCtrl, Request, Response, Router};
    use salvo::websocket::{Message, WebSocket, WebSocketUpgrade};
    use salvo::{Handler, async_trait};

    use crate::{LiveViewMessage, LiveViewSession};

    pub fn router<W>(widget: impl Fn() -> W + Send + Sync + 'static) -> Router
    where
        W: Widget + 'static,
    {
        let factory: Arc<dyn Fn() -> W + Send + Sync + 'static> = Arc::new(widget);
        Router::with_path("__glory/liveview").get(LiveViewHandler { factory })
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

    struct SessionWorker {
        sender: Sender<SessionRequest>,
        thread: Option<JoinHandle<()>>,
    }

    enum SessionRequest {
        Message {
            message: LiveViewMessage,
            reply: oneshot::Sender<Option<LiveViewMessage>>,
        },
        Close,
    }

    impl SessionWorker {
        fn spawn<W>(factory: Arc<dyn Fn() -> W + Send + Sync + 'static>) -> Result<(Self, LiveViewMessage), mpsc::RecvError>
        where
            W: Widget + 'static,
        {
            let (sender, receiver) = mpsc::channel();
            let (mount_sender, mount_receiver) = mpsc::channel();
            let thread = thread::spawn(move || {
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
            let (reply, receiver) = oneshot::channel();
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

    impl Drop for SessionWorker {
        fn drop(&mut self) {
            let _ = self.sender.send(SessionRequest::Close);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
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

#[cfg(test)]
mod tests {
    use glory_core::reflow::Cage;
    use glory_core::renderer::Command;
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
}
