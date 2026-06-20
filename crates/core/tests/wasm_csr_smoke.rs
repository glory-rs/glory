//! Headless-browser smoke tests for the CSR runtime.
//!
//! These tests run via `wasm-bindgen-test` against a headless browser
//! (Chrome/Firefox). They are intentionally minimal — the full
//! per-widget regression matrix already lives in the SSR snapshot
//! tests inside `crates/core/src/widgets/snapshot_tests.rs`, which
//! runs on the host toolchain. The wasm tests here exist to verify
//! that the CSR path *compiles and links*, that the entry-point types
//! (`BrowserHolder`, `glory::launch`-equivalents) are reachable from a
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

use glory_core::reflow::Cage;
use glory_core::web::events;
use glory_core::web::holders::BrowserHolder;
use glory_core::web::widgets::{button, div, span};
use glory_core::{Holder, Scope, Widget};
use wasm_bindgen::{JsCast, UnwrapThrowExt};
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

#[derive(Debug)]
struct CompactWrapperSmoke {
    clicked: Cage<bool>,
}

impl Widget for CompactWrapperSmoke {
    fn build(&mut self, ctx: &mut Scope) {
        let clicked = self.clicked;
        div()
            .id("compact-root")
            .fill(
                span().id("compact-wrapper").fill(
                    button()
                        .id("compact-button")
                        .text("Hit")
                        .on(events::click, move |_| clicked.revise(|mut value| *value = true)),
                ),
            )
            .show_in(ctx);
    }
}

#[wasm_bindgen_test]
fn compact_static_wrapper_keeps_dynamic_child_parented() {
    let document = glory_core::web::document();
    let host = document.create_element("div").unwrap_throw();
    document.body().unwrap_throw().append_child(&host).unwrap_throw();

    let clicked = Cage::new(false);
    let _holder = BrowserHolder::with_host_node(&host).mount(CompactWrapperSmoke { clicked });

    let wrapper = host.query_selector("#compact-wrapper").unwrap_throw().unwrap_throw();
    let button = host
        .query_selector("#compact-button")
        .unwrap_throw()
        .unwrap_throw()
        .unchecked_into::<web_sys::HtmlElement>();

    assert_eq!(button.parent_element().unwrap_throw().id(), wrapper.id());
    button.click();
    assert!(*clicked.get_untracked());

    host.remove();
}
