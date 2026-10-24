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

## Windows

Run from the repository root. `build.ps1` places Cargo's registry/Git cache,
target, temporary files and Web output under `D:/BuildCache/glory-semantic-ui`,
checks Cargo metadata's actual target directory, and restores the calling
environment afterwards. `-BuildRoot` accepts another directory under
`D:/BuildCache`. Use Ctrl+C to stop the Web server or close the desktop window.

Desktop:

```powershell
./examples/counter/build.ps1 -Platform desktop -Run
```

Web prerequisites are Rust's `wasm32-unknown-unknown` target, Python and
`wasm-bindgen-cli 0.2.129` (matching the locked WASM library). If the CLI is not
already available, install it with its caches, temporary files and binary on D:

```powershell
$env:CARGO_HOME = 'D:/BuildCache/glory-semantic-ui/cargo'
$env:CARGO_TARGET_DIR = 'D:/BuildCache/glory-semantic-ui/target'
$env:TEMP = 'D:/BuildCache/glory-semantic-ui/tmp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force $env:CARGO_HOME, $env:TEMP | Out-Null
cargo metadata --manifest-path examples/counter/Cargo.toml --no-deps --format-version 1
cargo install wasm-bindgen-cli --version 0.2.129 --locked --root D:/BuildCache/glory-semantic-ui/tools
```

Build and serve at <http://127.0.0.1:8790>:

```powershell
./examples/counter/build.ps1 -Platform web -Run -Bindgen D:/BuildCache/glory-semantic-ui/tools/bin/wasm-bindgen.exe
```

Omit `-Run` to build only. Serve the generated `counter-web` directory over HTTP;
opening `index.html` directly from the filesystem does not load WASM modules.
The old Trunk configuration, `web-csr` / `web-ssr` features and Salvo asset server
have been removed. This example uses client rendering; SSR/hydration have their
own host setup in `../semantic-web`.

## Linux / WSL

In WSL first verify `/mnt/d` is mounted from D:, then set all build paths there:

```sh
findmnt -T /mnt/d/BuildCache
export CARGO_HOME=/mnt/d/BuildCache/glory-semantic-ui/cargo
export CARGO_TARGET_DIR=/mnt/d/BuildCache/glory-semantic-ui/linux-counter-target
export TMPDIR=/mnt/d/BuildCache/glory-semantic-ui/tmp
mkdir -p "$CARGO_HOME" "$TMPDIR"
cargo metadata --manifest-path examples/counter/Cargo.toml --no-deps --format-version 1
cargo run --locked --manifest-path examples/counter/Cargo.toml --features desktop --bin counter-desktop
```

Check that metadata reports the configured target directory. Linux needs a
display server, system fonts and Fontconfig. For Web, build the library with
`--features web --lib --target wasm32-unknown-unknown`, run the matching
`wasm-bindgen --target web` on `counter.wasm`, and copy `index.html` alongside
the generated `counter.js` / `counter_bg.wasm` before serving that directory.

## Checks

With the D: environment above:

```powershell
cargo test --locked --manifest-path examples/counter/Cargo.toml
cargo clippy --locked --manifest-path examples/counter/Cargo.toml --features desktop --all-targets -- -D warnings
cargo clippy --locked --manifest-path examples/counter/Cargo.toml --features web --lib --target wasm32-unknown-unknown -- -D warnings
```

The Rust tests drive semantic events through the shared component, including
controlled input revisions and independent mount/cleanup behavior. The existing
Playwright counter project checks real browser buttons, typing, normalization
and reload. With the Web server running and Playwright installed:

```powershell
$env:GLORY_COUNTER_URL = 'http://127.0.0.1:8790'
npm --prefix tests/playwright run test:counter -- --output D:/BuildCache/glory-semantic-ui/counter-playwright
```
