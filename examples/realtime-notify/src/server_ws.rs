//! Server-side WebSocket push, wired with the serverfn `WebSocketEndpoint`
//! wire helpers and Salvo's WebSocket upgrade.

#![cfg(feature = "web-ssr")]

use std::time::Duration;

use futures::StreamExt;
use glory::serverfn::TransportMessage;
use salvo::prelude::*;
use salvo::websocket::{Message, WebSocket, WebSocketUpgrade};

use crate::notify::{Notification, endpoint};

#[handler]
pub async fn notify_handler(req: &mut Request, res: &mut Response) {
    if let Err(err) = WebSocketUpgrade::new().upgrade(req, res, handle_socket).await {
        res.render(err);
    }
}

async fn handle_socket(mut socket: WebSocket) {
    let endpoint = endpoint();
    let mut seq: u64 = 0;
    let mut ticker = tokio::time::interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            // Periodic server push.
            _ = ticker.tick() => {
                seq += 1;
                let payload = Notification { seq, message: format!("tick #{seq}") };
                let Ok(frame) = endpoint.encode_outgoing(&TransportMessage::Data(payload)) else {
                    break;
                };
                if socket.send(Message::text(frame)).await.is_err() {
                    break;
                }
            }
            // Incoming client message (a `Ping`).
            incoming = socket.next() => {
                let Some(Ok(message)) = incoming else {
                    break;
                };
                if !message.is_text() {
                    continue;
                }
                let Ok(text) = message.as_str() else {
                    continue;
                };
                match endpoint.decode_incoming(text) {
                    Ok(TransportMessage::Data(ping)) => {
                        seq += 1;
                        let echo = Notification { seq, message: format!("ack: {}", ping.note) };
                        if let Ok(frame) = endpoint.encode_outgoing(&TransportMessage::Data(echo)) {
                            if socket.send(Message::text(frame)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Ok(TransportMessage::Close { .. }) => break,
                    Ok(_) => continue,
                    Err(_) => continue,
                }
            }
        }
    }
}
