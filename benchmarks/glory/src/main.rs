//! Canonical rows workload. Business model and handwritten/XML frontends are separate.
#[cfg(target_arch = "wasm32")]
#[path = "browser.rs"]
mod browser;
#[cfg(target_arch = "wasm32")]
#[path = "../../data.rs"]
mod data;
#[cfg(target_arch = "wasm32")]
mod model;
#[cfg(all(target_arch = "wasm32", feature = "acceptance"))]
mod probes;
fn main() {
    #[cfg(target_arch = "wasm32")]
    browser::main();
}
