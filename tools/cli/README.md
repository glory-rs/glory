# Project-local Glory tools

`glory-cli` is a library. Each application owns a tiny private executable and
pins the library in its Cargo manifest and lockfile. No global `glory` binary
is installed. This follows the useful part of [Prisma Client Rust's installation
model](https://prisma.brendonovich.dev/getting-started/installation): a dedicated
workspace package runs independently of application source compilation.

This tool targets the current semantic architecture. It has no dependency on
Glory's runtime crates and is an independent host-only tooling workspace.
The active runtime/adapter and compiler package set is defined by the root
workspace's Cargo metadata; this independent tooling workspace is excluded.

## Final design

| Concern | Prisma's useful approach | Glory's implementation |
|---|---|---|
| Version ownership | CLI library pinned by the project | One `glory-cli` library; exact release or full Git revision, shared application Cargo.lock |
| Startup during application errors | A separate private workspace executable | `tools/glory` has no application dependency; `doctor` still works with broken application source |
| Convenient invocation | Project Cargo alias | `cargo glory` plus generated `bin/glory.ps1` and `bin/glory` |
| Host and cache isolation | Cargo controls tool compilation | Bin launchers select the native Rust target and approved Windows/WSL/macOS data-volume caches before compiling the private runner |
| Configuration | Application-owned setup | Infer ordinary entries/features/assets; put only overrides in `package.metadata.glory` |
| Auxiliary tools | Project command owns its tools | Resolve wasm-bindgen from Cargo.lock and install that exact version under the project build root |

Compared with a separate tool workspace/lock per application, sharing the
application workspace lock reduces duplicated pins and setup. The official CLI
source itself remains an independent workspace so runtime checks do not build
tooling dependencies. A private runner is not a new published Glory crate.

Dioxus-style build, run, serve/watch, release output and diagnostics are useful
here. Glory keeps handwritten Rust builders and the existing host lifecycle;
this implementation does not add an RSX compiler or a binary hot-patching system.

## Commands

- `doctor`: verify the selected application, framework version/source, Rust host
  and actual Cargo target. It reports the required wasm-bindgen version.
- `print`: JSON build plan, selected package/host/features and resolved paths.
- `build`: Web WASM + matching wasm-bindgen + HTML/assets, or a native binary.
- `run`: run a desktop/server binary, or serve Web without watching.
- `serve`: loopback Web server with refresh, or desktop/server watch with process
  restart. Rebuilds are debounced; failed native builds keep the old process.
  Each native generation runs a separate executable copy so Windows can link
  the next build. Ctrl+C stops/reaps the owned process. Restarts reset app state.
  Web requests must use `localhost` or the listening loopback IP and the actual
  listener port in `Host`; foreign or duplicate authorities are rejected before
  files or compiler diagnostics are read. HTTP/1.1 requires Host, while HTTP/1.0
  may omit it. Only origin-form request targets are accepted.
- `check`: Cargo check plus strict Clippy for the selected application/host.
- `fmt [--check]`: the application's ordinary rustfmt.
- `bundle`: release Web directory or desktop/server executable/assets plus a
  target-specific hash manifest. Native directory bundles are portable file
  layouts. macOS desktop output is an ad-hoc-signed `.app` with assets in
  `Contents/Resources` and hashes of the final signature. Other installers and
  mobile packages require separate tooling.
- `init`: create a private tool package, project Cargo alias and bin launchers.

Use `--help` for options. Rust hot-patching, RSX, a second UI lifecycle, automatic
global updates, remote deployment and application state preservation on reload
are outside this implementation.

Release Web builds stream compressible HTML/JS/WASM/CSS/JSON/SVG/text into gzip
sidecars when smaller than the original (files below 256 bytes are skipped).
Headers contain no source filename or timestamp, so equal inputs produce equal
compressed bytes with the pinned compressor. Sidecars enter `glory-build.json`;
an application asset with the same `.gz` name is a build error, never overwritten.
Original URLs and bytes remain available. The local server negotiates gzip and
identity using `Accept-Encoding`, emits `Vary` and the encoded length for HEAD,
and returns 406 when neither is acceptable. Watch/reload always uses identity
bytes so the development script can be inserted. A production server must
configure equivalent content negotiation; simply copying sidecars does not
configure a CDN. Brotli, asset fingerprinting, image conversion and WASM code
splitting remain separate work.

## First use in this repository

The package is not yet published. With Cargo home, target and temporary paths
set under `D:/BuildCache`, the bootstrap example can configure an application:

```powershell
python scripts/library_lockfiles.py
cargo run --locked --manifest-path tools/cli/Cargo.toml --example bootstrap -- init --manifest-path examples/counter/Cargo.toml --cli-path tools/cli
```

The repository's counter already has that runner. To configure another existing
application without setting Cargo paths by hand, invoke its bin launcher with
absolute paths (replace the application path):

```powershell
./examples/counter/bin/glory.ps1 init --manifest-path D:/Works/my-app/Cargo.toml --cli-path D:/Works/glory-rs/glory/tools/cli
```

Then run the new application's own `bin/glory.ps1 doctor` and commit its generated
files and Cargo.lock. The existing project must have a valid Cargo manifest;
its Rust source does not need to compile for initialization or diagnostics.

The configured counter is an example of the resulting project structure:

```text
Cargo.toml                   application + workspace members
Cargo.lock                   application and private tool versions
.cargo/config.toml           cargo glory alias
tools/glory/Cargo.toml       private, publish = false
tools/glory/src/main.rs      glory_cli::run_from_manifest(...)
bin/glory.ps1                Windows launcher
bin/glory                    WSL launcher
```

For a published release the private package uses an exact dependency such as
`glory-cli = "=0.6.0"`; during local development it uses a path to this package.
Git-based projects must pin the CLI and framework to the same complete commit.
Ordinary commands never update those pins. Commit the shared Cargo.lock.

The bin launchers set Cargo caches, the native tool target and temporary paths
before invoking Cargo. On first use they resolve the new tool dependency into
Cargo.lock; later invocations use `--locked`. Changes to an existing tool pin
require an explicit Cargo update. Application builds use a separate target
directory, so the enclosing `cargo run` cannot hold their build lock.

```powershell
./bin/glory.ps1 doctor
./bin/glory.ps1 print --platform web
./bin/glory.ps1 serve --platform web
./bin/glory.ps1 run --platform desktop
./bin/glory.ps1 build --platform web --release
./bin/glory.ps1 bundle --platform desktop
./bin/glory.ps1 print --platform desktop --target aarch64-apple-darwin
```

`cargo glory ...` is also available after configuring Cargo's D: environment;
it does not install or resolve a globally named CLI. The launcher scripts work
from any current directory. Tool compilation uses the Rust host, not the Web
target; application-only WASM workspace checks should select their application
package instead of compiling the private host tool for WASM.

## Minimal configuration

A package with a `cdylib` library defaults to Web. A `web` feature is selected
when present. A unique native binary and its required features are inferred.
`index.html` is used when present; otherwise a page mounting `#app` is generated.
The contents of `public/` are copied by default. Overrides live in the manifest:

```toml
[package.metadata.glory]
default-platform = "web"

[package.metadata.glory.desktop]
bin = "my-desktop-app"
features = ["desktop"]
# Optional built-in Rust triple. --target overrides this setting.
# target = "x86_64-pc-windows-msvc"

[package.metadata.glory.web]
port = 8080
features = ["web"]
assets = ["assets"]
```

Use `--package` to select an application under a virtual workspace. Do not add
the CLI as a dependency of an application's ordinary library target. The private
package only depends on `glory-cli`; application compile errors do not prevent
the tool from starting or printing its build plan.
`init` finds the containing workspace even when given a member manifest, preserves
comments and existing CLI pins, and keeps the host tool outside default application
members. A virtual workspace with one default application can infer its selection;
use `--package` when there are several.

## Isolation and scope

On Windows caches use `D:/BuildCache/glory-projects/<project-path-hash>`. WSL uses
a verified D:-backed `/mnt/d/BuildCache` mount. `GLORY_BUILD_ROOT` selects another
project root there; `--build-root` changes the application's output root.
An existing D: Cargo home may be shared as an immutable package download cache;
versions come from the project lock, not its contents. No C: cache fallback is
used. macOS uses `/Volumes/Data/Caches/glory/projects/<project-path-hash>`;
`GLORY_BUILD_ROOT` can select a project directory under that approved data-volume
cache root. Paths and existing symlink parents are checked before creating files.
Other native Unix cache layouts have not been implemented or accepted.

The matching wasm-bindgen CLI is installed into a version/host-specific project
directory on the first Web build. `--offline` prohibits installation; `--bindgen`
accepts an explicit executable only after an exact version check. There is no
implicit PATH lookup for wasm-bindgen. Binaryen optimization is not yet provided.

The dev server listens on loopback and serves generated output only. Its external
refresh script polls a same-origin development endpoint; release output contains
no development script. SSR applications retain responsibility for their server,
HTTP/CSP configuration and streaming bootstrap; this static Web server does not
replace Salvo or add an SSR rewrite protocol.

Bundles are published to `<build-root>/bundle/<package>/<platform>/<target>`;
each contains `glory-build.json` with the Rust host/target, release settings and
file SHA-256 hashes. The target suffix prevents different architectures from
replacing each other's output. Native assets preserve their configured relative
paths; resolve them against `current_exe()` for relocation. Asset names cannot
replace the executable or reserved `glory-build.json`.
Failed Web builds do not replace the last accepted output. The development server
waits for each build synchronously, so requests may pause while Cargo is running.

`--target` accepts built-in Windows/Linux/macOS targets for desktop/server and
only `wasm32-unknown-unknown` for Web. The configured platform still selects
application features and entry points; a triple does not install a mobile host.
`doctor` checks the selected target's standard library. Cross-compilation needs
an appropriate Cargo linker and SDK; `run` rejects a target different from the
Rust host before building or launching it. Native Unix tool launchers still
require the verified D: mount described above. Selecting an Apple target from
Windows does not establish macOS/iOS build or device acceptance.

## Verification

Set all Cargo/cache/temp paths on D: and verify `cargo metadata`'s target first.
Then run:

```powershell
python scripts/library_lockfiles.py
cargo fmt --manifest-path tools/cli/Cargo.toml -- --check
cargo test --locked --manifest-path tools/cli/Cargo.toml
cargo clippy --locked --manifest-path tools/cli/Cargo.toml --all-targets -- -D warnings
cargo build --locked --manifest-path tools/cli/Cargo.toml --example bootstrap
./tools/cli/tests/native-acceptance.ps1 -Driver D:/BuildCache/glory-semantic-ui/target/debug/examples/bootstrap.exe
```

Windows acceptance on 2026-10-26 used a disposable D: counter copy and real Chrome:
initial interaction, CSS rebuild/refresh, failed Rust rebuild preserving the
accepted files and working app, `doctor` with broken application source, and
recovery refresh all passed without browser errors. The driver is
`tests/web-acceptance.cjs`; never point it at the application's working sources.
Its `--fixture`, `--build`, `--log`, `--out`, `--driver` and `--playwright` paths
select the disposable project, running serve log, evidence output, private runner
and installed Playwright module. Set Cargo home and temporary paths on D: before
running it. Results are under `D:/BuildCache/glory-project-cli/browser-acceptance-final`.

The CLI tests cover configuration isolation, version/source mismatch,
artifact selection, output rollback, initialization without app compilation,
real TCP request handling and cancellation of owned child processes. Web release
bundling and a native SSR server's run/release bundle were also exercised. WSL
launcher syntax checks do not establish a Linux build or desktop window acceptance.

`tests/native-acceptance.ps1` creates a disposable application using the real
Glory facade, verifies target overrides and foreign/mobile/unknown-target
rejection, bundles a release executable with assets, checks every hash, relocates
and runs it from a path containing spaces, and checks lockfile and other-target
output preservation. It exercises native tooling and asset lookup; the console
fixture does not assert window/input or mobile device behavior.

## Native Apple hosts

The shell launcher works on macOS without GNU sha256sum/realpath/findmnt.
It isolates Cargo/tool/app/temp/Python caches under `/Volumes/Data/Caches/glory`;
Windows D: and verified WSL mount behavior remain supported. Canonical path
checks reject `..` and symlink escapes before creating output directories.

`package.metadata.glory.desktop.bundle-identifier` sets the macOS reverse-DNS
identifier; the default is `rs.glory.<package-name>`. `bundle --platform desktop`
creates `<package>.app/Contents/{MacOS,Resources,Info.plist}` on a macOS host,
verifies its ad-hoc signature, then hashes and atomically publishes the package.
Application assets are explicit; locate them relative to the bundle's Resources
folder rather than relying on Finder's working directory.

For Developer ID distribution, `scripts/sign-macos.sh APP IDENTITY NOTARY_PROFILE`
uses an explicitly supplied identity and an existing notarytool keychain profile.
It enables hardened runtime/timestamping, submits the ZIP, requires Accepted,
staples and verifies the ticket, runs Gatekeeper assessment and refreshes the
CLI integrity manifest. No credentials are discovered or embedded. The current
session verified ad-hoc installation; Developer ID/notarization requires the
application owner's credentials and remains unexecuted.

`tools/cli/tests/native-watch.py` exercises the real Cargo/notify/process flow:
a syntax error preserves the first live PID, a corrected source launches a new
PID, and Ctrl+C reaps it. It accepts a built bootstrap runner and keeps fixture
sources/logs/results in an explicit Apple cache directory.
