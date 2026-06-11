//! Mobile counter: the same widget code as the web/desktop counters,
//! hosted in a wry webview on Android / iOS through the command stream.
//!
//! Build (Android, from this directory):
//! ```text
//! cargo ndk -t arm64-v8a -o app/src/main/jniLibs build --lib
//! ```
//! or via the CLI: `glory build --target android`.
//!
//! Host-project wiring lives in `crates/cli/templates/mobile/`.

use glory_core::reflow::Cage;
use glory_core::web::events;
use glory_core::web::helpers::event_target_value;
use glory_core::web::widgets::*;
use glory_core::{Scope, Widget};

#[derive(Debug)]
pub struct Counter {
    value: Cage<i64>,
}

impl Counter {
    pub fn new() -> Self {
        Self { value: Cage::new(0) }
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Counter {
    fn build(&mut self, ctx: &mut Scope) {
        let value = self.value.clone();
        let increase = move |_| {
            value.revise(|mut value| *value += 1);
        };
        let value = self.value.clone();
        let decrease = move |_| {
            value.revise(|mut value| *value -= 1);
        };
        let value = self.value.clone();
        let set_from_input = move |ev| {
            let parsed = event_target_value(&ev).parse::<i64>().unwrap_or_default();
            value.revise(|mut value| *value = parsed);
        };

        div()
            .attr(
                "style",
                "font-family: sans-serif; padding: 2em; min-height: calc(var(--glory-viewport-height) - var(--glory-safe-top) - var(--glory-safe-bottom)); display: flex; gap: .5em; align-items: center;",
            )
            .fill(button().text("-").on(events::click, decrease))
            .fill(
                span()
                    .attr("style", "min-width: 4em; text-align: center; font-size: 1.5em;")
                    .text(self.value.clone()),
            )
            .fill(button().text("+").on(events::click, increase))
            .fill(input().attr("inputmode", "numeric").attr("placeholder", "set value").on(events::input, set_from_input))
            .show_in(ctx);
    }
}

/// Webview host loop shared by both mobile platforms: a trimmed-down
/// version of `glory-desktop`'s runtime (no menus, no hot reload).
#[cfg(any(target_os = "android", target_os = "ios"))]
mod host {
    use glory_core::renderer::EventData;
    use glory_core::web::holders::CommandHolder;
    use glory_core::{Holder, Widget};
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::window::WindowBuilder;
    use wry::WebViewBuilder;

    const BOOTSTRAP_HTML: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<style>
:root {
  --glory-safe-top: env(safe-area-inset-top, 0px);
  --glory-safe-right: env(safe-area-inset-right, 0px);
  --glory-safe-bottom: env(safe-area-inset-bottom, 0px);
  --glory-safe-left: env(safe-area-inset-left, 0px);
  --glory-viewport-height: 100vh;
  --glory-keyboard-inset-bottom: 0px;
}
html, body {
  min-height: 100%;
  margin: 0;
  overscroll-behavior: none;
  background: Canvas;
}
body {
  min-height: var(--glory-viewport-height);
  padding: var(--glory-safe-top) var(--glory-safe-right)
    calc(var(--glory-safe-bottom) + var(--glory-keyboard-inset-bottom))
    var(--glory-safe-left);
  box-sizing: border-box;
}
</style>
</head>
<body>
<script>
(() => {
  const root = document.documentElement;
  const dispatch = (name, detail = {}) => window.dispatchEvent(new CustomEvent(name, { detail }));
  const syncViewport = () => {
    const vv = window.visualViewport;
    const height = vv ? vv.height : window.innerHeight;
    const keyboard = Math.max(0, window.innerHeight - height - (vv ? vv.offsetTop : 0));
    root.style.setProperty("--glory-viewport-height", `${height}px`);
    root.style.setProperty("--glory-keyboard-inset-bottom", `${keyboard}px`);
    dispatch("glory:viewport", { height, keyboardInsetBottom: keyboard });
  };
  window.addEventListener("resize", syncViewport, { passive: true });
  if (window.visualViewport) {
    window.visualViewport.addEventListener("resize", syncViewport, { passive: true });
    window.visualViewport.addEventListener("scroll", syncViewport, { passive: true });
  }
  document.addEventListener("visibilitychange", () => {
    dispatch(document.hidden ? "glory:background" : "glory:foreground");
  });
  window.addEventListener("focus", () => dispatch("glory:foreground"), { passive: true });
  window.addEventListener("blur", () => dispatch("glory:background"), { passive: true });
  syncViewport();
})();
</script>
</body>
</html>"#;

    enum HostEvent {
        Ready,
        Dom(EventData),
        Query(glory_core::renderer::QueryResponse),
    }

    pub fn run<W: Widget + 'static>(widget: impl FnOnce() -> W + 'static) {
        let event_loop = EventLoopBuilder::<HostEvent>::with_user_event().build();
        let window = WindowBuilder::new().build(&event_loop).expect("create window");

        let ipc_proxy = event_loop.create_proxy();
        let webview = WebViewBuilder::new()
            .with_initialization_script(glory_desktop::WRY_INTERPRETER_JS)
            .with_html(BOOTSTRAP_HTML)
            .with_ipc_handler(move |request: wry::http::Request<String>| {
                match serde_json::from_str::<glory_desktop::IpcMessage>(request.body()) {
                    Ok(glory_desktop::IpcMessage::GloryWryReady(_)) => {
                        let _ = ipc_proxy.send_event(HostEvent::Ready);
                    }
                    Ok(glory_desktop::IpcMessage::GloryWryEvent(data)) => {
                        let _ = ipc_proxy.send_event(HostEvent::Dom(data));
                    }
                    Ok(glory_desktop::IpcMessage::GloryWryQuery(response)) => {
                        let _ = ipc_proxy.send_event(HostEvent::Query(response));
                    }
                    Err(err) => tracing::warn!(%err, "mobile-counter: undecodable IPC message"),
                }
            })
            .build(&window)
            .expect("create webview");

        let mut widget_factory = Some(widget);
        let mut holder: Option<CommandHolder> = None;

        event_loop.run(move |event, _target, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => *control_flow = ControlFlow::Exit,
                Event::UserEvent(HostEvent::Ready) => {
                    if let Some(factory) = widget_factory.take() {
                        let mounted = CommandHolder::new().mount(factory());
                        flush(&webview, &mounted);
                        holder = Some(mounted);
                    }
                    let _ = &window;
                }
                Event::UserEvent(HostEvent::Dom(data)) => {
                    if let Some(holder) = &holder {
                        holder.dispatch_event(data);
                        flush(&webview, holder);
                    }
                }
                Event::UserEvent(HostEvent::Query(response)) => {
                    if let Some(holder) = &holder {
                        holder.resolve_query(response);
                        flush(&webview, holder);
                    }
                }
                _ => {}
            }
        });
    }

    fn flush(webview: &wry::WebView, holder: &CommandHolder) {
        let batch = holder.take_batch();
        if batch.is_empty() {
            return;
        }
        let json = serde_json::to_string(&batch).expect("commands serialize");
        let _ = webview.evaluate_script(&format!("window.__gloryApplyWryBatch({json});"));
    }
}

/// Android entry: the Gradle host's `MainActivity` (a `WryActivity`
/// subclass, package `com.example.mobile_counter`) loads this cdylib;
/// the binding macros generate the JNI surface (as items inside
/// `bindings` — `#[no_mangle]` items in function bodies are still
/// exported) and `_start_app` runs the host loop on the activity's main
/// thread.
#[cfg(target_os = "android")]
mod android {
    fn _start_app() {
        super::host::run(super::Counter::new);
    }

    #[allow(dead_code)]
    fn bindings() {
        tao::android_binding!(com_example, mobile_counter, WryActivity, wry::android_setup, _start_app, ::tao);
        wry::android_binding!(com_example, mobile_counter, ::wry);
    }
}

/// iOS entry: the Xcode host links the staticlib and calls this from
/// `main` (UIKit takes over inside tao's event loop).
#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn start_app() {
    host::run(Counter::new);
}
