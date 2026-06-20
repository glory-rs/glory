//! Glory desktop host.
//!
//! The widget tree runs headless on the command-stream backend
//! (`glory-core` feature `backend-command`); this crate ships the two
//! halves that turn the stream into a real desktop app:
//!
//! - [`WRY_INTERPRETER_JS`] — the JS interpreter injected into a webview;
//!   it applies serialized [`Command`] batches to the real DOM and posts
//!   serialized [`EventData`] back over IPC.
//! - [`launch`] / [`launch_with_config`] (feature `runtime`, on by
//!   default) — a tao + wry window host wiring webview IPC to a
//!   [`CommandHolder`] transaction loop.
//!
//! ```ignore
//! // requires features = ["runtime"]
//! use glory_core::{Cage, Scope, Widget};
//! use glory_core::web::widgets::*;
//! use glory_core::web::events;
//!
//! #[derive(Debug)]
//! struct App;
//! impl Widget for App {
//!     fn build(&mut self, ctx: &mut Scope) {
//!         let count = Cage::new(0i64);
//!         let on_click = {
//!             let count = count.clone();
//!             move |_| count.revise(|mut c| *c += 1)
//!         };
//!         button().text(count).on(events::click, on_click).show_in(ctx);
//!     }
//! }
//!
//! fn main() {
//!     glory_desktop::launch(|| App);
//! }
//! ```
//!
//! # Threading
//!
//! Glory's reactive runtime is thread-local: the widget tree, the holder
//! and every flush live on the tao event-loop thread. Webview IPC messages
//! are marshalled onto that thread through an `EventLoopProxy` before any
//! handler runs.

pub use glory_core::renderer::{
    Command, CommandInsertPosition, CommandNode, CommandQueue, CommandRenderer, EventData, KeyboardData, PointerData, TargetData,
};
#[cfg(feature = "backend")]
pub use glory_core::web::holders::CommandHolder;

/// JS interpreter injected into the webview. Applies command batches via
/// `window.__gloryApplyWryBatch(commands)` and posts `{GloryWryEvent: …}` /
/// `{GloryWryReady: true}` messages through `window.ipc.postMessage`.
pub const WRY_INTERPRETER_JS: &str = include_str!("wry_interpreter.js");

/// Consumer of flushed command batches.
///
/// A batch is one reactive transaction (initial mount, or the settled
/// result of one dispatched event) and must be applied atomically and in
/// order.
pub trait CommandSink: 'static {
    fn flush(&self, batch: Vec<Command>);
}

/// Test/inspection sink that records every flushed batch.
#[derive(Default)]
pub struct RecordingSink {
    batches: std::cell::RefCell<Vec<Vec<Command>>>,
}

impl RecordingSink {
    /// All flushed batches, in flush order.
    pub fn batches(&self) -> Vec<Vec<Command>> {
        self.batches.borrow().clone()
    }

    /// All flushed commands, flattened.
    pub fn commands(&self) -> Vec<Command> {
        self.batches.borrow().iter().flatten().cloned().collect()
    }
}

impl CommandSink for RecordingSink {
    fn flush(&self, batch: Vec<Command>) {
        self.batches.borrow_mut().push(batch);
    }
}

/// Envelope for messages posted from the webview over IPC.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum IpcMessage {
    /// DOM is ready (interpreter installed, `document.body` registered as
    /// node 0). The host mounts the widget tree on receipt.
    GloryWryReady(bool),
    /// A DOM event captured by an `AttachEvent` listener.
    GloryWryEvent(Box<EventData>),
    /// Answer to a `Command::Query` read request.
    GloryWryQuery(glory_core::renderer::QueryResponse),
    /// Result of a [`DesktopWindowHandle::eval`] script. `value` holds the
    /// JSON-serialized return value when `ok`, or an error string otherwise.
    GloryWryEval { id: u64, ok: bool, value: String },
}

#[cfg(feature = "runtime")]
mod runtime;
#[cfg(feature = "runtime")]
pub use runtime::{
    ChildWebviewId, Desktop, DesktopChildBounds, DesktopChildSource, DesktopChildWebview, DesktopConfig, DesktopFileDropEvent, DesktopHotKeyEvent,
    DesktopHotKeySpec, DesktopHotKeyState, DesktopProtocol, DesktopProtocolRequest, DesktopProtocolResponse, DesktopTrayEvent,
    DesktopTrayMouseButton, DesktopTrayMouseButtonState, DesktopWindowHandle, DesktopWindowId, DesktopWindowState, EvalError, MenuItemSpec, MenuSpec,
    TrayIconImage, TrayIconSpec, asset_url, desktop_protocol_response, launch, launch_with_config, launch_with_handle,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpreter_consumes_full_command_surface() {
        for command in [
            "Create",
            "SetAttribute",
            "RemoveAttribute",
            "SetProperty",
            "RemoveProperty",
            "AddClass",
            "RemoveClass",
            "SetText",
            "SetHtml",
            "Insert",
            "Remove",
            "AttachEvent",
            "DetachEvent",
        ] {
            assert!(
                WRY_INTERPRETER_JS.contains(&format!("\"{command}\"")),
                "{command} missing from interpreter"
            );
        }
        assert!(WRY_INTERPRETER_JS.contains("__gloryApplyWryCommand"));
        assert!(WRY_INTERPRETER_JS.contains("__gloryApplyWryBatch"));
        assert!(WRY_INTERPRETER_JS.contains("GloryWryEvent"));
        assert!(WRY_INTERPRETER_JS.contains("GloryWryReady"));
        assert!(WRY_INTERPRETER_JS.contains("__gloryWryQuery"));
        assert!(WRY_INTERPRETER_JS.contains("__gloryWryEval"));
        assert!(WRY_INTERPRETER_JS.contains("GloryWryEval"));
    }

    #[test]
    fn wire_format_matches_interpreter_expectations() {
        // Externally tagged JSON — the shape `decode()` in the interpreter
        // understands. Changing this is a wire-protocol break.
        let json = serde_json::to_string(&Command::Insert {
            parent: 1,
            child: 2,
            position: CommandInsertPosition::After(7),
        })
        .unwrap();
        assert_eq!(json, r#"{"Insert":{"parent":1,"child":2,"position":{"After":7}}}"#);

        let event: IpcMessage = serde_json::from_str(r#"{"GloryWryEvent":{"name":"click","node_id":4}}"#).unwrap();
        match event {
            IpcMessage::GloryWryEvent(data) => {
                assert_eq!(data.name, "click");
                assert_eq!(data.node_id, 4);
            }
            other => panic!("unexpected message {other:?}"),
        }
        assert!(matches!(
            serde_json::from_str(r#"{"GloryWryReady":true}"#).unwrap(),
            IpcMessage::GloryWryReady(true)
        ));

        let eval: IpcMessage = serde_json::from_str(r#"{"GloryWryEval":{"id":7,"ok":true,"value":"42"}}"#).unwrap();
        match eval {
            IpcMessage::GloryWryEval { id, ok, value } => {
                assert_eq!(id, 7);
                assert!(ok);
                assert_eq!(value, "42");
            }
            other => panic!("unexpected message {other:?}"),
        }
    }

    #[test]
    fn reload_message_wire_shape_matches_cli() {
        use glory_hot_reload::ReloadMessage;
        // Must stay in sync with what glory-cli's reload server serializes
        // (it uses the same shared type, so this guards the JSON shape).
        let full: ReloadMessage = serde_json::from_str(r#"{"type":"full"}"#).unwrap();
        assert_eq!(full, ReloadMessage::Full);
        let style: ReloadMessage = serde_json::from_str(r#"{"type":"style","css_path":"pkg/app.css"}"#).unwrap();
        assert_eq!(
            style,
            ReloadMessage::Style {
                css_path: "pkg/app.css".into()
            }
        );
        let functions: ReloadMessage = serde_json::from_str(r#"{"type":"functions","payload":"{\"reloads\":[]}"}"#).unwrap();
        match functions {
            ReloadMessage::Functions { payload } => {
                let batch: glory_hot_reload::FunctionReloadBatch = serde_json::from_str(&payload).unwrap();
                assert!(batch.reloads.is_empty());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "backend")]
    fn holder_batches_flow_into_sink() {
        use glory_core::web::widgets::button;
        use glory_core::{Holder, Scope, Widget};

        #[derive(Debug)]
        struct App;
        impl Widget for App {
            fn build(&mut self, ctx: &mut Scope) {
                button().text("hi").show_in(ctx);
            }
        }

        let sink = RecordingSink::default();
        let holder = CommandHolder::new().mount(App);
        sink.flush(holder.take_batch());

        let commands = sink.commands();
        assert!(commands.iter().any(|c| matches!(c, Command::Create { name, .. } if name == "button")));
        assert!(commands.iter().any(|c| matches!(c, Command::SetText { value, .. } if value == "hi")));
        assert!(
            commands.iter().any(|c| matches!(c, Command::Insert { parent: 0, .. })),
            "root insert must target reserved host id 0: {commands:?}"
        );
    }
}
