# Cross-platform counter

One semantic `Counter` component runs in a browser or a Blitz desktop window.
Both hosts mount exactly the same component, including its state, event handlers
and typed styles. No DOM event conversion or host-specific CSS is used by it.

- `src/lib.rs`: shared `Widget`, Owner-owned text `Cage` with a derived integer, semantic
  `activate` / `value_changed` handlers and portable styles.
- `src/web.rs`: WASM startup, DOM mount lookup and the retained `WebApp` guard.
- `src/main.rs`: desktop `WindowApp`, window attributes and event loop.

Clear resets to zero; -1/+1 decrement/increment; the text field accepts signed
32-bit integers. In-progress text (including a lone minus) stays editable;
empty, invalid or out-of-range text displays a numeric value of zero. Committing
the input with Enter or pressing a button normalizes the text.
Arithmetic saturates at the integer limits. State belongs to each mounted component, so
independent mounts do not share a counter.

An owned Effect uses the semantic `Scope::set_input_value` helper after each host
tick. It acknowledges edits even when typing and Enter/Clear in one tick restore
the previous value; canonical echoes preserve the host's caret/composition. The
Effect holds a weak Scope and a live node reference, with no host-specific code.

This is a standalone example workspace. Its default features select no host.
`web` enables the DOM host; `desktop` enables Blitz's desktop shell and AccessKit.
The desktop entry targets Windows, Linux and macOS; Android still needs a mobile
host. Platform device/OS input acceptance follows Glory's adapter support, not
the fact that the shared component compiles.

## Project-local commands

This example owns a small `counter-tools` workspace package, a shared Cargo.lock,
a `cargo glory` alias, and `bin/` launchers. It depends on the new library-only
`glory-cli` through a repository-relative path. No global Glory CLI is installed.
The tool package does not depend on the application's library, so it can start
and diagnose the project even when application Rust code fails to compile.

Windows (from the repository root):

```powershell
./examples/counter/bin/glory.ps1 doctor
./examples/counter/bin/glory.ps1 serve --platform web
./examples/counter/bin/glory.ps1 run --platform desktop
./examples/counter/bin/glory.ps1 build --platform web --release
./examples/counter/bin/glory.ps1 bundle --platform web
```

The Web server uses loopback port 8080 by default (`--port` overrides it).
`serve` watches sources/assets and refreshes the page after a successful build.
A failed rebuild preserves the previous output and displays compiler errors.
A refresh starts a new app; reactive state is not preserved across rebuilds.
`run --platform web` starts the server without watching.

No extra project configuration is needed here: the CLI infers the cdylib Web
entry, the `web` feature, and the unique desktop binary's required features.
See [the tool package README](../../tools/cli/README.md) for manifest overrides,
other applications, workspace selection and reproducible tool installation.

The launchers set caches, native tool output and temporary files under
`D:/BuildCache/glory-projects/<project-path-hash>` before Cargo starts. Application
builds use a separate `app-target` directory. `GLORY_BUILD_ROOT` can select another
project directory under D:/BuildCache. A matching wasm-bindgen is installed in
that project's versioned tools directory on first build. No global bindgen is
selected implicitly; `--bindgen` accepts an explicitly supplied, checked binary.

For WSL, first verify the D: mount, then use the shell launcher:

```sh
sh ./examples/counter/bin/glory doctor
sh ./examples/counter/bin/glory serve --platform web
```

With an already configured D: Cargo environment, `cargo glory ...` also works
inside the example. It is a project alias for the private tool package. Prefer
the bin launcher when Cargo's default target is configured for cross-compilation.

`build.ps1` remains a small forwarding convenience for existing CI commands;
`-BuildRoot` selects the CLI build root and `-Bindgen` passes an explicit binary.
Its Web output is now `<BuildRoot>/web-debug`, or `web-release` in release mode.
Desktop installers, mobile hosts and Rust hot-patching are not implemented.

## Checks

Set Cargo home, target and temporary directories on D: and verify metadata first.
Then the shared component can still be tested directly:

```powershell
cargo test --locked --manifest-path examples/counter/Cargo.toml
./examples/counter/bin/glory.ps1 check --platform web
./examples/counter/bin/glory.ps1 check --platform desktop
./examples/counter/bin/glory.ps1 fmt --check
```

The dedicated Playwright counter project verifies actual buttons, controlled
input, Enter commits and reset. The CLI package also has a disposable-project
browser acceptance driver for rebuild/error recovery. OS IME, screen-reader and
other device acceptance follow the adapters' documented boundaries.
