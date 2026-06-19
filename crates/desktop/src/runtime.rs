//! tao + wry window host.
//!
//! Transaction loop (see `CommandHolder` docs): each webview posts
//! `GloryWryReady` once its DOM is up → the host mounts that window's
//! widget tree and flushes the initial batch; every subsequent
//! `GloryWryEvent` is marshalled onto the event-loop thread, dispatched
//! into the owning window's holder, and the settled patch batch is flushed
//! back with one `evaluate_script` call.
//!
//! Multi-window: every window owns an independent `CommandHolder` (one
//! reactive scope per `HolderId`), webview and command queue; IPC events
//! carry a stable [`DesktopWindowId`] so batches never cross windows.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use glory_core::renderer::EventData;
use glory_core::web::holders::CommandHolder;
use glory_core::{Holder, Widget};
use glory_hot_reload::{FunctionReloadBatch, ReloadMessage};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
use tao::window::{Fullscreen, Window, WindowBuilder, WindowId};
use wry::{WebView, WebViewBuilder};

use crate::IpcMessage;

/// Stable process-local id for a desktop window.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct DesktopWindowId(usize);

impl DesktopWindowId {
    pub fn as_usize(self) -> usize {
        self.0
    }
}

/// Cached window state visible to widget callbacks.
#[derive(Clone, Debug)]
pub struct DesktopWindowState {
    id: DesktopWindowId,
    fullscreen: bool,
    maximized: bool,
    focused: bool,
    zoom_level: f64,
    closed: bool,
}

impl DesktopWindowState {
    fn new(id: DesktopWindowId) -> Self {
        Self {
            id,
            fullscreen: false,
            maximized: false,
            focused: true,
            zoom_level: 1.0,
            closed: false,
        }
    }

    pub fn id(&self) -> DesktopWindowId {
        self.id
    }

    pub fn is_fullscreen(&self) -> bool {
        self.fullscreen
    }

    pub fn is_maximized(&self) -> bool {
        self.maximized
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn zoom_level(&self) -> f64 {
        self.zoom_level
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

/// Handle that widget callbacks can capture to control their native window.
#[derive(Clone)]
pub struct DesktopWindowHandle {
    id: DesktopWindowId,
    proxy: EventLoopProxy<HostEvent>,
    state: Rc<RefCell<DesktopWindowState>>,
    window_queue: Rc<RefCell<Vec<PendingWindow>>>,
    next_window_index: Rc<Cell<usize>>,
}

impl std::fmt::Debug for DesktopWindowHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopWindowHandle")
            .field("id", &self.id)
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl DesktopWindowHandle {
    fn new(
        id: DesktopWindowId,
        proxy: EventLoopProxy<HostEvent>,
        state: Rc<RefCell<DesktopWindowState>>,
        window_queue: Rc<RefCell<Vec<PendingWindow>>>,
        next_window_index: Rc<Cell<usize>>,
    ) -> Self {
        Self {
            id,
            proxy,
            state,
            window_queue,
            next_window_index,
        }
    }

    pub fn id(&self) -> DesktopWindowId {
        self.id
    }

    pub fn state(&self) -> DesktopWindowState {
        self.state.borrow().clone()
    }

    pub fn is_fullscreen(&self) -> bool {
        self.state.borrow().is_fullscreen()
    }

    pub fn is_maximized(&self) -> bool {
        self.state.borrow().is_maximized()
    }

    pub fn zoom_level(&self) -> f64 {
        self.state.borrow().zoom_level()
    }

    pub fn drag_window(&self) -> bool {
        self.send(WindowCommand::DragWindow)
    }

    pub fn set_fullscreen(&self, fullscreen: bool) -> bool {
        self.state.borrow_mut().fullscreen = fullscreen;
        self.send(WindowCommand::SetFullscreen(fullscreen))
    }

    pub fn set_maximized(&self, maximized: bool) -> bool {
        self.state.borrow_mut().maximized = maximized;
        self.send(WindowCommand::SetMaximized(maximized))
    }

    pub fn toggle_maximized(&self) -> bool {
        let next = !self.is_maximized();
        self.state.borrow_mut().maximized = next;
        self.send(WindowCommand::SetMaximized(next))
    }

    pub fn focus(&self) -> bool {
        self.send(WindowCommand::Focus)
    }

    pub fn set_zoom_level(&self, zoom_level: f64) -> bool {
        if !zoom_level.is_finite() || zoom_level <= 0.0 {
            return false;
        }
        self.state.borrow_mut().zoom_level = zoom_level;
        self.send(WindowCommand::SetZoomLevel(zoom_level))
    }

    pub fn close(&self) -> bool {
        self.close_window(self.id)
    }

    pub fn close_window(&self, id: DesktopWindowId) -> bool {
        self.proxy
            .send_event(HostEvent::WindowCommand {
                id,
                command: WindowCommand::Close,
            })
            .is_ok()
    }

    pub fn open_window<W>(&self, config: DesktopConfig, widget: impl FnOnce(DesktopWindowHandle) -> W + 'static) -> DesktopWindowId
    where
        W: Widget + 'static,
    {
        let id = DesktopWindowId(self.next_window_index.get());
        self.next_window_index.set(id.0 + 1);
        self.window_queue.borrow_mut().push(PendingWindow::new(id, config, widget));
        let _ = self.proxy.send_event(HostEvent::OpenQueuedWindows);
        id
    }

    fn send(&self, command: WindowCommand) -> bool {
        self.proxy.send_event(HostEvent::WindowCommand { id: self.id, command }).is_ok()
    }
}

/// One menu entry. `id` is what your `on_menu` callback receives.
#[derive(Clone, Debug)]
pub struct MenuItemSpec {
    pub id: String,
    pub label: String,
}

impl MenuItemSpec {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// Declarative window menu: a list of `(submenu title, items)` pairs.
///
/// Platform notes: on Windows the menu attaches to the window; on macOS it
/// becomes the global application menu (the first window's spec wins);
/// other platforms currently log a warning.
#[derive(Clone, Debug, Default)]
pub struct MenuSpec {
    pub submenus: Vec<(String, Vec<MenuItemSpec>)>,
}

impl MenuSpec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn submenu(mut self, title: impl Into<String>, items: Vec<MenuItemSpec>) -> Self {
        self.submenus.push((title.into(), items));
        self
    }
}

/// Window/host options for one window.
#[derive(Clone)]
pub struct DesktopConfig {
    pub title: String,
    pub inner_size: (f64, f64),
    pub resizable: bool,
    /// Webview devtools (defaults to on in debug builds).
    pub devtools: bool,
    /// Coalesce redundant writes per batch before they cross IPC.
    pub coalesce: bool,
    /// Dev-mode hook invoked on the event-loop thread when `glory-cli
    /// watch/serve` pushes a function-reload batch (the desktop
    /// counterpart of the browser's `glory:function-reload` CustomEvent).
    /// Re-register reloadable closures / revise signals here; the settled
    /// patch batch is flushed to the webview automatically afterwards.
    pub on_function_reload: Option<Rc<dyn Fn(&CommandHolder, FunctionReloadBatch)>>,
    /// Filesystem root served through the `glory://` protocol (see
    /// [`asset_url`]). Defaults to `GLORY_SITE_ROOT` (set by glory-cli),
    /// falling back to the executable's directory.
    pub assets_root: Option<std::path::PathBuf>,
    /// Native window menu.
    pub menu: Option<MenuSpec>,
    /// Invoked on the event-loop thread when a [`MenuSpec`] item is
    /// activated; receives the item id. Signal writes settle and flush
    /// automatically afterwards.
    pub on_menu: Option<Rc<dyn Fn(&CommandHolder, &str)>>,
}

impl std::fmt::Debug for DesktopConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopConfig")
            .field("title", &self.title)
            .field("inner_size", &self.inner_size)
            .field("resizable", &self.resizable)
            .field("devtools", &self.devtools)
            .field("coalesce", &self.coalesce)
            .field("on_function_reload", &self.on_function_reload.is_some())
            .field("assets_root", &self.assets_root)
            .field("menu", &self.menu)
            .field("on_menu", &self.on_menu.is_some())
            .finish()
    }
}

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            title: "Glory".to_owned(),
            inner_size: (900.0, 640.0),
            resizable: true,
            devtools: cfg!(debug_assertions),
            coalesce: true,
            on_function_reload: None,
            assets_root: None,
            menu: None,
            on_menu: None,
        }
    }
}

const BOOTSTRAP_HTML: &str = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body></body></html>";

enum HostEvent {
    Ready(DesktopWindowId),
    Dom(DesktopWindowId, Box<EventData>),
    Query(DesktopWindowId, glory_core::renderer::QueryResponse),
    Reload(ReloadMessage),
    Menu(String),
    WindowCommand { id: DesktopWindowId, command: WindowCommand },
    OpenQueuedWindows,
}

#[derive(Clone, Copy, Debug)]
enum WindowCommand {
    DragWindow,
    SetFullscreen(bool),
    SetMaximized(bool),
    Focus,
    SetZoomLevel(f64),
    Close,
}

/// Connects to `glory-cli`'s `/live_reload` websocket when the CLI is
/// driving this process (`GLORY_WATCH=ON` + `GLORY_RELOAD_PORT` env), and
/// marshals reload messages onto the event-loop thread. Reconnects with a
/// 1s backoff — the CLI restarts its reload server between rebuilds —
/// and exits once the event loop is gone.
fn spawn_reload_client(proxy: EventLoopProxy<HostEvent>) {
    if std::env::var("GLORY_WATCH").map(|v| v != "ON").unwrap_or(true) {
        return;
    }
    let Ok(port) = std::env::var("GLORY_RELOAD_PORT") else {
        return;
    };
    let url = format!("ws://127.0.0.1:{port}/live_reload");
    std::thread::spawn(move || {
        loop {
            let Ok((mut socket, _)) = tungstenite::connect(&url) else {
                std::thread::sleep(std::time::Duration::from_secs(1));
                continue;
            };
            tracing::debug!("glory-desktop: reload websocket connected");
            loop {
                match socket.read() {
                    Ok(message) if message.is_text() => {
                        let text = message.into_text().unwrap_or_default();
                        match serde_json::from_str::<ReloadMessage>(text.as_str()) {
                            Ok(reload) => {
                                if proxy.send_event(HostEvent::Reload(reload)).is_err() {
                                    return; // event loop is gone
                                }
                            }
                            Err(err) => tracing::warn!(%err, "glory-desktop: undecodable reload message"),
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break, // reconnect
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
}

/// URL for a static asset served through the `glory://` custom protocol.
///
/// Pass an [`Asset::public_path`](glory_core::assets::Asset::public_path)
/// (or any absolute web path). Platform differences are absorbed here:
/// WebView2 rewrites custom protocols to `http://glory.localhost/...`.
pub fn asset_url(public_path: &str) -> String {
    let path = if public_path.starts_with('/') {
        public_path.to_owned()
    } else {
        format!("/{public_path}")
    };
    if cfg!(windows) {
        format!("http://glory.localhost{path}")
    } else {
        format!("glory://localhost{path}")
    }
}

/// Filesystem root for `glory://` requests: explicit config wins, then the
/// CLI-provided `GLORY_SITE_ROOT`, then the executable's directory.
fn assets_root(config: &DesktopConfig) -> std::path::PathBuf {
    if let Some(root) = &config.assets_root {
        return root.clone();
    }
    if let Ok(root) = std::env::var("GLORY_SITE_ROOT") {
        return std::path::PathBuf::from(root);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn install_bundle_asset_manifest(root: &std::path::Path) {
    glory_core::assets::clear_asset_manifest();
    let path = root.join("glory-bundle.json");
    match std::fs::read_to_string(&path) {
        Ok(json) => match glory_core::assets::AssetManifest::from_bundle_json(&json) {
            Ok(manifest) if !manifest.is_empty() => glory_core::assets::install_asset_manifest(manifest),
            Ok(_) => {}
            Err(err) => tracing::warn!(%err, path = %path.display(), "glory-desktop: invalid asset manifest"),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => tracing::warn!(%err, path = %path.display(), "glory-desktop: asset manifest read failed"),
    }
}

/// Resolves a request path under `root`, rejecting traversal outside it.
fn resolve_asset_path(root: &std::path::Path, request_path: &str) -> Option<std::path::PathBuf> {
    let relative = request_path.trim_start_matches('/');
    if relative.is_empty() {
        return None;
    }
    // Reject traversal components before touching the filesystem.
    if std::path::Path::new(relative)
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return None;
    }
    let candidate = root.join(relative);
    candidate.is_file().then_some(candidate)
}

fn mime_for(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or_default() {
        "html" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

fn serve_asset(root: &std::path::Path, request: wry::http::Request<Vec<u8>>) -> wry::http::Response<std::borrow::Cow<'static, [u8]>> {
    let path = request.uri().path().to_owned();
    let response = |status: u16, mime: &str, body: Vec<u8>| {
        wry::http::Response::builder()
            .status(status)
            .header("content-type", mime.to_owned())
            .body(std::borrow::Cow::Owned(body))
            .expect("static response builds")
    };
    match resolve_asset_path(root, &path) {
        Some(file) => match std::fs::read(&file) {
            Ok(bytes) => response(200, mime_for(&file), bytes),
            Err(err) => {
                tracing::warn!(%err, %path, "glory-desktop: asset read failed");
                response(500, "text/plain", b"asset read failed".to_vec())
            }
        },
        None => response(404, "text/plain", b"not found".to_vec()),
    }
}

type MountFn = Box<dyn FnOnce(CommandHolder) -> CommandHolder>;
type MountFactory = Box<dyn FnOnce(DesktopWindowHandle) -> MountFn>;

struct PendingWindow {
    id: DesktopWindowId,
    config: DesktopConfig,
    state: Rc<RefCell<DesktopWindowState>>,
    mount: MountFactory,
}

impl PendingWindow {
    fn new<W>(id: DesktopWindowId, config: DesktopConfig, widget: impl FnOnce(DesktopWindowHandle) -> W + 'static) -> Self
    where
        W: Widget + 'static,
    {
        Self {
            id,
            config,
            state: Rc::new(RefCell::new(DesktopWindowState::new(id))),
            mount: Box::new(move |window| Box::new(move |holder| holder.mount(widget(window)))),
        }
    }
}

struct WindowSlot {
    /// Original creation id — what the IPC closures and menu routes
    /// carry. Stable across window closes (slots are removed, not
    /// reindexed).
    id: DesktopWindowId,
    window: Window,
    webview: WebView,
    holder: Option<CommandHolder>,
    mount: Option<MountFn>,
    config: DesktopConfig,
    state: Rc<RefCell<DesktopWindowState>>,
    /// Keeps the muda menu alive for the window's lifetime.
    #[allow(dead_code)]
    menu: Option<muda::Menu>,
}

/// Multi-window host builder.
///
/// ```ignore
/// glory_desktop::Desktop::new()
///     .window(DesktopConfig { title: "Main".into(), ..Default::default() }, || MainApp)
///     .window(DesktopConfig { title: "Tools".into(), ..Default::default() }, || ToolsApp)
///     .run();
/// ```
///
/// Every window runs an isolated widget tree (own `CommandHolder`, own
/// command queue). The process exits when the last window closes.
#[derive(Default)]
pub struct Desktop {
    windows: Vec<PendingWindow>,
    next_window_index: usize,
}

impl Desktop {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn window<W>(mut self, config: DesktopConfig, widget: impl FnOnce() -> W + 'static) -> Self
    where
        W: Widget + 'static,
    {
        self = self.window_with_handle(config, move |_| widget());
        self
    }

    pub fn window_with_handle<W>(mut self, config: DesktopConfig, widget: impl FnOnce(DesktopWindowHandle) -> W + 'static) -> Self
    where
        W: Widget + 'static,
    {
        let id = DesktopWindowId(self.next_window_index);
        self.next_window_index += 1;
        self.windows.push(PendingWindow::new(id, config, widget));
        self
    }

    /// Opens all registered windows and runs the event loop. Never returns.
    pub fn run(self) -> ! {
        assert!(!self.windows.is_empty(), "Desktop::run called with no windows registered");

        let event_loop = EventLoopBuilder::<HostEvent>::with_user_event().build();
        let mut slots: Vec<(WindowId, WindowSlot)> = Vec::new();
        let mut menu_routes: HashMap<String, DesktopWindowId> = HashMap::new();
        let proxy = event_loop.create_proxy();
        let window_queue: Rc<RefCell<Vec<PendingWindow>>> = Rc::new(RefCell::new(Vec::new()));
        let next_window_index = Rc::new(Cell::new(self.next_window_index));

        for pending in self.windows {
            create_window(
                &event_loop,
                pending,
                proxy.clone(),
                window_queue.clone(),
                next_window_index.clone(),
                &mut slots,
                &mut menu_routes,
            );
        }

        let menu_proxy = proxy.clone();
        muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
            let _ = menu_proxy.send_event(HostEvent::Menu(event.id().0.clone()));
        }));

        spawn_reload_client(proxy.clone());

        event_loop.run(move |event, target, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    window_id,
                    ..
                } => {
                    close_slot_by_window_id(&mut slots, window_id);
                    if slots.is_empty() {
                        *control_flow = ControlFlow::Exit;
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::Focused(focused),
                    window_id,
                    ..
                } => {
                    if let Some(slot) = slot_by_window_id(&mut slots, window_id) {
                        slot.state.borrow_mut().focused = focused;
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(_),
                    window_id,
                    ..
                } => {
                    if let Some(slot) = slot_by_window_id(&mut slots, window_id) {
                        let mut state = slot.state.borrow_mut();
                        state.maximized = slot.window.is_maximized();
                        state.fullscreen = slot.window.fullscreen().is_some();
                    }
                }
                Event::UserEvent(HostEvent::Ready(id)) => {
                    let Some(slot) = slot_by_id(&mut slots, id) else {
                        return;
                    };
                    let Some(mount) = slot.mount.take() else {
                        tracing::warn!("glory-desktop: webview re-issued Ready; remount is not supported yet");
                        return;
                    };
                    let holder = CommandHolder::new();
                    holder.set_coalesce(slot.config.coalesce);
                    let holder = mount(holder);
                    flush(&slot.webview, &holder);
                    slot.holder = Some(holder);
                    let _ = &slot.window;
                }
                Event::UserEvent(HostEvent::Dom(id, data)) => {
                    if let Some(slot) = slot_by_id(&mut slots, id)
                        && let Some(holder) = &slot.holder
                    {
                        holder.dispatch_event(*data);
                        flush(&slot.webview, holder);
                    }
                }
                Event::UserEvent(HostEvent::Query(id, response)) => {
                    if let Some(slot) = slot_by_id(&mut slots, id)
                        && let Some(holder) = &slot.holder
                    {
                        holder.resolve_query(response);
                        flush(&slot.webview, holder);
                    }
                }
                Event::UserEvent(HostEvent::Menu(menu_id)) => {
                    let Some(id) = menu_routes.get(&menu_id).copied() else { return };
                    if let Some(slot) = slot_by_id(&mut slots, id)
                        && let (Some(callback), Some(holder)) = (&slot.config.on_menu, &slot.holder)
                    {
                        holder.update(|| callback(holder, &menu_id));
                        flush(&slot.webview, holder);
                    }
                }
                Event::UserEvent(HostEvent::WindowCommand { id, command }) => {
                    apply_window_command(&mut slots, id, command);
                    if slots.is_empty() {
                        *control_flow = ControlFlow::Exit;
                    }
                }
                Event::UserEvent(HostEvent::OpenQueuedWindows) => {
                    let pending = window_queue.borrow_mut().drain(..).collect::<Vec<_>>();
                    for window in pending {
                        create_window(
                            target,
                            window,
                            proxy.clone(),
                            window_queue.clone(),
                            next_window_index.clone(),
                            &mut slots,
                            &mut menu_routes,
                        );
                    }
                }
                Event::UserEvent(HostEvent::Reload(message)) => match message {
                    ReloadMessage::Full => {
                        // The CLI rebuilds and restarts this process for full
                        // reloads; reaching here means only assets changed.
                        tracing::info!("glory-desktop: full reload requested (handled by glory-cli process restart)");
                    }
                    ReloadMessage::Style { css_path } => {
                        // Same link-swap dance as the browser reload script.
                        let path = serde_json::to_string(&css_path).expect("string serializes");
                        let script = format!(
                            "(() => {{ for (const link of document.querySelectorAll('link[rel=stylesheet]')) {{ if (link.href.includes({path})) {{ const next = link.cloneNode(); next.href = link.href.split('?')[0] + '?' + Date.now(); link.replaceWith(next); return; }} }} console.warn('glory: no stylesheet matching', {path}); }})();"
                        );
                        for (_, slot) in &slots {
                            if let Err(err) = slot.webview.evaluate_script(&script) {
                                tracing::error!(%err, "glory-desktop: style reload failed");
                            }
                        }
                    }
                    ReloadMessage::Functions { payload } => match serde_json::from_str::<FunctionReloadBatch>(&payload) {
                        Ok(batch) => {
                            for (_, slot) in &slots {
                                if let (Some(callback), Some(holder)) = (&slot.config.on_function_reload, &slot.holder) {
                                    holder.update(|| callback(holder, batch.clone()));
                                    flush(&slot.webview, holder);
                                }
                            }
                        }
                        Err(err) => tracing::warn!(%err, "glory-desktop: undecodable function reload payload"),
                    },
                },
                _ => {}
            }
        })
    }
}

fn create_window(
    target: &EventLoopWindowTarget<HostEvent>,
    pending: PendingWindow,
    proxy: EventLoopProxy<HostEvent>,
    window_queue: Rc<RefCell<Vec<PendingWindow>>>,
    next_window_index: Rc<Cell<usize>>,
    slots: &mut Vec<(WindowId, WindowSlot)>,
    menu_routes: &mut HashMap<String, DesktopWindowId>,
) {
    let PendingWindow { id, config, state, mount } = pending;
    let window = WindowBuilder::new()
        .with_title(&config.title)
        .with_inner_size(tao::dpi::LogicalSize::new(config.inner_size.0, config.inner_size.1))
        .with_resizable(config.resizable)
        .build(target)
        .expect("glory-desktop: failed to create window");

    {
        let mut state = state.borrow_mut();
        state.focused = true;
        state.maximized = window.is_maximized();
        state.fullscreen = window.fullscreen().is_some();
    }

    let assets_root_dir = assets_root(&config);
    install_bundle_asset_manifest(&assets_root_dir);

    let handle = DesktopWindowHandle::new(id, proxy.clone(), state.clone(), window_queue, next_window_index);
    let mount = mount(handle);
    let ipc_proxy = proxy.clone();
    let webview = WebViewBuilder::new()
        .with_initialization_script(crate::WRY_INTERPRETER_JS)
        .with_html(BOOTSTRAP_HTML)
        .with_devtools(config.devtools)
        .with_custom_protocol("glory".into(), move |_webview_id, request| serve_asset(&assets_root_dir, request))
        .with_ipc_handler(
            move |request: wry::http::Request<String>| match serde_json::from_str::<IpcMessage>(request.body()) {
                Ok(IpcMessage::GloryWryReady(_)) => {
                    let _ = ipc_proxy.send_event(HostEvent::Ready(id));
                }
                Ok(IpcMessage::GloryWryEvent(data)) => {
                    let _ = ipc_proxy.send_event(HostEvent::Dom(id, data));
                }
                Ok(IpcMessage::GloryWryQuery(response)) => {
                    let _ = ipc_proxy.send_event(HostEvent::Query(id, response));
                }
                Err(err) => {
                    tracing::warn!(%err, "glory-desktop: undecodable IPC message");
                }
            },
        )
        .build(&window)
        .expect("glory-desktop: failed to create webview");

    let menu = config.menu.as_ref().map(|spec| {
        let menu = build_menu(spec, id, menu_routes);
        attach_menu(&menu, &window);
        menu
    });

    slots.push((
        window.id(),
        WindowSlot {
            id,
            window,
            webview,
            holder: None,
            mount: Some(mount),
            config,
            state,
            menu,
        },
    ));
}

fn slot_by_id<'s>(slots: &'s mut [(WindowId, WindowSlot)], id: DesktopWindowId) -> Option<&'s mut WindowSlot> {
    slots.iter_mut().map(|(_, slot)| slot).find(|slot| slot.id == id)
}

fn slot_by_window_id<'s>(slots: &'s mut [(WindowId, WindowSlot)], window_id: WindowId) -> Option<&'s mut WindowSlot> {
    slots.iter_mut().find(|(id, _)| *id == window_id).map(|(_, slot)| slot)
}

fn close_slot_by_window_id(slots: &mut Vec<(WindowId, WindowSlot)>, window_id: WindowId) -> bool {
    let Some(position) = slots.iter().position(|(id, _)| *id == window_id) else {
        return false;
    };
    let (_, slot) = slots.remove(position);
    slot.state.borrow_mut().closed = true;
    true
}

fn close_slot_by_id(slots: &mut Vec<(WindowId, WindowSlot)>, id: DesktopWindowId) -> bool {
    let Some(position) = slots.iter().position(|(_, slot)| slot.id == id) else {
        return false;
    };
    let (_, slot) = slots.remove(position);
    slot.state.borrow_mut().closed = true;
    true
}

fn apply_window_command(slots: &mut Vec<(WindowId, WindowSlot)>, id: DesktopWindowId, command: WindowCommand) {
    if matches!(command, WindowCommand::Close) {
        close_slot_by_id(slots, id);
        return;
    }

    let Some(slot) = slot_by_id(slots, id) else {
        tracing::warn!(window_id = id.as_usize(), "glory-desktop: window command target no longer exists");
        return;
    };

    match command {
        WindowCommand::DragWindow => {
            if let Err(err) = slot.window.drag_window() {
                tracing::warn!(%err, window_id = id.as_usize(), "glory-desktop: drag_window failed");
            }
        }
        WindowCommand::SetFullscreen(fullscreen) => {
            slot.window.set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
            slot.state.borrow_mut().fullscreen = fullscreen;
        }
        WindowCommand::SetMaximized(maximized) => {
            slot.window.set_maximized(maximized);
            slot.state.borrow_mut().maximized = maximized;
        }
        WindowCommand::Focus => {
            slot.window.set_focus();
            slot.state.borrow_mut().focused = true;
        }
        WindowCommand::SetZoomLevel(zoom_level) => {
            if let Err(err) = slot.webview.zoom(zoom_level) {
                tracing::warn!(%err, window_id = id.as_usize(), "glory-desktop: set_zoom_level failed");
            } else {
                slot.state.borrow_mut().zoom_level = zoom_level;
            }
        }
        WindowCommand::Close => unreachable!("handled before slot lookup"),
    }
}

fn build_menu(spec: &MenuSpec, window_id: DesktopWindowId, routes: &mut HashMap<String, DesktopWindowId>) -> muda::Menu {
    let menu = muda::Menu::new();
    for (title, items) in &spec.submenus {
        let submenu = muda::Submenu::new(title, true);
        for item in items {
            routes.insert(item.id.clone(), window_id);
            let menu_item = muda::MenuItem::with_id(muda::MenuId(item.id.clone()), &item.label, true, None);
            if let Err(err) = submenu.append(&menu_item) {
                tracing::error!(%err, "glory-desktop: failed to append menu item");
            }
        }
        if let Err(err) = menu.append(&submenu) {
            tracing::error!(%err, "glory-desktop: failed to append submenu");
        }
    }
    menu
}

#[allow(unused_variables)]
// muda's per-window attachment takes a raw HWND; this is the one
// unavoidable unsafe call in the desktop host (workspace denies
// unsafe_code elsewhere).
#[allow(unsafe_code)]
fn attach_menu(menu: &muda::Menu, window: &Window) {
    #[cfg(target_os = "windows")]
    {
        use tao::platform::windows::WindowExtWindows;
        if let Err(err) = unsafe { menu.init_for_hwnd(window.hwnd()) } {
            tracing::error!(%err, "glory-desktop: failed to attach window menu");
        }
    }
    #[cfg(target_os = "macos")]
    {
        menu.init_for_nsapp();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        tracing::warn!("glory-desktop: window menus are not wired up on this platform yet");
    }
}

/// Opens a window and runs `widget` in it with default [`DesktopConfig`].
/// Never returns; the process exits with the window.
pub fn launch<W>(widget: impl FnOnce() -> W + 'static) -> !
where
    W: Widget + 'static,
{
    launch_with_config(DesktopConfig::default(), widget)
}

/// See [`launch`]. For multiple windows use [`Desktop`].
pub fn launch_with_config<W>(config: DesktopConfig, widget: impl FnOnce() -> W + 'static) -> !
where
    W: Widget + 'static,
{
    Desktop::new().window(config, widget).run()
}

/// See [`launch_with_config`], but passes a [`DesktopWindowHandle`] into the
/// root widget factory so callbacks can control the native window.
pub fn launch_with_handle<W>(config: DesktopConfig, widget: impl FnOnce(DesktopWindowHandle) -> W + 'static) -> !
where
    W: Widget + 'static,
{
    Desktop::new().window_with_handle(config, widget).run()
}

fn flush(webview: &wry::WebView, holder: &CommandHolder) {
    let batch = holder.take_batch();
    if batch.is_empty() {
        return;
    }
    let json = serde_json::to_string(&batch).expect("renderer commands always serialize");
    let script = format!("window.__gloryApplyWryBatch({json});");
    if let Err(err) = webview.evaluate_script(&script) {
        tracing::error!(%err, "glory-desktop: failed to flush command batch");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_path_resolution_rejects_traversal() {
        let root = std::env::temp_dir().join("glory-asset-test");
        std::fs::create_dir_all(root.join("img")).unwrap();
        std::fs::write(root.join("img/logo.png"), b"png").unwrap();

        assert!(resolve_asset_path(&root, "/img/logo.png").is_some());
        assert!(resolve_asset_path(&root, "img/logo.png").is_some());
        assert!(resolve_asset_path(&root, "/missing.png").is_none());
        assert!(resolve_asset_path(&root, "/").is_none());
        assert!(resolve_asset_path(&root, "/../secret.txt").is_none());
        assert!(resolve_asset_path(&root, "/img/../../secret.txt").is_none());
    }

    #[test]
    fn mime_table_covers_common_types() {
        assert_eq!(mime_for(std::path::Path::new("a.css")), "text/css");
        assert_eq!(mime_for(std::path::Path::new("a.wasm")), "application/wasm");
        assert_eq!(mime_for(std::path::Path::new("a.svg")), "image/svg+xml");
        assert_eq!(mime_for(std::path::Path::new("a.unknown")), "application/octet-stream");
    }

    #[test]
    fn asset_url_normalizes_path() {
        let url = asset_url("assets/logo.png");
        assert!(url.ends_with("/assets/logo.png"), "{url}");
        assert_eq!(asset_url("/x.css"), asset_url("x.css"));
    }

    #[test]
    fn installs_bundle_asset_manifest_from_assets_root() {
        glory_core::assets::clear_asset_manifest();

        let root = std::env::temp_dir().join(format!("glory-desktop-manifest-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("glory-bundle.json"),
            r#"{
                "asset_map": {
                    "/assets/logo.png": "/assets/logo.0123456789abcdef.png"
                }
            }"#,
        )
        .unwrap();

        install_bundle_asset_manifest(&root);
        let asset = glory_core::assets::Asset::from_static("assets/logo.png", "assets/logo.png");
        assert_eq!(asset.public_path(), "/assets/logo.0123456789abcdef.png");

        glory_core::assets::clear_asset_manifest();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_bundle_asset_manifest_clears_previous_mapping() {
        glory_core::assets::install_asset_manifest(glory_core::assets::AssetManifest::from_mappings([(
            "/assets/logo.png",
            "/assets/logo.hashed.png",
        )]));

        let root = std::env::temp_dir().join(format!("glory-desktop-missing-manifest-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        install_bundle_asset_manifest(&root);

        let asset = glory_core::assets::Asset::from_static("assets/logo.png", "assets/logo.png");
        assert_eq!(asset.public_path(), "/assets/logo.png");

        glory_core::assets::clear_asset_manifest();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn menu_spec_builds_routes() {
        let spec = MenuSpec::new().submenu("File", vec![MenuItemSpec::new("open", "Open"), MenuItemSpec::new("quit", "Quit")]);
        let mut routes = HashMap::new();
        let window_id = DesktopWindowId(3);
        let _menu = build_menu(&spec, window_id, &mut routes);
        assert_eq!(routes.get("open"), Some(&window_id));
        assert_eq!(routes.get("quit"), Some(&window_id));
    }
}
