//! Headless-browser smoke tests for the CSR runtime.
//!
//! These tests run via `wasm-bindgen-test` against a headless browser
//! (Chrome/Firefox). They are intentionally minimal — the full
//! per-widget regression matrix already lives in the SSR snapshot
//! tests inside `crates/core/src/widgets/snapshot_tests.rs`, which
//! runs on the host toolchain. The wasm tests here exist to verify
//! that the CSR path *compiles and links*, that the entry-point types
//! (`BrowerHolder`, `glory::launch`-equivalents) are reachable from a
//! wasm32 target, and to provide a stub for future end-to-end work.
//!
//! Run with:
//!
//! ```sh
//! cargo install wasm-bindgen-cli
//! # Chrome:
//! cargo test -p glory-core --target wasm32-unknown-unknown \
//!     --no-default-features --features web-csr \
//!     --test wasm_csr_smoke
//! ```
//!
//! The `--features web-csr` is required; the test is otherwise gated
//! out so that host `cargo test` doesn't try to compile it.

#![cfg(all(target_arch = "wasm32", feature = "web-csr"))]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn smoke_runtime_loads() {
    // If we got here the wasm bundle linked successfully and the
    // entry-point types from glory_core are reachable. Detailed widget
    // semantics are covered by the SSR snapshot suite on the host
    // toolchain.
    let _ = glory_core::reflow::Cage::new(0_i32);
}
