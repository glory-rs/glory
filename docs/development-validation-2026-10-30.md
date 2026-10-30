# 2026-10-30 development update validation

Versions: Apace 0.3.0, Takia 0.15.1, Wingy 0.31.1, Glory/Sapid 0.6.1.
These are unpublished development changes. No release tags or registry
uploads were created. Exact versions use sibling source paths; see
[development versions](development-versions.md) for dependency compatibility notes.

All 79 library/example/tool manifests declare or inherit Rust 1.95. Every
workspace, including independent Takia benchmarks/examples, uses resolver 3.
Library/example/benchmark Cargo.lock files are removed from the Git index but
retained locally. Website keeps its tracked lock and now references 0.6.1.

Glory and Sapid each have one root LICENSE and one root NOTICE, with
Apache-2.0 metadata and [licensing guidance](licensing.md). Crate-local copies
and Glory's MIT files were removed as requested. User Cargo metadata edits and
Sapid's pre-existing quality review document were preserved.

| Check | Result |
| --- | --- |
| Takia, Rust 1.95, all features | 7,010 passed, 4 ignored |
| Takia no_std, workspace tools, comparison benches | Passed |
| Wingy Windows tests/docs, Rust 1.95 | 81 passed, 8 ignored |
| Wingy cross-checks, Rust 1.95 | PowerPC X11, AppKit, UIKit, Web and Redox passed |
| Apace workspace, Rust 1.95 | 687 passed, 3 ignored |
| Apace parallel/writing-mode tests and strict Clippy | Passed |
| Glory workspace/all features, Rust 1.95 | 1,189 passed, 2 ignored |
| Glory CLI, Rust 1.95, latest toml_edit and SHA-2 | 26 passed; strict Clippy passed |
| Glory check-semantic-ui.ps1 | Passed, including source identity, fork/model tests, CLI acceptance, package/raster and strict Clippy |
| Sapid check.ps1 -Web -Native | Passed, including native examples, strict Clippy, WASM builds and stream/SSR fixtures |
| Sapid Python tooling | 55 passed |
| Website native tests and native/WASM strict Clippy | Passed |
| Website WASM binding, generated assets, browser and routing acceptance | Passed |
| Takia/Wingy/Apace dependency licenses | Passed cargo-deny |
| Fresh library + independent example lock generation | Passed; subsequent run preserved existing bytes |
| Workflow YAML and git diff --check | Passed |

Strict Clippy uses the existing CI Rust 1.98 toolchain; Rust 1.95 validates the
minimum compiler separately. Cross-compilation and model tests do not constitute
macOS/iOS/Android device, OS IME or screen-reader interaction acceptance.
Raw logs and registry/dependency catalogs are under
`D:/BuildCache/glory-release-20261030`. Test counts are per configuration and
must not be added together as distinct cases.
