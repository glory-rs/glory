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

use std::borrow::Cow;
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
use wry::{WebView, WebViewBuilder, WebViewId};

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
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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

/// RGBA image used for tray icons.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrayIconImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl TrayIconImage {
    pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Self {
        Self { rgba, width, height }
    }
}

/// System tray icon configuration for a desktop window.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrayIconSpec {
    pub id: String,
    pub tooltip: Option<String>,
    pub title: Option<String>,
    pub icon: Option<TrayIconImage>,
    pub icon_is_template: bool,
    pub menu: Option<MenuSpec>,
    pub menu_on_left_click: bool,
    pub menu_on_right_click: bool,
}

impl TrayIconSpec {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            menu_on_left_click: true,
            menu_on_right_click: true,
            ..Default::default()
        }
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn icon_rgba(mut self, rgba: Vec<u8>, width: u32, height: u32) -> Self {
        self.icon = Some(TrayIconImage::from_rgba(rgba, width, height));
        self
    }

    pub fn icon_template(mut self, value: bool) -> Self {
        self.icon_is_template = value;
        self
    }

    pub fn menu(mut self, menu: MenuSpec) -> Self {
        self.menu = Some(menu);
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopTrayMouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopTrayMouseButtonState {
    Up,
    Down,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DesktopTrayEvent {
    Click {
        id: String,
        button: DesktopTrayMouseButton,
        button_state: DesktopTrayMouseButtonState,
    },
    DoubleClick {
        id: String,
        button: DesktopTrayMouseButton,
    },
    Enter {
        id: String,
    },
    Move {
        id: String,
    },
    Leave {
        id: String,
    },
}

/// Global hotkey configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopHotKeySpec {
    pub id: String,
    pub accelerator: String,
}

impl DesktopHotKeySpec {
    pub fn new(id: impl Into<String>, accelerator: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            accelerator: accelerator.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopHotKeyState {
    Pressed,
    Released,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopHotKeyEvent {
    pub id: String,
    pub accelerator: String,
    pub state: DesktopHotKeyState,
}

/// Request received by a desktop custom protocol handler.
pub type DesktopProtocolRequest = wry::http::Request<Vec<u8>>;

/// Response sent from a desktop custom protocol handler.
pub type DesktopProtocolResponse = wry::http::Response<Cow<'static, [u8]>>;

type DesktopProtocolCallback = Rc<dyn Fn(WebViewId, DesktopProtocolRequest, wry::RequestAsyncResponder)>;

/// Asynchronous custom protocol registered on each desktop webview.
///
/// The handler receives Wry's [`RequestAsyncResponder`], so slow work can be
/// moved to a thread or async runtime before calling `respond`.
#[derive(Clone)]
pub struct DesktopProtocol {
    name: String,
    handler: DesktopProtocolCallback,
}

impl DesktopProtocol {
    pub fn new(name: impl Into<String>, handler: impl Fn(WebViewId, DesktopProtocolRequest, wry::RequestAsyncResponder) + 'static) -> Self {
        let name = name.into();
        assert!(
            !name.eq_ignore_ascii_case("glory"),
            "`glory` is reserved for the built-in desktop asset protocol"
        );
        Self {
            name,
            handler: Rc::new(handler),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Debug for DesktopProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesktopProtocol").field("name", &self.name).finish()
    }
}

/// Builds a response suitable for [`wry::RequestAsyncResponder::respond`].
pub fn desktop_protocol_response(status: u16, mime: &str, body: impl Into<Cow<'static, [u8]>>) -> DesktopProtocolResponse {
    wry::http::Response::builder()
        .status(status)
        .header("content-type", mime)
        .body(body.into())
        .expect("desktop protocol response builds")
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
    /// Extra asynchronous custom protocols registered on the webview.
    ///
    /// `glory` is reserved for the built-in static asset protocol.
    pub custom_protocols: Vec<DesktopProtocol>,
    /// Native window menu.
    pub menu: Option<MenuSpec>,
    /// Invoked on the event-loop thread when a [`MenuSpec`] item is
    /// activated; receives the item id. Signal writes settle and flush
    /// automatically afterwards.
    pub on_menu: Option<Rc<dyn Fn(&CommandHolder, &str)>>,
    /// Optional system tray icon owned by this window.
    pub tray: Option<TrayIconSpec>,
    /// Invoked on the event-loop thread when the tray icon emits an event.
    pub on_tray: Option<Rc<dyn Fn(&CommandHolder, DesktopTrayEvent)>>,
    /// Global hotkeys registered while this window is alive.
    pub hotkeys: Vec<DesktopHotKeySpec>,
    /// Invoked on the event-loop thread when a registered global hotkey is
    /// pressed or released.
    pub on_hotkey: Option<Rc<dyn Fn(&CommandHolder, DesktopHotKeyEvent)>>,
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
            .field("custom_protocols", &self.custom_protocols)
            .field("menu", &self.menu)
            .field("on_menu", &self.on_menu.is_some())
            .field("tray", &self.tray)
            .field("on_tray", &self.on_tray.is_some())
            .field("hotkeys", &self.hotkeys)
            .field("on_hotkey", &self.on_hotkey.is_some())
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
            custom_protocols: Vec::new(),
            menu: None,
            on_menu: None,
            tray: None,
            on_tray: None,
            hotkeys: Vec::new(),
            on_hotkey: None,
        }
    }
}

impl DesktopConfig {
    /// Registers an asynchronous custom protocol on this window.
    pub fn with_custom_protocol(mut self, protocol: DesktopProtocol) -> Self {
        self.custom_protocols.push(protocol);
        self
    }

    pub fn with_tray(mut self, tray: TrayIconSpec) -> Self {
        self.tray = Some(tray);
        self
    }

    pub fn with_hotkey(mut self, hotkey: DesktopHotKeySpec) -> Self {
        self.hotkeys.push(hotkey);
        self
    }
}

const BOOTSTRAP_HTML: &str = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body></body></html>";

enum HostEvent {
    Ready(DesktopWindowId),
    Dom(DesktopWindowId, Box<EventData>),
    Query(DesktopWindowId, glory_core::renderer::QueryResponse),
    Reload(ReloadMessage),
    Menu(String),
    Tray(tray_icon::TrayIconEvent),
    HotKey(global_hotkey::GlobalHotKeyEvent),
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

fn serve_asset(root: &std::path::Path, request: DesktopProtocolRequest) -> DesktopProtocolResponse {
    let path = request.uri().path().to_owned();
    match resolve_asset_path(root, &path) {
        Some(file) => match std::fs::read(&file) {
            Ok(bytes) => desktop_protocol_response(200, mime_for(&file), bytes),
            Err(err) => {
                tracing::warn!(%err, %path, "glory-desktop: asset read failed");
                desktop_protocol_response(500, "text/plain", b"asset read failed".to_vec())
            }
        },
        None => desktop_protocol_response(404, "text/plain", b"not found".to_vec()),
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
    /// Keeps the tray icon alive for the window's lifetime.
    #[allow(dead_code)]
    tray: Option<tray_icon::TrayIcon>,
    registered_hotkeys: Vec<global_hotkey::hotkey::HotKey>,
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
        let mut tray_routes: HashMap<String, DesktopWindowId> = HashMap::new();
        let mut hotkey_routes: HashMap<u32, (DesktopWindowId, DesktopHotKeySpec)> = HashMap::new();
        let needs_hotkey_manager = self.windows.iter().any(|window| !window.config.hotkeys.is_empty());
        let mut hotkey_manager = needs_hotkey_manager.then(create_hotkey_manager).flatten();
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
                &mut tray_routes,
                hotkey_manager.as_ref(),
                &mut hotkey_routes,
            );
        }

        let menu_proxy = proxy.clone();
        muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
            let _ = menu_proxy.send_event(HostEvent::Menu(event.id().0.clone()));
        }));
        let tray_proxy = proxy.clone();
        tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
            let _ = tray_proxy.send_event(HostEvent::Tray(event));
        }));
        let hotkey_proxy = proxy.clone();
        global_hotkey::GlobalHotKeyEvent::set_event_handler(Some(move |event| {
            let _ = hotkey_proxy.send_event(HostEvent::HotKey(event));
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
                    close_slot_by_window_id(&mut slots, window_id, hotkey_manager.as_ref(), &mut hotkey_routes);
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
                Event::UserEvent(HostEvent::Tray(event)) => {
                    let Some(id) = tray_routes.get(event.id().as_ref()).copied() else { return };
                    if let Some(slot) = slot_by_id(&mut slots, id)
                        && let (Some(callback), Some(holder), Some(event)) = (&slot.config.on_tray, &slot.holder, map_tray_event(event))
                    {
                        holder.update(|| callback(holder, event));
                        flush(&slot.webview, holder);
                    }
                }
                Event::UserEvent(HostEvent::HotKey(event)) => {
                    let Some((id, spec)) = hotkey_routes.get(&event.id()).cloned() else { return };
                    if let Some(slot) = slot_by_id(&mut slots, id)
                        && let (Some(callback), Some(holder)) = (&slot.config.on_hotkey, &slot.holder)
                    {
                        let event = DesktopHotKeyEvent {
                            id: spec.id,
                            accelerator: spec.accelerator,
                            state: map_hotkey_state(event.state()),
                        };
                        holder.update(|| callback(holder, event));
                        flush(&slot.webview, holder);
                    }
                }
                Event::UserEvent(HostEvent::WindowCommand { id, command }) => {
                    apply_window_command(&mut slots, id, command, hotkey_manager.as_ref(), &mut hotkey_routes);
                    if slots.is_empty() {
                        *control_flow = ControlFlow::Exit;
                    }
                }
                Event::UserEvent(HostEvent::OpenQueuedWindows) => {
                    let pending = window_queue.borrow_mut().drain(..).collect::<Vec<_>>();
                    for window in pending {
                        if hotkey_manager.is_none() && !window.config.hotkeys.is_empty() {
                            hotkey_manager = create_hotkey_manager();
                        }
                        create_window(
                            target,
                            window,
                            proxy.clone(),
                            window_queue.clone(),
                            next_window_index.clone(),
                            &mut slots,
                            &mut menu_routes,
                            &mut tray_routes,
                            hotkey_manager.as_ref(),
                            &mut hotkey_routes,
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

fn create_hotkey_manager() -> Option<global_hotkey::GlobalHotKeyManager> {
    match global_hotkey::GlobalHotKeyManager::new() {
        Ok(manager) => Some(manager),
        Err(err) => {
            tracing::warn!(%err, "glory-desktop: global hotkey manager unavailable");
            None
        }
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
    tray_routes: &mut HashMap<String, DesktopWindowId>,
    hotkey_manager: Option<&global_hotkey::GlobalHotKeyManager>,
    hotkey_routes: &mut HashMap<u32, (DesktopWindowId, DesktopHotKeySpec)>,
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
    let mut webview = WebViewBuilder::new()
        .with_initialization_script(crate::WRY_INTERPRETER_JS)
        .with_html(BOOTSTRAP_HTML)
        .with_devtools(config.devtools)
        .with_asynchronous_custom_protocol("glory".into(), move |_webview_id, request, responder| {
            responder.respond(serve_asset(&assets_root_dir, request));
        })
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
        );

    for protocol in &config.custom_protocols {
        let name = protocol.name.clone();
        let handler = protocol.handler.clone();
        webview = webview.with_asynchronous_custom_protocol(name, move |webview_id, request, responder| {
            handler.as_ref()(webview_id, request, responder);
        });
    }

    let webview = webview.build(&window).expect("glory-desktop: failed to create webview");

    let menu = config.menu.as_ref().map(|spec| {
        let menu = build_menu(spec, id, menu_routes);
        attach_menu(&menu, &window);
        menu
    });
    let tray = config
        .tray
        .as_ref()
        .and_then(|spec| match build_tray(spec, id, tray_routes, menu_routes) {
            Ok(tray) => Some(tray),
            Err(err) => {
                tracing::warn!(%err, window_id = id.as_usize(), "glory-desktop: tray icon creation failed");
                None
            }
        });
    let registered_hotkeys = register_hotkeys(&config, id, hotkey_manager, hotkey_routes);

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
            tray,
            registered_hotkeys,
        },
    ));
}

fn slot_by_id<'s>(slots: &'s mut [(WindowId, WindowSlot)], id: DesktopWindowId) -> Option<&'s mut WindowSlot> {
    slots.iter_mut().map(|(_, slot)| slot).find(|slot| slot.id == id)
}

fn slot_by_window_id<'s>(slots: &'s mut [(WindowId, WindowSlot)], window_id: WindowId) -> Option<&'s mut WindowSlot> {
    slots.iter_mut().find(|(id, _)| *id == window_id).map(|(_, slot)| slot)
}

fn close_slot_by_window_id(
    slots: &mut Vec<(WindowId, WindowSlot)>,
    window_id: WindowId,
    hotkey_manager: Option<&global_hotkey::GlobalHotKeyManager>,
    hotkey_routes: &mut HashMap<u32, (DesktopWindowId, DesktopHotKeySpec)>,
) -> bool {
    let Some(position) = slots.iter().position(|(id, _)| *id == window_id) else {
        return false;
    };
    let (_, slot) = slots.remove(position);
    unregister_hotkeys(&slot, hotkey_manager, hotkey_routes);
    slot.state.borrow_mut().closed = true;
    true
}

fn close_slot_by_id(
    slots: &mut Vec<(WindowId, WindowSlot)>,
    id: DesktopWindowId,
    hotkey_manager: Option<&global_hotkey::GlobalHotKeyManager>,
    hotkey_routes: &mut HashMap<u32, (DesktopWindowId, DesktopHotKeySpec)>,
) -> bool {
    let Some(position) = slots.iter().position(|(_, slot)| slot.id == id) else {
        return false;
    };
    let (_, slot) = slots.remove(position);
    unregister_hotkeys(&slot, hotkey_manager, hotkey_routes);
    slot.state.borrow_mut().closed = true;
    true
}

fn apply_window_command(
    slots: &mut Vec<(WindowId, WindowSlot)>,
    id: DesktopWindowId,
    command: WindowCommand,
    hotkey_manager: Option<&global_hotkey::GlobalHotKeyManager>,
    hotkey_routes: &mut HashMap<u32, (DesktopWindowId, DesktopHotKeySpec)>,
) {
    if matches!(command, WindowCommand::Close) {
        close_slot_by_id(slots, id, hotkey_manager, hotkey_routes);
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

fn build_tray(
    spec: &TrayIconSpec,
    window_id: DesktopWindowId,
    tray_routes: &mut HashMap<String, DesktopWindowId>,
    menu_routes: &mut HashMap<String, DesktopWindowId>,
) -> Result<tray_icon::TrayIcon, String> {
    let mut builder = tray_icon::TrayIconBuilder::new()
        .with_id(spec.id.clone())
        .with_menu_on_left_click(spec.menu_on_left_click)
        .with_menu_on_right_click(spec.menu_on_right_click)
        .with_icon_as_template(spec.icon_is_template);
    if let Some(tooltip) = &spec.tooltip {
        builder = builder.with_tooltip(tooltip);
    }
    if let Some(title) = &spec.title {
        builder = builder.with_title(title);
    }
    if let Some(icon) = &spec.icon {
        let icon = tray_icon::Icon::from_rgba(icon.rgba.clone(), icon.width, icon.height).map_err(|err| err.to_string())?;
        builder = builder.with_icon(icon);
    }
    if let Some(menu) = &spec.menu {
        builder = builder.with_menu(Box::new(build_menu(menu, window_id, menu_routes)));
    }
    tray_routes.insert(spec.id.clone(), window_id);
    builder.build().map_err(|err| err.to_string())
}

fn register_hotkeys(
    config: &DesktopConfig,
    window_id: DesktopWindowId,
    manager: Option<&global_hotkey::GlobalHotKeyManager>,
    routes: &mut HashMap<u32, (DesktopWindowId, DesktopHotKeySpec)>,
) -> Vec<global_hotkey::hotkey::HotKey> {
    let Some(manager) = manager else {
        if !config.hotkeys.is_empty() {
            tracing::warn!(
                window_id = window_id.as_usize(),
                "glory-desktop: skipping hotkeys because manager is unavailable"
            );
        }
        return Vec::new();
    };

    let mut registered = Vec::new();
    for spec in &config.hotkeys {
        match spec.accelerator.parse::<global_hotkey::hotkey::HotKey>() {
            Ok(hotkey) => match manager.register(hotkey) {
                Ok(()) => {
                    routes.insert(hotkey.id(), (window_id, spec.clone()));
                    registered.push(hotkey);
                }
                Err(err) => tracing::warn!(%err, id = %spec.id, accelerator = %spec.accelerator, "glory-desktop: hotkey registration failed"),
            },
            Err(err) => tracing::warn!(%err, id = %spec.id, accelerator = %spec.accelerator, "glory-desktop: invalid hotkey accelerator"),
        }
    }
    registered
}

fn unregister_hotkeys(
    slot: &WindowSlot,
    manager: Option<&global_hotkey::GlobalHotKeyManager>,
    routes: &mut HashMap<u32, (DesktopWindowId, DesktopHotKeySpec)>,
) {
    for hotkey in &slot.registered_hotkeys {
        routes.remove(&hotkey.id());
        if let Some(manager) = manager
            && let Err(err) = manager.unregister(*hotkey)
        {
            tracing::warn!(%err, window_id = slot.id.as_usize(), hotkey = %hotkey, "glory-desktop: hotkey unregister failed");
        }
    }
}

fn map_tray_event(event: tray_icon::TrayIconEvent) -> Option<DesktopTrayEvent> {
    match event {
        tray_icon::TrayIconEvent::Click {
            id, button, button_state, ..
        } => Some(DesktopTrayEvent::Click {
            id: id.as_ref().to_owned(),
            button: map_tray_button(button),
            button_state: map_tray_button_state(button_state),
        }),
        tray_icon::TrayIconEvent::DoubleClick { id, button, .. } => Some(DesktopTrayEvent::DoubleClick {
            id: id.as_ref().to_owned(),
            button: map_tray_button(button),
        }),
        tray_icon::TrayIconEvent::Enter { id, .. } => Some(DesktopTrayEvent::Enter { id: id.as_ref().to_owned() }),
        tray_icon::TrayIconEvent::Move { id, .. } => Some(DesktopTrayEvent::Move { id: id.as_ref().to_owned() }),
        tray_icon::TrayIconEvent::Leave { id, .. } => Some(DesktopTrayEvent::Leave { id: id.as_ref().to_owned() }),
        _ => None,
    }
}

fn map_tray_button(button: tray_icon::MouseButton) -> DesktopTrayMouseButton {
    match button {
        tray_icon::MouseButton::Left => DesktopTrayMouseButton::Left,
        tray_icon::MouseButton::Right => DesktopTrayMouseButton::Right,
        tray_icon::MouseButton::Middle => DesktopTrayMouseButton::Middle,
    }
}

fn map_tray_button_state(state: tray_icon::MouseButtonState) -> DesktopTrayMouseButtonState {
    match state {
        tray_icon::MouseButtonState::Up => DesktopTrayMouseButtonState::Up,
        tray_icon::MouseButtonState::Down => DesktopTrayMouseButtonState::Down,
    }
}

fn map_hotkey_state(state: global_hotkey::HotKeyState) -> DesktopHotKeyState {
    match state {
        global_hotkey::HotKeyState::Pressed => DesktopHotKeyState::Pressed,
        global_hotkey::HotKeyState::Released => DesktopHotKeyState::Released,
    }
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
    fn desktop_protocol_response_sets_status_type_and_body() {
        let response = desktop_protocol_response(202, "application/json", b"{}".to_vec());
        assert_eq!(response.status(), 202);
        assert_eq!(response.headers()["content-type"].to_str().unwrap(), "application/json");
        assert_eq!(response.body().as_ref(), b"{}");
    }

    #[test]
    fn desktop_config_records_custom_protocols() {
        let protocol = DesktopProtocol::new("api", |_webview_id, _request, _responder| {});
        let config = DesktopConfig::default().with_custom_protocol(protocol);

        assert_eq!(config.custom_protocols[0].name(), "api");
        assert!(format!("{config:?}").contains("api"));
    }

    #[test]
    fn desktop_config_records_tray_and_hotkeys() {
        let tray = TrayIconSpec::new("main-tray")
            .tooltip("Glory")
            .title("G")
            .icon_rgba(vec![255, 0, 0, 255], 1, 1)
            .menu(MenuSpec::new().submenu("App", vec![MenuItemSpec::new("quit", "Quit")]));
        let config = DesktopConfig::default()
            .with_tray(tray)
            .with_hotkey(DesktopHotKeySpec::new("toggle", "cmdorctrl+KeyK"));

        assert_eq!(config.tray.as_ref().unwrap().id, "main-tray");
        assert_eq!(config.tray.as_ref().unwrap().icon.as_ref().unwrap().width, 1);
        assert_eq!(config.hotkeys[0].id, "toggle");
        assert!(config.hotkeys[0].accelerator.parse::<global_hotkey::hotkey::HotKey>().is_ok());
        assert!(format!("{config:?}").contains("main-tray"));
    }

    #[test]
    fn tray_and_hotkey_event_mapping_is_stable() {
        assert_eq!(map_tray_button(tray_icon::MouseButton::Right), DesktopTrayMouseButton::Right);
        assert_eq!(
            map_tray_button_state(tray_icon::MouseButtonState::Down),
            DesktopTrayMouseButtonState::Down
        );
        assert_eq!(map_hotkey_state(global_hotkey::HotKeyState::Released), DesktopHotKeyState::Released);
    }

    #[test]
    #[should_panic(expected = "reserved")]
    fn desktop_protocol_rejects_builtin_glory_scheme() {
        let _ = DesktopProtocol::new("glory", |_webview_id, _request, _responder| {});
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
