use glory::reflow::{Bond, Cage};
use glory::serverfn::{TransportMessage, WebSocketConnectionState};
use glory::web::widgets::*;
use glory::{Scope, Widget};

use crate::notify::{Notification, endpoint};

/// Realtime notifications page. It opens a WebSocket via the shared
/// [`crate::notify::endpoint`] and renders the latest server push reactively.
///
/// On the client the socket is live; on non-wasm builds `connect()` returns the
/// graceful "unsupported" stub, so the same widget compiles everywhere.
#[derive(Debug)]
pub struct App {
    booted: Cage<bool>,
    state: Cage<WebSocketConnectionState>,
    latest: Cage<Option<TransportMessage<Notification>>>,
}

impl App {
    pub fn new() -> Self {
        let socket = endpoint().connect();
        Self {
            booted: Cage::new(false),
            state: socket.state(),
            latest: socket.latest(),
        }
    }
}

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        // Touch `booted` so re-mounts stay idempotent (the socket is created in
        // `new`, before the first build).
        if !*self.booted.get_untracked() {
            self.booted.revise(|mut booted| *booted = true);
        }

        let state = self.state;
        let latest = self.latest;

        main()
            .attr("style", "font-family:system-ui;max-width:40rem;margin:2rem auto")
            .fill(h1().text("Realtime notifications"))
            .fill(p().text(Bond::new(move || format!("connection: {:?}", *state.get()))))
            .fill(p().text(Bond::new(move || match &*latest.get() {
                Some(TransportMessage::Data(Notification { seq, message })) => format!("latest #{seq}: {message}"),
                Some(other) => format!("latest: {other:?}"),
                None => "latest: (none yet)".to_owned(),
            })))
            .show_in(ctx);
    }
}
