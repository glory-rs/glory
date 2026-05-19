# glory-cli — the Glory build tool

`glory-cli` is installed as the `glory` binary. It bundles wasm
compilation (via `wasm-bindgen`), Sass / Tailwind / Lightning CSS
processing, static-asset mirroring, an HTTP serve loop, and a
filesystem watcher into a single subcommand-driven CLI.

## Install

```sh
cargo install --path crates/cli      # from this workspace
# or
cargo install glory-cli              # from crates.io once released
```

Both forms produce the `glory` binary on your `PATH`.

## Subcommands

```text
glory new <name>     scaffold a new Glory project from the built-in template
glory build          one-shot build (wasm + css + assets) into the site dir
glory serve          like `build`, then host the result over HTTP
glory watch          serve + filesystem notifier; rebuild on .rs/.css/.scss/.sass/assets changes
glory test           run cargo-test for the current project
glory end-to-end     run end-to-end tests (uses Playwright when configured)
```

Each subcommand accepts `--help` for its flags. Global flags live on
`glory` itself (`--manifest-path`, `--release`, `--verbose`, etc.).

## Typical workflow

```sh
glory new my-app
cd my-app
glory watch
# edit src/...; refresh the browser; rebuilds run on save
```

For deployment:

```sh
glory build --release
# the site/ directory is the deployable artefact
```

## Project layout the tool expects

```
my-app/
  Cargo.toml          # contains [package.metadata.glory] config (output_name, site_*)
  src/main.rs         # or src/lib.rs for SSR projects
  style/main.scss     # optional, processed via Sass + Lightning CSS
  public/             # assets, mirrored verbatim to site/
```

The `[package.metadata.glory]` keys correspond 1-to-1 with
[`GloryConfig`](../core/src/config.rs) (`output_name`, `site_root`,
`site_pkg_dir`, `site_addr`, `reload_port`, ...).

---

# Internals

## File view

This is mainly relevant for the `watch` mode.

```mermaid
graph TD;
  subgraph Watcher[watch]
    Watch[FS Notifier];
  end
  Watch-->|"*.rs & input.css"| TailW;
  Watch-->|"*.sass & *.scss"| Sass;
  Watch-->|"*.css"| Append;
  Watch-->|"*.rs"| WASM;
  Watch-->|"*.rs"| BIN;
  Watch-->|"assets/**"| Mirror;

  subgraph style
    TailW[Tailwind CSS];
    Sass;
    CSSProc[CSS Processor<br>Lightning CSS];
    Append{{append}};
  end

  TailW --> Append;
  Sass --> Append;
  Append --> CSSProc;

  subgraph rust
    WASM[Client WASM];
    BIN[Server BIN];
  end

  subgraph asset
    Mirror
  end

  subgraph update
    WOC[target/site/<br>Write-on-change FS];
    Live[Live Reload];
    Server;
  end

  Mirror -->|"site/**"| WOC;
  WASM -->|"site/pkg/app.wasm"| WOC;
  BIN -->|"server/app"| WOC;
  CSSProc -->|"site/pkg/app.css"| WOC;

  Live -.->|Port scan| Server;

  WOC -->|"target/server/app<br>site/**"| Server;
  WOC -->|"site/pkg/app.css, <br>client & server change"| Live;

  Live -->|"Reload all or<br>update app.css"| Browser

  Browser;
  Server -.- Browser;
```

## Concurrency view

Very approximate

```mermaid
stateDiagram-v2
    wasm: Build front
    bin: Build server
    style: Build style
    asset: Mirror assets
    serve: Run server

    state wait_for_start <<fork>>
      [*] --> wait_for_start
      wait_for_start --> wasm
      wait_for_start --> bin
      wait_for_start --> style
      wait_for_start --> asset

    reload: Reload
    state join_state <<join>>
      wasm --> join_state
      bin --> join_state
      style --> join_state
      asset --> join_state
    state if_state <<choice>>
        join_state --> if_state
        if_state --> reload: Ok
        if_state --> serve: Ok
        if_state --> [*] : Err
```
