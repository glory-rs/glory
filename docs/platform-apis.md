# Platform API Examples

Glory keeps platform APIs outside the widget surface. Widgets stay portable;
desktop/mobile/native hosts provide platform capabilities through config,
menus, command-backend queries, custom events, or app-owned services.

## Window Configuration

Desktop window size, title, resizability, devtools, command coalescing, assets,
and menus are configured per window:

```rust
let config = glory_desktop::DesktopConfig {
    title: "Inspector".into(),
    inner_size: (1024.0, 720.0),
    resizable: true,
    devtools: cfg!(debug_assertions),
    coalesce: true,
    ..Default::default()
};

glory_desktop::launch_with_config(config, || App);
```

For multiple windows:

```rust
glory_desktop::Desktop::new()
    .window(glory_desktop::DesktopConfig { title: "Main".into(), ..Default::default() }, || MainApp)
    .window(glory_desktop::DesktopConfig { title: "Tools".into(), ..Default::default() }, || ToolsApp)
    .run();
```

## Menus As Commands

Menus are the current stable way to route host-native actions back into the
reactive tree:

```rust
let reset = count;
let config = glory_desktop::DesktopConfig {
    menu: Some(glory_desktop::MenuSpec::new().submenu(
        "File",
        vec![glory_desktop::MenuItemSpec::new("reset", "Reset")],
    )),
    on_menu: Some(std::rc::Rc::new(move |_holder, id| {
        if id == "reset" {
            reset.revise(|mut value| *value = 0);
        }
    })),
    ..Default::default()
};
```

`on_menu` runs on the host event-loop thread. Signal writes settle and flush
back to the window automatically.

## File Dialogs

Glory does not hard-code a file dialog dependency. App crates should open file
dialogs from host callbacks such as `on_menu`, then write the selected path into
a `Cage` or send it through an app service. A typical desktop app can use a
dialog crate from the host callback:

```rust
let selected_file = glory::Cage::new(None::<String>);
let target = selected_file;

let config = glory_desktop::DesktopConfig {
    menu: Some(glory_desktop::MenuSpec::new().submenu(
        "File",
        vec![glory_desktop::MenuItemSpec::new("open", "Open...")],
    )),
    on_menu: Some(std::rc::Rc::new(move |_holder, id| {
        if id == "open" {
            // Example integration point:
            // if let Some(path) = rfd::FileDialog::new().pick_file() {
            //     target.revise(|mut value| *value = Some(path.display().to_string()));
            // }
        }
    })),
    ..Default::default()
};
```

Keeping the dialog crate in the app avoids forcing desktop-only dependencies on
mobile/native/server builds.

## DOM Queries Across Hosts

Code that needs platform state should use asynchronous command-backend queries
instead of synchronous DOM reads:

```rust
let node = glory::NodeRef::new();
input().node_ref(node.clone()).show_in(ctx);

// Later, in command-backed hosts:
// let value = node.value().await?;
// let rect = node.bounding_rect().await?;
// let scroll = node.scroll_offset().await?;
```

These query kinds map to `renderer::NodeQuery` and are answered by the host.
Unsupported hosts return `QueryError::Unsupported`.

## Mobile Viewport And Lifecycle

Generated mobile apps install safe-area and keyboard CSS variables:

- `--glory-safe-top`
- `--glory-safe-right`
- `--glory-safe-bottom`
- `--glory-safe-left`
- `--glory-viewport-height`
- `--glory-keyboard-inset-bottom`

The mobile bootstrap also emits `glory:viewport`, `glory:foreground`, and
`glory:background` browser custom events. App code can listen through normal
JavaScript integration or bridge them into `EventData` in a custom host.

## Event Payload Families

Command-backed hosts can populate optional `EventData` families:

- `pointer`
- `keyboard`
- `target`
- `clipboard`
- `selection`
- `scroll`
- `resize`
- `media`
- `extra`

Hosts should fill only fields they can provide accurately. Widgets should treat
missing fields as unsupported platform data, not as zero values unless using the
convenience helpers such as `target_value()`.
