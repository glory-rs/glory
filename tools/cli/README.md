# Project-local Glory tools

`glory-cli` is a library. Each application owns a tiny private executable and
pins the library in its Cargo manifest and lockfile. No global `glory` binary
is installed. This follows the useful part of [Prisma Client Rust's installation
model](https://prisma.brendonovich.dev/getting-started/installation): a dedicated
workspace package runs independently of application source compilation.

This tool targets the current semantic architecture. It has no dependency on
Glory's runtime crates and is an independent host-only tooling workspace.
The root workspace still contains the same twelve runtime/adapter packages.

## Final design

| Concern | Prisma's useful approach | Glory's implementation |
|---|---|---|
| Version ownership | CLI library pinned by the project | One `glory-cli` library; exact release or full Git revision, shared application Cargo.lock |
| Startup during application errors | A separate private workspace executable | `tools/glory` has no application dependency; `doctor` still works with broken application source |
| Convenient invocation | Project Cargo alias | `cargo glory` plus generated `bin/glory.ps1` and `bin/glory` |
| Host and cache isolation | Cargo controls tool compilation | Bin launchers select the native Rust target and D: caches before compiling the private runner |
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
- `serve`: loopback Web server, filesystem watch, debounced rebuild and browser
  refresh. A failed rebuild keeps the previous output and reports the error.
- `check`: Cargo check plus strict Clippy for the selected application/host.
- `fmt [--check]`: the application's ordinary rustfmt.
- `bundle`: release Web directory or server executable/assets plus a hash
  manifest. Desktop installers and mobile hosts are not implemented.
- `init`: create a private tool package, project Cargo alias and bin launchers.

Use `--help` for options. Rust hot-patching, RSX, a second UI lifecycle, automatic
global updates, remote deployment and application state preservation on reload
are outside this implementation.

## First use in this repository

The package is not yet published. With Cargo home, target and temporary paths
set under `D:/BuildCache`, the bootstrap example can configure an application:

```powershell
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
`glory-cli = "=0.3.1"`; during local development it uses a path to this package.
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
used. Other native Unix cache layouts have not been implemented or accepted.

The matching wasm-bindgen CLI is installed into a version/host-specific project
directory on the first Web build. `--offline` prohibits installation; `--bindgen`
accepts an explicit executable only after an exact version check. There is no
implicit PATH lookup for wasm-bindgen. Binaryen optimization is not yet provided.

The dev server listens on loopback and serves generated output only. Its external
refresh script polls a same-origin development endpoint; release output contains
no development script. SSR applications retain responsibility for their server,
HTTP/CSP configuration and streaming bootstrap; this static Web server does not
replace Salvo or add an SSR rewrite protocol.

Bundles are published to `<build-root>/bundle/<package>/web` or `.../server`;
each contains `glory-build.json` with release settings and file SHA-256 hashes.
Failed Web builds do not replace the last accepted output. The development server
waits for each build synchronously, so requests may pause while Cargo is running.

## Verification

Set all Cargo/cache/temp paths on D: and verify `cargo metadata`'s target first.
Then run:

```powershell
cargo fmt --manifest-path tools/cli/Cargo.toml -- --check
cargo test --locked --manifest-path tools/cli/Cargo.toml
cargo clippy --locked --manifest-path tools/cli/Cargo.toml --all-targets -- -D warnings
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

The CLI's fifteen tests cover configuration isolation, version/source mismatch,
artifact selection, output rollback, initialization without app compilation,
real TCP request handling and cancellation of owned child processes. Web release
bundling and a native SSR server's run/release bundle were also exercised. WSL
launcher syntax checks do not establish a Linux build or desktop window acceptance.
