//! Desktop lifecycle. Wry callbacks enqueue strings; Rust UI stays on this loop.
use crate::Renderer;
mod assets;
pub mod browser;
pub use assets::{AssetProvider, AssetSource, MAX_RANGE_BYTES};
use glory_core::executor::Executor;
use glory_ui::{App, Widget};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    rc::Rc,
    sync::Arc,
    task::{Wake, Waker},
    time::{Duration, Instant},
};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};
use wry::{WebViewBuilder, http::Response};

#[derive(Debug)]
pub struct ShellError(pub String);
impl std::fmt::Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ShellError {}
impl From<glory_ui::Error> for ShellError {
    fn from(error: glory_ui::Error) -> Self {
        Self(error.to_string())
    }
}
impl From<glory_render::Error> for ShellError {
    fn from(error: glory_render::Error) -> Self {
        Self(error.to_string())
    }
}

/// Explicit immutable local resources. Paths are exact URL paths, never files.
pub struct Asset {
    pub mime: String,
    pub bytes: Vec<u8>,
}
/// Optional application-owned native shell integration. Every method runs on
/// the window thread, outside UI runtime borrows. Native callbacks must enqueue
/// work and wake this loop; they must not invoke application UI closures.
pub trait NativeLifecycle {
    fn opened(&mut self, _window: &NativeWindow, _wake: &Waker) -> Result<(), ShellError> {
        Ok(())
    }
    /// Return false to keep a reachable application window alive (e.g. tray).
    fn close_requested(&mut self, _window: &NativeWindow) -> Result<bool, ShellError> {
        Ok(true)
    }
    /// Return true to finish the shell. The application still owns shutdown.
    fn poll(&mut self, _window: &NativeWindow) -> Result<bool, ShellError> {
        Ok(false)
    }
    fn closed(&mut self, _window: &NativeWindow) {}
}
#[cfg(target_os = "windows")]
pub use tao::platform::windows::WindowExtWindows as NativeWindowExtWindows;
pub use tao::window::Window as NativeWindow;
pub struct Config {
    /// No tray, startup registration, updater, or platform hooks by default.
    pub native: Option<Box<dyn NativeLifecycle>>,
    pub title: String,
    /// Explicit isolated native HTTP(S) reader windows; absent by default.
    pub browser: Option<browser::Controller>,
    pub width: f64,
    pub height: f64,
    pub css: String,
    pub assets: BTreeMap<String, Asset>,
    /// Explicit capability provider for bounded asynchronous local range reads.
    pub asset_provider: Option<Arc<dyn AssetProvider>>,
    /// Trusted application bootstrap, run at document start after the bridge.
    pub initialization_script: Option<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            native: None,
            title: "Glory".into(),
            browser: None,
            width: 800.,
            height: 600.,
            css: String::new(),
            assets: BTreeMap::new(),
            asset_provider: None,
            initialization_script: None,
        }
    }
}
impl Config {
    fn document(&self) -> Result<String, ShellError> {
        if !self.width.is_finite()
            || !self.height.is_finite()
            || self.width <= 0.
            || self.height <= 0.
        {
            return Err(ShellError("invalid Wry window dimensions".into()));
        }
        let mut bytes = self.css.len();
        for (path, asset) in &self.assets {
            if !path.starts_with('/')
                || path.contains(['?', '#', '\\'])
                || path.split('/').any(|p| p == ".." || p == ".")
                || path == "/index.html"
            {
                return Err(ShellError("invalid Wry asset path".into()));
            }
            if wry::http::HeaderValue::from_str(&asset.mime).is_err() {
                return Err(ShellError("invalid Wry asset MIME type".into()));
            }
            bytes = bytes.saturating_add(asset.bytes.len());
        }
        if bytes > 32 * 1024 * 1024 {
            return Err(ShellError("Wry local asset budget exceeded".into()));
        }
        let css = self.css.replace('<', "\\3c ");
        Ok(format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>{}</style><style id=\"glory-author\">{css}</style></head><body><div id=\"glory-root\"></div></body></html>",
            glory_html::escape(&self.title),
            glory_html::BASE_CSS
        ))
    }
}

pub struct WindowApp {
    app: Option<App>,
    renderer: Renderer,
    executor: Rc<Executor>,
}
impl Default for WindowApp {
    fn default() -> Self {
        Self::new()
    }
}
impl WindowApp {
    pub fn new() -> Self {
        let executor = Rc::new(Executor::new(4096));
        let mut renderer = None;
        let app = App::with_host(executor.clone(), |runtime| {
            let output = Renderer::new(runtime);
            renderer = Some(output.clone());
            output
        });
        Self {
            app: Some(app),
            renderer: renderer.expect("renderer factory"),
            executor,
        }
    }
    pub fn app(&self) -> &App {
        self.app.as_ref().expect("live Wry app")
    }
    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }
    pub fn mount(&self, widget: impl Widget) -> glory_ui::Result {
        self.app().mount(widget)
    }
    pub fn mount_with<W: Widget>(&self, factory: impl FnOnce() -> W) -> glory_ui::Result {
        self.app().mount_with(factory)
    }
    fn observe(&self, waker: &Waker) -> Result<(), ShellError> {
        self.executor.observe(waker);
        self.executor.dirty();
        for _ in 0..64 {
            if !self.executor.poll_one() {
                break;
            }
        }
        let turn = self.app().settle(App::DEFAULT_SETTLE_PASSES)?;
        self.executor.dirty();
        if turn.pending || self.executor.work() {
            waker.wake_by_ref();
        }
        Ok(())
    }
}
impl Drop for WindowApp {
    fn drop(&mut self) {
        drop(self.app.take());
        self.renderer.close();
        self.executor.close();
    }
}

enum Message {
    Wake,
    Ipc(String),
}
struct LoopWake(EventLoopProxy<Message>);
impl Wake for LoopWake {
    fn wake(self: Arc<Self>) {
        let _ = self.0.send_event(Message::Wake);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        let _ = self.0.send_event(Message::Wake);
    }
}
pub struct Shell {
    event_loop: EventLoop<Message>,
}
impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}
impl Shell {
    /// Construct on the platform UI/main thread.
    pub fn new() -> Self {
        Self {
            event_loop: EventLoopBuilder::with_user_event().build(),
        }
    }
    pub fn run(self, app: WindowApp, config: Config) -> Result<(), ShellError> {
        self.run_until(app, config, |_| Ok(false))
    }
    /// A local lifecycle hook for application shutdown and native acceptance.
    /// Called after bounded host turns, outside renderer and executor borrows.
    pub fn run_until(
        mut self,
        app: WindowApp,
        config: Config,
        mut finished: impl FnMut(&WindowApp) -> Result<bool, ShellError>,
    ) -> Result<(), ShellError> {
        let html = config.document()?;
        let window = WindowBuilder::new()
            .with_title(&config.title)
            .with_inner_size(tao::dpi::LogicalSize::new(config.width, config.height))
            .build(&self.event_loop)
            .map_err(|e| ShellError(e.to_string()))?;
        let proxy = self.event_loop.create_proxy();
        let waker = Waker::from(Arc::new(LoopWake(proxy.clone())));
        let mut browser_host = config
            .browser
            .map(|controller| browser::Host::new(controller, waker.clone()))
            .transpose()?;
        let initial_url = if cfg!(target_os = "windows") {
            "http://glory.localhost/index.html"
        } else {
            "glory://localhost/index.html"
        };
        let index = html.into_bytes();
        let assets = config.assets;
        let local_media = app.renderer.0.borrow().local_media;
        let media_policy = if local_media {
            "; media-src glory: http://glory.localhost"
        } else {
            ""
        };
        let policy = format!(
            "default-src 'none'; script-src 'none'; style-src 'unsafe-inline' glory: http://glory.localhost; img-src glory: http://glory.localhost data:; font-src glory: http://glory.localhost; base-uri 'none'; frame-src 'none'; form-action 'none'{media_policy}"
        );
        let asset_pool = config.asset_provider.map(assets::Pool::new).transpose()?;
        let builder = WebViewBuilder::new()
            .with_asynchronous_custom_protocol("glory".into(), move |_, request, responder| {
                let path = request.uri().path();
                if request
                    .uri()
                    .host()
                    .is_none_or(|host| host == "localhost" || host == "glory.localhost")
                    && path != "/index.html"
                    && !assets.contains_key(path)
                    && let Some(pool) = &asset_pool
                {
                    pool.submit(request, responder);
                    return;
                }
                let (status, mime, body) = if request.method() != "GET"
                    || request
                        .uri()
                        .host()
                        .is_some_and(|host| host != "localhost" && host != "glory.localhost")
                {
                    (403, "text/plain", Vec::new())
                } else if path == "/index.html" {
                    (200, "text/html; charset=utf-8", index.clone())
                } else if let Some(asset) = assets.get(path) {
                    (200, asset.mime.as_str(), asset.bytes.clone())
                } else {
                    (404, "text/plain", Vec::new())
                };
                responder.respond(
                    Response::builder()
                        .status(status)
                        .header("Content-Type", mime)
                        .header("X-Content-Type-Options", "nosniff")
                        .header("Content-Security-Policy", policy.as_str())
                        .body(Cow::Owned(body))
                        .expect("validated static response headers"),
                );
            })
            .with_initialization_script(app.renderer.initialization_script())
            .with_navigation_handler(move |url| url == initial_url)
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .with_ipc_handler(move |request| {
                // Callbacks never borrow UI or invoke user closures.
                if request.body().len() <= crate::protocol::MAX_IPC_BYTES {
                    let _ = proxy.send_event(Message::Ipc(request.into_body()));
                }
            })
            .with_url(initial_url);
        let builder = if let Some(script) = config.initialization_script {
            builder.with_initialization_script(script)
        } else {
            builder
        };
        #[cfg(target_os = "linux")]
        let webview = {
            use tao::platform::unix::WindowExtUnix;
            use wry::WebViewBuilderExtUnix;
            builder.build_gtk(window.default_vbox().expect("Tao GTK container"))
        };
        #[cfg(not(target_os = "linux"))]
        let webview = builder.build(&window);
        let webview = webview.map_err(|e| ShellError(e.to_string()))?;
        let mut native = config.native;
        if let Some(host) = &mut native {
            host.opened(&window, &waker)?;
        }
        let mut result = Ok(());
        let mut ready = false;
        let mut exiting = false;
        let mut deadline = Some(Instant::now() + Duration::from_secs(15));
        self.event_loop.run_return(|event,target,flow| {
            if exiting {*flow=ControlFlow::Exit;return;}
            *flow=deadline.map_or(ControlFlow::Wait,ControlFlow::WaitUntil);
            if let Event::WindowEvent {window_id,event:ref window_event,..}=event
                && browser_host.as_mut().is_some_and(|host|host.window_event(window_id,window_event)) {return;}
            if matches!(event,Event::WindowEvent{window_id,event:WindowEvent::CloseRequested,..} if window_id==window.id()) {
                match native.as_mut().map_or(Ok(true),|host|host.close_requested(&window)) {
                    Ok(false)=>return,
                    Err(error)=>result=Err(error),
                    Ok(true)=>{},
                }
                exiting=true;*flow=ControlFlow::Exit;return;
            }
            let turn=(||->Result<bool,ShellError>{
                if let Event::UserEvent(Message::Ipc(message))=&event {
                    app.renderer.receive(message)?;
                    ready |= app.renderer.0.borrow().transport.ready;
                    if ready && !app.renderer.in_flight() {deadline=None;}
                }
                if !matches!(event,Event::MainEventsCleared|Event::UserEvent(_)|Event::NewEvents(_)) {return Ok(false);}
                if deadline.is_some_and(|deadline|Instant::now()>=deadline) {return Err(ShellError("Wry document/acknowledgement timed out".into()));}
                if let Some(host)=&mut browser_host {host.poll(target);}
                if native.as_mut().is_some_and(|host|match host.poll(&window) {Ok(quit)=>quit,Err(error)=>{result=Err(error);true}}) {return Ok(true);}
                app.observe(&waker)?;
                if let Some(script)=app.renderer.take_script()? {
                    webview.evaluate_script(&script).map_err(|e|ShellError(e.to_string()))?;
                    deadline=Some(Instant::now()+Duration::from_secs(15));
                }
                finished(&app)
            })();
            match turn {Ok(true)=>{exiting=true;*flow=ControlFlow::Exit;},Err(error)=>{result=Err(error);exiting=true;*flow=ControlFlow::Exit;},_=>{}}
            if !matches!(*flow,ControlFlow::Exit) {*flow=deadline.map_or(ControlFlow::Wait,ControlFlow::WaitUntil);}
        });
        if let Some(host) = &mut native {
            host.closed(&window);
        }
        drop(native);
        drop(browser_host);
        drop(app);
        drop(webview);
        drop(window);
        result
    }
}
