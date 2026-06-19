# Glory Desktop Guide

Glory desktop apps run the same widget tree on the command-stream backend and
apply the resulting commands inside a wry webview.

## Crate Setup

Use the desktop runtime feature from an app crate outside the Glory workspace:

```toml
[dependencies]
glory = { path = "../../crates/glory", default-features = false, features = ["backend-command"] }
glory-desktop = { path = "../../crates/desktop", features = ["runtime"] }
```

Do not enable `web-csr` for desktop. Desktop uses `backend-command`, which
requires the multi-holder scheduler.

## Launch

```rust
fn main() {
    glory_desktop::launch(|| App);
}
```

For custom window options:

```rust
fn main() {
    glory_desktop::launch_with_config(
        glory_desktop::DesktopConfig {
            title: "Counter".into(),
            inner_size: (900.0, 640.0),
            resizable: true,
            devtools: cfg!(debug_assertions),
            ..Default::default()
        },
        || App,
    );
}
```

If the root widget needs to control the native window, use
`launch_with_handle`:

```rust
fn main() {
    glory_desktop::launch_with_handle(Default::default(), |window| App { window });
}
```

## Multi-Window

Each window owns an independent `CommandHolder`, command queue, webview, and
reactive root. State does not cross windows unless you explicitly share it
through your own backend.

```rust
fn main() {
    glory_desktop::Desktop::new()
        .window(glory_desktop::DesktopConfig { title: "Main".into(), ..Default::default() }, || MainApp)
        .window(glory_desktop::DesktopConfig { title: "Tools".into(), ..Default::default() }, || ToolsApp)
        .run();
}
```

Use `window_with_handle` when a window's widget tree needs a handle:

```rust
glory_desktop::Desktop::new()
    .window_with_handle(Default::default(), |window| MainApp { window })
    .run();
```

## Window Controls

`DesktopWindowHandle` is cloneable and can be captured by widget event
callbacks. It queues commands onto the tao/wry event loop and exposes a cached
state snapshot for common queries:

```rust
let toggle_fullscreen = {
    let window = self.window.clone();
    move |_| {
        window.set_fullscreen(!window.is_fullscreen());
    }
};

let open_tools = {
    let window = self.window.clone();
    move |_| {
        window.open_window(
            glory_desktop::DesktopConfig {
                title: "Tools".into(),
                ..Default::default()
            },
            |tools| ToolsApp { window: tools },
        );
    }
};
```

Available controls include `drag_window`, `set_fullscreen`,
`set_maximized`, `toggle_maximized`, `focus`, `set_zoom_level`, `close`,
`close_window(id)`, and `open_window`. `DesktopWindowId` is process-local and
stable for the lifetime of the window.

## Menus

Menus are declared per window. On Windows they attach to the window; on macOS
they become the application menu.

```rust
use std::rc::Rc;

use glory_desktop::{DesktopConfig, MenuItemSpec, MenuSpec};

let config = DesktopConfig {
    menu: Some(MenuSpec::new().submenu(
        "File",
        vec![MenuItemSpec::new("reset", "Reset")],
    )),
    on_menu: Some(Rc::new(|holder, id| {
        if id == "reset" {
            holder.update(|| {
                // revise signals here
            });
        }
    })),
    ..Default::default()
};
```

Menu callbacks run on the event-loop thread. Signal writes settle and flush back
to the webview automatically.

For native file dialogs, keep the dialog crate in the app and open it from a
host callback such as `on_menu`; see [Platform APIs](platform-apis.md#file-dialogs)
for the `rfd` integration pattern.

## Tray And Hotkeys

`DesktopConfig::tray` keeps a system tray icon alive for the window lifetime.
Tray icon events and global hotkeys are delivered on the event-loop thread, so
signal writes can be batched and flushed like menu callbacks:

```rust
use std::rc::Rc;

use glory_desktop::{DesktopConfig, DesktopHotKeySpec, TrayIconSpec};

let config = DesktopConfig {
    tray: Some(
        TrayIconSpec::new("main-tray")
            .tooltip("Glory")
            .icon_rgba(vec![255, 255, 255, 255], 1, 1),
    ),
    on_tray: Some(Rc::new(|holder, event| {
        holder.update(|| {
            // react to DesktopTrayEvent
        });
    })),
    hotkeys: vec![DesktopHotKeySpec::new("toggle", "cmdorctrl+KeyK")],
    on_hotkey: Some(Rc::new(|holder, event| {
        if event.id == "toggle" {
            holder.update(|| {
                // toggle app state
            });
        }
    })),
    ..Default::default()
};
```

Hotkey accelerators use the `global-hotkey` parser (`cmdorctrl+KeyK`,
`shift+alt+KeyQ`, and the `keyboard-types` `Code::*` names).

## Assets

Declare assets once:

```rust
let logo = glory::asset!("assets/logo.png");
let src = glory_desktop::asset_url(logo.public_path());

let icons = glory::asset_folder!("assets/icons");
for icon in icons.assets() {
    let src = glory_desktop::asset_url(icon.public_path());
}
```

The desktop runtime serves `glory://` URLs from:

1. `DesktopConfig::assets_root`, when set.
2. `GLORY_SITE_ROOT`, when running through `glory serve`.
3. The executable directory, which is what `glory bundle --target desktop`
   prepares by copying site files beside the executable.

Traversal outside the assets root is rejected.

`glory bundle` writes `glory-bundle.json` with an `asset_map` from declared
public paths to content-hashed copies. When that manifest exists under the
assets root, the desktop runtime installs it before mounting the widget tree,
so `logo.public_path()` resolves to the hashed URL while the original file
remains available for compatibility.

## Custom Protocols

The built-in `glory://` asset protocol is asynchronous internally. Register
additional long-running resource or RPC protocols with `DesktopProtocol`; the
handler owns Wry's `RequestAsyncResponder`, so it can reply from a worker
thread or runtime task:

```rust
use glory_desktop::{desktop_protocol_response, DesktopConfig, DesktopProtocol};

let config = DesktopConfig::default().with_custom_protocol(DesktopProtocol::new(
    "app",
    |_webview_id, request, responder| {
        std::thread::spawn(move || {
            let body = format!("path={}", request.uri().path()).into_bytes();
            responder.respond(desktop_protocol_response(200, "text/plain", body));
        });
    },
));
```

`glory` is reserved for the built-in static asset protocol. On Windows, custom
protocol URLs are exposed through Wry's localhost rewrite in the same way as
`asset_url`.

## Hot Reload

Run the app through:

```sh
glory serve --target desktop
```

When `GLORY_WATCH=ON`, the desktop runtime connects to the CLI reload websocket.
Style changes are link-swapped in the webview. Function reload batches invoke
`DesktopConfig::on_function_reload` on the event-loop thread:

```rust
let config = glory_desktop::DesktopConfig {
    on_function_reload: Some(std::rc::Rc::new(|holder, batch| {
        holder.update(|| {
            // update function registry / revise signals from batch
        });
    })),
    ..Default::default()
};
```

## Server Functions From Desktop

Desktop clients can call server functions through the non-wasm HTTP client:

```toml
glory-serverfn = { path = "../../crates/serverfn", features = ["reqwest-client"] }
```

Set the server base URL before invoking generated server-function stubs:

```rust
glory_serverfn::set_server_url("http://127.0.0.1:8000");
```

## Bundle

```sh
glory bundle --release --target desktop
```

The bundle lands in `dist/<project>/` and contains:

- the desktop executable;
- mirrored site/assets files beside the executable;
- platform installer artifacts under `installers/`;
- `glory-bundle.json` with target and artifact metadata.

The executable-directory asset fallback means `asset_url("/logo.png")` works
from the bundle root without additional configuration.

On Windows, `glory bundle --target desktop` stages a WiX installer project under
`installers/windows/`. If `heat.exe`, `candle.exe`, and `light.exe` are on PATH
or `WIX_BIN` points at the WiX bin directory, the CLI also emits an `.msi`;
otherwise it leaves `product.wxs`, `staging/`, and `build-msi.ps1` for a later
installer build.

On Linux, the same command emits a Debian package under `installers/linux/`.
The `.deb` installs the bundle under `/usr/lib/<package>` and adds a launcher
symlink plus a freedesktop `.desktop` file.
