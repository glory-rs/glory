//! Shared notification protocol for the realtime example.
//!
//! `Ping` is what the client sends *up*; `Notification` is what the server
//! pushes *down*. The [`endpoint`] helper builds the typed
//! [`WebSocketEndpoint`] both sides agree on.

use glory::serverfn::WebSocketEndpoint;
use serde::{Deserialize, Serialize};

/// Path the WebSocket is served at.
pub const NOTIFY_PATH: &str = "/ws/notify";

/// Client -> server.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Ping {
    pub note: String,
}

/// Server -> client push payload.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Notification {
    pub seq: u64,
    pub message: String,
}

/// The typed endpoint shared by the client (`connect`) and the server
/// (`decode_incoming` / `encode_outgoing`).
pub fn endpoint() -> WebSocketEndpoint<Ping, Notification> {
    WebSocketEndpoint::new(NOTIFY_PATH)
}
