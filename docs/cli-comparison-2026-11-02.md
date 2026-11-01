# Glory CLI compared with Dioxus CLI

Comparison date: 2026-11-02. Glory baseline: `5bd5359` (0.6.1).
Dioxus baseline: sibling checkout `430cfb563f1b70602ef387f0c80f171d84527b41`
(0.8.0-alpha.3, commit dated 2026-10-29). This is a source comparison, not a
claim that the alpha's entire feature set has passed platform acceptance here.
The [official Dioxus tooling guide](https://dioxuslabs.com/learn/0.7/guides/tools/)
provides the released 0.7 context. Commands and flags below were checked against
the pinned source because documentation examples can lag or differ by version.

Glory already implements project-local CLI installation, `init`, `doctor`,
`print`, `build`, `run`, `serve`, `check`, `fmt` and `bundle`. It pins tooling
through the application's workspace/lock and keeps the CLI out of runtime
dependency graphs. Dioxus uses a standalone `dx` executable. Preserving Glory's
installation model is a deliberate choice, not a missing global installer.

## Gaps and decisions

“Implemented” means delivered by this change. “Later” identifies a useful gap
that remains open, with its implementation boundary stated explicitly.

| Capability | Dioxus at the pinned revision | Glory before this change | Decision and resulting state |
|---|---|---|---|
| Package/platform/target selection | Packages, binaries, examples, platform/renderer aliases and target triples | Packages; Web/desktop/server; native triples | **Implemented:** `--bin` with required-feature selection. Examples, renderer aliases and mobile targets remain later work. |
| Feature overrides | Features, disabled defaults, all features | Manifest-only host features | **Implemented:** repeatable `--features`/`-F` and `--no-default-features`, shared by metadata/build/check/watch. Keep all-features out for now: host feature sets can be mutually exclusive. |
| Cargo profile/options | Release, custom profiles, Cargo/rustc arguments | Release build; check ignored release | **Implemented:** release checks. Custom profiles and arbitrary Cargo/rustc passthrough are later work; they need artifact/profile handling and must preserve cache/lock constraints. |
| Native application arguments | Arguments through serve/run | No passthrough | **Implemented:** `run/serve -- ...`, direct OS arguments, preserved through restart and fallback. Launcher offline scanning stops at `--`. |
| External build integration | Print client/server args in several shell/JSON formats | JSON summary only | **Implemented:** JSON build program/argv/cwd/environment overrides, produced by the same command builder used for execution. Shell rendering and separate client/server plans remain later work. |
| Completion scripts | clap-generated shell completion | None | **Implemented:** Bash, Zsh, Fish and PowerShell scripts for private launchers; no project loading. Dynamic package/feature names and Elvish remain unsupported. |
| Host portability | Native Windows/macOS/Linux tooling | Windows, macOS and verified D:-backed WSL caches | **Later:** ordinary native Linux cache/storage support. Cross-target planning does not mean the current launcher works on a Linux machine without the verified Windows data mount. |
| WASM release optimization | Managed pinned Binaryen with configurable levels | wasm-bindgen and gzip sidecars | **Implemented:** explicit executable plus exact version, `-Oz`, staged before compression/hash publication, failure rollback and version metadata. Automatic downloads/configurable levels remain later work. |
| New applications | `new` and template-driven `init` | `init` adds tools to an existing Cargo app | **Later:** `new` should emit tested Web/native templates and pin framework/CLI sources together. Current development versions and sibling engine paths are unpublished; do not generate a starter that claims to resolve from the registry today. |
| View-language tooling | RSX formatting/checks and HTML-to-RSX translation | Ordinary rustfmt/check/Clippy; Glory XML macros exist | **Later:** a formatter for Glory's own DSL. Do not port an RSX formatter/translator to a different syntax. Rustfmt alone does not establish DSL-formatting parity. |
| Hot development | RSX hot reload, optional Rust hotpatch, interactive UI/devtools | Full Web reload/native restart, error overlay, old process survives failed builds | **Later:** finer asset refresh and nonblocking builds. **Do not add now:** Rust binary hotpatch/state-preservation promises; these require a separate runtime/protocol design. |
| Web dev server | HTTPS, proxies and integrated fullstack serving | Loopback static server with authority checks | **Later:** explicit SPA fallback, HTTPS and proxy configuration, browser opening, base-path support and asynchronous builds. Preserve same-origin diagnostics and loopback defaults when extending it. |
| Fullstack/SSG | Client/server builds, server-function integration, prerendering | A selected server executable; SSR/Salvo integration is application-owned | **Later:** an explicit two-entry orchestration contract. No second SSR lifecycle or Dioxus server-function protocol should be introduced to claim parity. |
| Asset pipeline | Asset collection, hashing/optimization, Tailwind, optional WASM splitting | Explicit copies, gzip and integrity manifest | **Later:** fingerprinted URLs, CSS/JS/image optimization and opt-in external CSS tools. Brotli/code splitting require additional delivery/runtime contracts; an integrity hash is not URL fingerprinting. |
| Mobile workflows | Android/iOS build/package/device/simulator orchestration | Runtime examples and app-owned mobile scripts, no mobile CLI platform | **Later:** wrap and generalize maintained mobile build hosts, then verify installation/lifecycle per target. Merely accepting `--platform android` would not close this gap. |
| Distribution | Platform installers/packages, signing options | Native directories, ad-hoc signed macOS app; separate signing/notarization script | **Later:** installer metadata/icons and explicit signing/packaging integration. Credentials and actual device/store acceptance remain separate. |
| Diagnostics and machine logs | System-tool diagnostics, verbose/trace and JSON logs | Selected-project doctor, JSON plan, textual command logs | **Later:** structured events/errors and more SDK/linker diagnostics. Existing strict version/source/target checks remain in place; `print` is not advertised as a JSON logging mode. |
| Configuration management | Dioxus.toml tools, CLI settings, schema generation | Strict `package.metadata.glory`, inference | **Later:** schema/documentation generation if configuration expands. Keep a single manifest-based configuration source; no global settings store is needed for parity. |
| Global self-update/component registry | `self-update`, `components` | None | **Do not add now:** global updates conflict with project-owned versions; Glory has no supported registry contract and must not depend on downstream component repositories. Use explicit Cargo dependency updates and ordinary component crates. |

## Implementation boundaries

The selected changes close ordinary build/run/release gaps without a new runtime
dependency, compiler, tool workspace or lockfile. Feature lists add to configured
or inferred host features and deduplicate them; disabling defaults does not erase
explicit host features. Selecting a binary also selects its declared required
features. A typo or unresolved feature is rejected by Cargo metadata before a
build. `check --bin` checks that entry; unqualified checks retain all-targets.

The JSON command is a build invocation, even when printed for diagnostics; it
does not claim to describe every bindgen, optimizer, asset-copy or bundle step.
Environment overrides are explicit, while inherited process variables and Cargo
configuration still apply. Non-UTF-8 application arguments pass through on Unix;
JSON command export reports an error for paths it cannot represent as UTF-8.

The optimizer is opt-in and limited to release Web `build`/`bundle`; `print
--release` can describe the selection and `doctor` can verify the executable.
Version checks happen before compilation. Optimization touches only bindgen's
staged WASM, before application assets, gzip sidecars and final hashes. Failures
do not publish a partial site or silently fall back to unoptimized output.

## Verification

Executed on macOS ARM64 with Rust 1.98.1; the declared MSRV stays 1.95. All new
artifacts/cache/temp files live under `/Volumes/Data/Caches/glory`.

- Independent CLI locked/offline tests, rustfmt and strict all-targets Clippy.
- A real Cargo fixture proves binary selection over a conflicting configured
  entry, required/additional features, disabled defaults, release-independent
  direct command replay, native argument boundaries and lockfile preservation.
  A minimal framework stub keeps these Cargo tests independent of runtime crates.
  Its deliberately invalid other binary also proves `check --bin` selection.
- Actual counter plan selects `counter-wry` and `desktop-wry` without editing
  the manifest. This plan check does not claim Wry window acceptance.
- Actual counter release Web build uses wasm-bindgen 0.2.130 and Binaryen 133.
  Chrome executes the optimized module: increment, edit-then-increment and clear
  pass with zero page errors. Every output hash and gzip round trip is checked.
- Injected optimizer exit failure preserves all accepted output bytes and removes
  staging. A wrong requested optimizer version is rejected. Unit coverage also
  rejects successful tools that produce missing/truncated/invalid module output.
- Real native watch fixture proves application arguments survive rebuilding,
  failed compilation preserves the old process, recovery changes PID, and Ctrl+C
  reaps it. Bash completion behavior, Bash/Zsh syntax and actual PowerShell
  platform/shell completion pass. Fish output is generated but Fish is not
  installed on this host, so its runtime behavior is unverified.

Evidence: `/Volumes/Data/Caches/glory/semantic-ui/cli-parity-web/` (build log,
browser/failure JSON and screenshot), `cli-parity-watch/`, `cli-parity-shells/`
and `cli-parity-wry-plan.json` in the same cache directory. This is not a claim
of Windows/Linux/mobile acceptance or a Rust 1.95 toolchain run.

## Primary source map

All Dioxus source links below use the compared commit, rather than mutable main.

- [Commands](https://github.com/DioxusLabs/dioxus/blob/430cfb563f1b70602ef387f0c80f171d84527b41/packages/cli/src/cli/mod.rs), [target/build options](https://github.com/DioxusLabs/dioxus/blob/430cfb563f1b70602ef387f0c80f171d84527b41/packages/cli/src/cli/target.rs), [serve options](https://github.com/DioxusLabs/dioxus/blob/430cfb563f1b70602ef387f0c80f171d84527b41/packages/cli/src/cli/serve.rs).
- [Build/fullstack/SSG](https://github.com/DioxusLabs/dioxus/blob/430cfb563f1b70602ef387f0c80f171d84527b41/packages/cli/src/cli/build.rs), [Binaryen](https://github.com/DioxusLabs/dioxus/blob/430cfb563f1b70602ef387f0c80f171d84527b41/packages/cli/src/wasm_opt.rs), [bundling](https://github.com/DioxusLabs/dioxus/blob/430cfb563f1b70602ef387f0c80f171d84527b41/packages/cli/src/cli/bundle.rs).
- [Web configuration](https://github.com/DioxusLabs/dioxus/blob/430cfb563f1b70602ef387f0c80f171d84527b41/packages/cli/src/config/web.rs), [print integration](https://github.com/DioxusLabs/dioxus/blob/430cfb563f1b70602ef387f0c80f171d84527b41/packages/cli/src/cli/print.rs), [logging](https://github.com/DioxusLabs/dioxus/blob/430cfb563f1b70602ef387f0c80f171d84527b41/packages/cli/src/cli/verbosity.rs).
- Glory: [tooling README](../tools/cli/README.md), [arguments](../tools/cli/src/args.rs), [selection and command planning](../tools/cli/src/project.rs), [build publication](../tools/cli/src/build.rs), [optimizer](../tools/cli/src/wasm_opt.rs), [completions](../tools/cli/src/completions.rs).
