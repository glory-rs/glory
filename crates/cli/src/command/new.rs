use std::{fs, path::Path};

use crate::ext::anyhow::{Context, Result, bail, ensure};
use clap::{Args, ValueEnum};
use tokio::process::Command;

use crate::ext::exe::Exe;

// A subset of the cargo-generate commands available.
// See: https://github.com/cargo-generate/cargo-generate/blob/main/src/args.rs

#[derive(Clone, Debug, Args, PartialEq, Eq)]
#[clap(about)]
pub struct NewCommand {
    /// Built-in template to materialize when --git/--path is not used.
    #[clap(long, value_enum, default_value_t = TemplateKind::Web)]
    pub template: TemplateKind,

    /// Git repository to clone template from. Can be a URL (like
    /// `https://github.com/rust-cli/cli-template`), a path (relative or absolute), or an
    /// `owner/repo` abbreviated GitHub URL (like `rust-cli/cli-template`).
    #[clap(short, long, group("SpecificPath"))]
    pub git: Option<String>,

    /// Branch to use when installing from git
    #[clap(short, long, conflicts_with = "tag")]
    pub branch: Option<String>,

    /// Tag to use when installing from git
    #[clap(short, long, conflicts_with = "branch")]
    pub tag: Option<String>,

    /// Local path to copy the template from. Can not be specified together with --git.
    #[clap(short, long, group("SpecificPath"))]
    pub path: Option<String>,

    /// Directory to create / project name; if the name isn't in kebab-case, it will be converted
    /// to kebab-case unless `--force` is given.
    #[clap(long, short, value_parser)]
    pub name: Option<String>,

    /// Don't convert the project name to kebab-case before creating the directory.
    /// Note that cargo generate won't overwrite an existing directory, even if `--force` is given.
    #[clap(long, short, action)]
    pub force: bool,

    /// Enables more verbose output.
    #[clap(long, short, action)]
    pub verbose: bool,

    /// Generate the template directly into the current dir. No subfolder will be created and no vcs is initialized.
    #[clap(long, action)]
    pub init: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum TemplateKind {
    Web,
    Ssr,
    Fullstack,
    Desktop,
    Mobile,
}

impl NewCommand {
    pub async fn run(&self) -> Result<()> {
        if self.git.is_none() && self.path.is_none() {
            return self.write_builtin_template();
        }

        let args = self.to_args();
        let exe = Exe::CargoGenerate.get().await.dot()?;

        let mut process = Command::new(exe)
            .arg("generate")
            .args(&args)
            .spawn()
            .context("Could not spawn cargo-generate command (verify that it is installed)")?;
        process.wait().await.dot()?;
        Ok(())
    }

    pub fn to_args(&self) -> Vec<String> {
        let mut args = vec![];
        opt_push(&mut args, "git", &absolute_git_url(&self.git));
        opt_push(&mut args, "branch", &self.branch);
        opt_push(&mut args, "tag", &self.tag);
        opt_push(&mut args, "path", &self.path);
        opt_push(&mut args, "name", &self.name);
        bool_push(&mut args, "force", self.force);
        bool_push(&mut args, "verbose", self.verbose);
        bool_push(&mut args, "init", self.init);
        args
    }

    fn write_builtin_template(&self) -> Result<()> {
        self.write_builtin_template_in(Path::new("."))
    }

    fn write_builtin_template_in(&self, base_dir: &Path) -> Result<()> {
        let raw_name = self
            .name
            .as_deref()
            .or_else(|| self.init.then_some("glory-app"))
            .context("built-in templates require --name unless --init is used")?;
        let package_name = if self.force { raw_name.to_owned() } else { kebab_case(raw_name) };
        ensure!(
            is_valid_package_name(&package_name),
            "template name `{package_name}` is not a valid Cargo package name"
        );
        let crate_name = package_name.replace('-', "_");
        let root = if self.init {
            base_dir.to_path_buf()
        } else {
            base_dir.join(&package_name)
        };
        if root.exists() && !self.init && !self.force {
            bail!(
                "refusing to overwrite existing directory `{}`; pass --force to overwrite template files",
                root.display()
            );
        }

        fs::create_dir_all(root.join("src")).context(format!("create template directory `{}`", root.display()))?;
        for file in self.template.files(&package_name, &crate_name) {
            let path = root.join(file.path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).context(format!("create template directory `{}`", parent.display()))?;
            }
            if path.exists() && !self.force {
                bail!(
                    "refusing to overwrite existing file `{}`; pass --force to overwrite template files",
                    path.display()
                );
            }
            fs::write(&path, file.body).context(format!("write template file `{}`", path.display()))?;
        }
        Ok(())
    }
}

fn bool_push(args: &mut Vec<String>, name: &str, set: bool) {
    if set {
        args.push(format!("--{name}"))
    }
}

fn opt_push(args: &mut Vec<String>, name: &str, arg: &Option<String>) {
    if let Some(arg) = arg {
        args.push(format!("--{name}"));
        args.push(arg.clone());
    }
}

/// Workaround to support short `new --git glory-rs/start` command when behind Git proxy.
/// See https://github.com/cargo-generate/cargo-generate/issues/752.
fn absolute_git_url(url: &Option<String>) -> Option<String> {
    match url {
        Some(url) => match url.as_str() {
            "glory-rs/start" => Some("https://github.com/glory-rs/start".to_string()),
            "glory-rs/start-salvo" => Some("https://github.com/glory-rs/start-salvo".to_string()),
            _ => Some(url.to_string()),
        },
        None => None,
    }
}

struct TemplateFile {
    path: &'static str,
    body: String,
}

impl TemplateKind {
    fn files(self, package_name: &str, crate_name: &str) -> Vec<TemplateFile> {
        match self {
            TemplateKind::Web => web_template(package_name, crate_name),
            TemplateKind::Ssr => ssr_template(package_name, crate_name, false),
            TemplateKind::Fullstack => ssr_template(package_name, crate_name, true),
            TemplateKind::Desktop => desktop_template(package_name),
            TemplateKind::Mobile => mobile_template(package_name, crate_name),
        }
    }
}

fn kebab_case(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = true;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && !last_was_sep {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('-');
            last_was_sep = true;
        }
    }
    out.trim_matches('-').to_owned()
}

fn pascal_case(value: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if uppercase_next {
                out.push(ch.to_ascii_uppercase());
                uppercase_next = false;
            } else {
                out.push(ch);
            }
        } else {
            uppercase_next = true;
        }
    }
    if out.is_empty() { "GloryApp".to_owned() } else { out }
}

fn is_valid_package_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_')
        && value.bytes().next().is_some_and(|byte| byte.is_ascii_lowercase())
}

fn web_template(package_name: &str, crate_name: &str) -> Vec<TemplateFile> {
    vec![
        TemplateFile {
            path: "Cargo.toml",
            body: format!(
                r#"[package]
name = "{package_name}"
version = "0.1.0"
edition = "2024"

[dependencies]
glory = {{ version = "0.3.1", default-features = false, features = ["web-csr"] }}
wasm-bindgen = {{ version = "0.2", features = ["enable-interning"] }}

[package.metadata.glory]
output_name = "{crate_name}"
site_root = "target/site"
site_pkg_dir = "pkg"
lib_features = ["web-csr"]
lib_default_features = false
"#
            ),
        },
        TemplateFile {
            path: "index.html",
            body: r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Glory</title>
  </head>
  <body></body>
</html>
"#
            .to_owned(),
        },
        TemplateFile {
            path: "src/main.rs",
            body: r#"use glory::reflow::Cage;
use glory::web::events;
use glory::web::holders::BrowserHolder;
use glory::web::widgets::*;
use glory::{Scope, Widget};

#[derive(Debug)]
struct App {
    count: Cage<i64>,
}

impl App {
    fn new() -> Self {
        Self { count: Cage::new(0) }
    }
}

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        let count = self.count;
        let increment = move |_| count.revise(|mut value| *value += 1);

        div()
            .attr("style", "font-family: sans-serif; max-width: 36rem; margin: 4rem auto;")
            .fill(h1().text("Glory"))
            .fill(p().text("Edit src/main.rs and run `glory serve`."))
            .fill(button().on(events::click, increment).text(self.count))
            .show_in(ctx);
    }
}

fn main() {
    BrowserHolder::new().mount(App::new());
}
"#
            .to_owned(),
        },
    ]
}

fn ssr_template(package_name: &str, crate_name: &str, fullstack: bool) -> Vec<TemplateFile> {
    let server_features = if fullstack {
        r#""glory/salvo", "glory/server-fn", "glory-serverfn/salvo", "dep:tokio", "dep:salvo", "dep:tracing-subscriber""#
    } else {
        r#""glory/salvo", "dep:tokio", "dep:salvo", "dep:tracing-subscriber""#
    };
    let csr_features = if fullstack {
        r#""glory/web-csr", "glory/server-fn", "dep:wasm-bindgen""#
    } else {
        r#""glory/web-csr""#
    };
    let glory_features = if fullstack { r#""routing", "server-fn""# } else { r#""routing""# };
    let serverfn_dep = if fullstack {
        r#"glory-serverfn = { version = "0.3.1" }
wasm-bindgen = { version = "0.2", optional = true }
"#
    } else {
        ""
    };
    let mut files = vec![
        TemplateFile {
            path: "Cargo.toml",
            body: format!(
                r#"[package]
name = "{package_name}"
version = "0.1.0"
edition = "2024"

[features]
web-csr = [{csr_features}]
web-ssr = [{server_features}]

[dependencies]
glory = {{ version = "0.3.1", default-features = false, features = [{glory_features}] }}
{serverfn_dep}serde = {{ version = "1", features = ["derive"] }}
salvo = {{ version = "0.93", default-features = true, optional = true, features = ["serve-static"] }}
tokio = {{ version = "1", optional = true, features = ["macros", "rt-multi-thread"] }}
tracing-subscriber = {{ version = "0.3", optional = true }}

[package.metadata.glory]
output_name = "{crate_name}"
site_root = "target/site"
site_pkg_dir = "pkg"
site_addr = "127.0.0.1:8000"
bin_features = ["web-ssr"]
bin_default_features = false
lib_features = ["web-csr"]
lib_default_features = false
"#
            ),
        },
        TemplateFile {
            path: "src/main.rs",
            body: if fullstack { FULLSTACK_MAIN.to_owned() } else { SSR_MAIN.to_owned() },
        },
        TemplateFile {
            path: "src/app.rs",
            body: if fullstack { FULLSTACK_APP.to_owned() } else { SSR_APP.to_owned() },
        },
    ];
    if fullstack {
        files.push(TemplateFile {
            path: "src/api.rs",
            body: FULLSTACK_API.to_owned(),
        });
    }
    files
}

fn desktop_template(package_name: &str) -> Vec<TemplateFile> {
    vec![
        TemplateFile {
            path: "Cargo.toml",
            body: format!(
                r#"[package]
name = "{package_name}"
version = "0.1.0"
edition = "2024"

[dependencies]
glory-core = {{ version = "0.3.1", default-features = false, features = ["backend-command"] }}
glory-desktop = {{ version = "0.3.1", default-features = false, features = ["runtime"] }}
"#
            ),
        },
        TemplateFile {
            path: "src/main.rs",
            body: DESKTOP_MAIN.to_owned(),
        },
    ]
}

fn mobile_template(package_name: &str, crate_name: &str) -> Vec<TemplateFile> {
    let android_package = format!("com.example.{crate_name}");
    let ios_name = pascal_case(package_name);
    vec![
        TemplateFile {
            path: "Cargo.toml",
            body: format!(
                r#"[package]
name = "{package_name}"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "staticlib", "rlib"]

[features]
default = []
mobile = []

[dependencies]
glory-core = {{ version = "0.3.1", default-features = false, features = ["backend-command"] }}
glory-desktop = {{ version = "0.3.1", default-features = false, features = ["backend"] }}
serde_json = "1"
tracing = "0.1"

[target.'cfg(any(target_os = "android", target_os = "ios"))'.dependencies]
wry = "0.55"
tao = "0.35"
"#
            ),
        },
        TemplateFile {
            path: "src/lib.rs",
            body: MOBILE_LIB.replace("glory_app", crate_name),
        },
        TemplateFile {
            path: "android/settings.gradle.kts",
            body: include_str!("../../templates/mobile/android/settings.gradle.kts").replace("glory-app", package_name),
        },
        TemplateFile {
            path: "android/app/build.gradle.kts",
            body: include_str!("../../templates/mobile/android/app/build.gradle.kts")
                .replace("com.example.mobile_counter", &android_package)
                .replace("mobile_counter", crate_name),
        },
        TemplateFile {
            path: "android/app/src/main/AndroidManifest.xml",
            body: include_str!("../../templates/mobile/android/app/src/main/AndroidManifest.xml").to_owned(),
        },
        TemplateFile {
            path: "android/app/src/main/kotlin/com/example/glory_app/MainActivity.kt",
            body: include_str!("../../templates/mobile/android/app/src/main/kotlin/com/example/mobile_counter/MainActivity.kt")
                .replace("com.example.mobile_counter", &android_package),
        },
        TemplateFile {
            path: "ios/project.yml",
            body: include_str!("../../templates/mobile/ios/project.yml")
                .replace("GloryApp", &ios_name)
                .replace("mobile_counter", crate_name)
                .replace("com.example.mobile-counter", &format!("com.example.{package_name}")),
        },
        TemplateFile {
            path: "ios/main.swift",
            body: include_str!("../../templates/mobile/ios/main.swift").to_owned(),
        },
    ]
}

const SSR_MAIN: &str = r#"mod app;

#[cfg(feature = "web-ssr")]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    use app::App;
    use glory::Holder;
    use glory::web::holders::{SalvoHandler, ServerHolder};
    use salvo::prelude::*;

    let handler = SalvoHandler::new(|config, url| ServerHolder::new(config, url).mount(App::new())).await;
    let site_addr = handler.config.site_addr.clone();
    let router = Router::new().push(Router::with_path("<**path>").get(StaticDir::new(["target/site"])));
    let service = salvo::Service::new(router).catcher(salvo::catcher::Catcher::default().hoop(handler));
    let acceptor = TcpListener::new(site_addr).bind().await;
    Server::new(acceptor).serve(service).await;
}

#[cfg(feature = "web-csr")]
fn main() {
    use app::App;
    use glory::Holder;
    use glory::web::holders::BrowserHolder;
    BrowserHolder::new().mount(App::new());
}

#[cfg(all(not(feature = "web-ssr"), not(feature = "web-csr")))]
fn main() {}
"#;

const SSR_APP: &str = r#"use glory::web::widgets::*;
use glory::{Scope, Widget};

#[derive(Debug)]
pub struct App;

impl App {
    pub fn new() -> Self {
        Self
    }
}

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        div()
            .attr("style", "font-family: sans-serif; max-width: 42rem; margin: 4rem auto;")
            .fill(h1().text("Glory SSR"))
            .fill(p().text("This page is rendered on the server and hydrated in the browser."))
            .show_in(ctx);
    }
}
"#;

const FULLSTACK_MAIN: &str = r#"mod api;
mod app;

#[cfg(feature = "web-ssr")]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    use app::App;
    use glory::Holder;
    use glory::web::holders::{SalvoHandler, ServerHolder};
    use salvo::prelude::*;

    let handler = SalvoHandler::new(|config, url| ServerHolder::new(config, url).mount(App::new())).await;
    let site_addr = handler.config.site_addr.clone();
    let router = Router::new()
        .push(glory::serverfn::salvo_mount::router())
        .push(Router::with_path("<**path>").get(StaticDir::new(["target/site"])));
    let service = salvo::Service::new(router).catcher(salvo::catcher::Catcher::default().hoop(handler));
    let acceptor = TcpListener::new(site_addr).bind().await;
    Server::new(acceptor).serve(service).await;
}

#[cfg(feature = "web-csr")]
fn main() {
    use app::App;
    use glory::Holder;
    use glory::web::holders::BrowserHolder;
    BrowserHolder::new().mount(App::new());
}

#[cfg(all(not(feature = "web-ssr"), not(feature = "web-csr")))]
fn main() {}
"#;

const FULLSTACK_APP: &str = r#"use glory::reflow::Cage;
use glory::spawn::spawn_local;
use glory::web::events;
use glory::web::widgets::*;
use glory::{Scope, Widget};

use crate::api::add_todo;

#[derive(Debug)]
pub struct App {
    title: Cage<String>,
    status: Cage<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            title: Cage::new(String::new()),
            status: Cage::new("idle".to_owned()),
        }
    }
}

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        let title = self.title;
        let update_title = move |ev| title.revise(|mut value| *value = event_value(&ev));
        let title = self.title;
        let status = self.status;
        let submit = move |_| {
            let title = title.get_untracked().clone();
            status.revise(|mut value| *value = "saving".to_owned());
            spawn_local(async move {
                let next = match add_todo(title).await {
                    Ok(todo) => format!("saved: {todo}"),
                    Err(err) => format!("error: {err}"),
                };
                status.revise(|mut value| *value = next);
            });
        };

        div()
            .attr("style", "font-family: sans-serif; max-width: 42rem; margin: 4rem auto;")
            .fill(h1().text("Glory Fullstack"))
            .fill(input().attr("placeholder", "Todo title").on(events::input, update_title))
            .fill(button().on(events::click, submit).text("Save on server"))
            .fill(p().text(self.status))
            .show_in(ctx);
    }
}

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
fn event_value<T>(event: &T) -> String
where
    T: wasm_bindgen::JsCast,
{
    glory::web::event_target_value(event)
}

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
fn event_value<T>(_event: &T) -> String {
    String::new()
}
"#;

const FULLSTACK_API: &str = r#"use glory::serverfn::ServerFnError;

#[glory::server]
pub async fn add_todo(title: String) -> Result<String, ServerFnError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(ServerFnError::field_error("title", "required"));
    }
    Ok(title.to_owned())
}
"#;

const DESKTOP_MAIN: &str = r#"use glory_core::reflow::Cage;
use glory_core::web::events;
use glory_core::web::widgets::*;
use glory_core::{Scope, Widget};
use glory_desktop::DesktopConfig;

#[derive(Debug)]
struct App {
    count: Cage<i64>,
}

impl App {
    fn new() -> Self {
        Self { count: Cage::new(0) }
    }
}

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        let count = self.count;
        let increment = move |_| count.revise(|mut value| *value += 1);

        div()
            .attr("style", "font-family: sans-serif; padding: 2rem;")
            .fill(h1().text("Glory Desktop"))
            .fill(button().on(events::click, increment).text(self.count))
            .show_in(ctx);
    }
}

fn main() {
    let config = DesktopConfig {
        title: "Glory Desktop".to_owned(),
        inner_size: (480.0, 260.0),
        ..Default::default()
    };
    glory_desktop::launch_with_config(config, App::new);
}
"#;

const MOBILE_LIB: &str = r#"use glory_core::reflow::Cage;
use glory_core::web::events;
use glory_core::web::widgets::*;
use glory_core::{Scope, Widget};

#[derive(Debug)]
pub struct App {
    count: Cage<i64>,
}

impl App {
    pub fn new() -> Self {
        Self { count: Cage::new(0) }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for App {
    fn build(&mut self, ctx: &mut Scope) {
        let count = self.count;
        let increment = move |_| count.revise(|mut value| *value += 1);

        div()
            .attr("style", "font-family: sans-serif; padding: 2rem; min-height: calc(var(--glory-viewport-height) - var(--glory-safe-top) - var(--glory-safe-bottom));")
            .fill(h1().text("Glory Mobile"))
            .fill(button().on(events::click, increment).text(self.count))
            .show_in(ctx);
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
mod host {
    use glory_core::renderer::{EventData, QueryResponse};
    use glory_core::web::holders::CommandHolder;
    use glory_core::{Holder, Widget};
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::window::WindowBuilder;
    use wry::WebViewBuilder;

    const BOOTSTRAP_HTML: &str = concat!(
        "<!doctype html><html><head><meta charset=\"utf-8\">",
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1, viewport-fit=cover\">",
        "<style>",
        ":root{--glory-safe-top:env(safe-area-inset-top,0px);--glory-safe-right:env(safe-area-inset-right,0px);",
        "--glory-safe-bottom:env(safe-area-inset-bottom,0px);--glory-safe-left:env(safe-area-inset-left,0px);",
        "--glory-viewport-height:100vh;--glory-keyboard-inset-bottom:0px}",
        "html,body{min-height:100%;margin:0;overscroll-behavior:none;background:Canvas}",
        "body{min-height:var(--glory-viewport-height);padding:var(--glory-safe-top) var(--glory-safe-right) ",
        "calc(var(--glory-safe-bottom) + var(--glory-keyboard-inset-bottom)) var(--glory-safe-left);box-sizing:border-box}",
        "</style></head><body><script>",
        "(()=>{const root=document.documentElement;",
        "const dispatch=(name,detail={})=>window.dispatchEvent(new CustomEvent(name,{detail}));",
        "const syncViewport=()=>{const vv=window.visualViewport;const height=vv?vv.height:window.innerHeight;",
        "const keyboard=Math.max(0,window.innerHeight-height-(vv?vv.offsetTop:0));",
        "root.style.setProperty('--glory-viewport-height',`${height}px`);",
        "root.style.setProperty('--glory-keyboard-inset-bottom',`${keyboard}px`);",
        "dispatch('glory:viewport',{height,keyboardInsetBottom:keyboard});};",
        "window.addEventListener('resize',syncViewport,{passive:true});",
        "if(window.visualViewport){window.visualViewport.addEventListener('resize',syncViewport,{passive:true});",
        "window.visualViewport.addEventListener('scroll',syncViewport,{passive:true});}",
        "document.addEventListener('visibilitychange',()=>dispatch(document.hidden?'glory:background':'glory:foreground'));",
        "window.addEventListener('focus',()=>dispatch('glory:foreground'),{passive:true});",
        "window.addEventListener('blur',()=>dispatch('glory:background'),{passive:true});syncViewport();})();",
        "</script></body></html>"
    );

    enum HostEvent {
        Ready,
        Dom(EventData),
        Query(QueryResponse),
    }

    pub fn run<W: Widget + 'static>(widget: impl FnOnce() -> W + 'static) {
        let event_loop = EventLoopBuilder::<HostEvent>::with_user_event().build();
        let window = WindowBuilder::new().build(&event_loop).expect("create mobile webview window");

        let ipc_proxy = event_loop.create_proxy();
        let webview = WebViewBuilder::new()
            .with_initialization_script(glory_desktop::WRY_INTERPRETER_JS)
            .with_html(BOOTSTRAP_HTML)
            .with_ipc_handler(move |request: wry::http::Request<String>| match serde_json::from_str::<glory_desktop::IpcMessage>(request.body()) {
                Ok(glory_desktop::IpcMessage::GloryWryReady(_)) => {
                    let _ = ipc_proxy.send_event(HostEvent::Ready);
                }
                Ok(glory_desktop::IpcMessage::GloryWryEvent(data)) => {
                    let _ = ipc_proxy.send_event(HostEvent::Dom(data));
                }
                Ok(glory_desktop::IpcMessage::GloryWryQuery(response)) => {
                    let _ = ipc_proxy.send_event(HostEvent::Query(response));
                }
                Err(err) => tracing::warn!(%err, "undecodable mobile IPC message"),
            })
            .build(&window)
            .expect("create mobile webview");

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

#[cfg(target_os = "android")]
mod android {
    fn _start_app() {
        super::host::run(super::App::new);
    }

    #[allow(dead_code)]
    fn bindings() {
        tao::android_binding!(com_example, glory_app, WryActivity, wry::android_setup, _start_app, ::tao);
        wry::android_binding!(com_example, glory_app, ::wry);
    }
}

#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn start_app() {
    host::run(App::new);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_template_defaults_to_web_and_writes_project() {
        let dir = temp_dir::TempDir::new().unwrap();
        let result = NewCommand {
            template: TemplateKind::Web,
            git: None,
            branch: None,
            tag: None,
            path: None,
            name: Some("My App".to_owned()),
            force: false,
            verbose: false,
            init: false,
        }
        .write_builtin_template_in(dir.path());

        result.unwrap();
        assert!(dir.path().join("my-app/Cargo.toml").exists());
        assert!(dir.path().join("my-app/src/main.rs").exists());
    }

    #[test]
    fn git_templates_still_forward_to_cargo_generate_args() {
        let args = NewCommand {
            template: TemplateKind::Web,
            git: Some("glory-rs/start".to_owned()),
            branch: Some("main".to_owned()),
            tag: None,
            path: None,
            name: Some("demo".to_owned()),
            force: true,
            verbose: true,
            init: false,
        }
        .to_args();

        assert_eq!(
            args,
            vec![
                "--git",
                "https://github.com/glory-rs/start",
                "--branch",
                "main",
                "--name",
                "demo",
                "--force",
                "--verbose"
            ]
        );
    }

    #[test]
    fn fullstack_template_contains_server_mount() {
        let files = TemplateKind::Fullstack.files("demo", "demo");
        let manifest = files.iter().find(|file| file.path == "Cargo.toml").unwrap();
        let main = files.iter().find(|file| file.path == "src/main.rs").unwrap();
        assert!(manifest.body.contains("glory-serverfn"));
        assert!(main.body.contains("glory::serverfn::salvo_mount::router()"));
        assert!(files.iter().any(|file| file.path == "src/api.rs"));
    }

    #[test]
    fn mobile_template_contains_android_and_ios_hosts() {
        let files = TemplateKind::Mobile.files("my-mobile", "my_mobile");
        let lib = files.iter().find(|file| file.path == "src/lib.rs").unwrap();
        let android = files.iter().find(|file| file.path == "android/app/build.gradle.kts").unwrap();
        let manifest = files.iter().find(|file| file.path == "android/app/src/main/AndroidManifest.xml").unwrap();
        let ios = files.iter().find(|file| file.path == "ios/project.yml").unwrap();

        assert!(lib.body.contains("my_mobile"));
        assert!(lib.body.contains("viewport-fit=cover"));
        assert!(lib.body.contains("glory-keyboard-inset-bottom"));
        assert!(lib.body.contains("glory:background"));
        assert!(android.body.contains("com.example.my_mobile"));
        assert!(android.body.contains("--target-dir"));
        assert!(manifest.body.contains("adjustResize"));
        assert!(ios.body.contains("-lmy_mobile"));
        assert!(ios.body.contains("../target/mobile/aarch64-apple-ios/release"));
    }
}
