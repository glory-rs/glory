//! SSR rendering must work from every Tokio runtime flavor, and from no
//! runtime at all.
//!
//! Run with:
//!
//! ```text
//! cargo test -p glory-core --features web-ssr --test ssr_runtime_flavors
//! ```

#![cfg(all(feature = "web-ssr", not(feature = "single-app")))]

use glory_core::config::GloryConfig;
use glory_core::reflow::resource_in;
use glory_core::web::holders::ServerHolder;
use glory_core::web::widgets::div;
use glory_core::{Holder, Scope, Widget};

#[derive(Debug)]
struct ResourceWidget;

impl Widget for ResourceWidget {
    fn build(&mut self, ctx: &mut Scope) {
        // An async resource forces the `spawn_local` path.
        let value = resource_in(ctx, || async { "loaded" });
        div().text(value).show_in(ctx);
    }
}

fn render() -> String {
    ServerHolder::new(GloryConfig::default(), "/").mount(ResourceWidget).app_html()
}

/// Regression: `spawn_local` reached for `block_in_place`, which panics with
/// "can call blocking only when running on the multi-threaded runtime".
/// `#[tokio::test]` is current-thread by default, so any downstream test that
/// rendered a page with a resource in it panicked instead of rendering.
#[tokio::test]
async fn renders_on_a_current_thread_runtime() {
    assert!(render().contains("loaded"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn renders_on_a_multi_thread_runtime() {
    assert!(render().contains("loaded"));
}

/// No runtime at all: `Handle::current()` would have panicked here too.
#[test]
fn renders_without_a_runtime() {
    assert!(render().contains("loaded"));
}
