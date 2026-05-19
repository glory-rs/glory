//! Glory — an experimental Rust web framework.
//!
//! This crate is the public front door. Components, signals, and the
//! widget lifecycle live in [`glory_core`] (re-exported); the optional
//! `glory_routing` is re-exported as `glory::routing` when the
//! `routing` feature is enabled.
//!
//! See the crate-level rustdoc on [`glory_core`] for the architecture
//! diagram and the type-by-type rundown.

#[macro_use]
mod cfg;

#[doc(no_inline)]
pub use glory_core::*;

cfg_feature! {
    #![feature ="routing"]
    #[doc(no_inline)]
    pub use glory_routing as routing;
}

/// Mount a root widget into the browser body and return the running
/// holder.
///
/// This is the convenience entry point that replaces the explicit
/// `BrowerHolder::new().mount(widget)` boilerplate. Only available
/// when building for `wasm32` with the `web-csr` feature.
///
/// ```ignore
/// fn main() {
///     glory::launch(MyApp::new());
/// }
/// ```
///
/// Use [`launch_with_host`] when you need to mount under a specific
/// element instead of `<body>`.
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub fn launch<W>(widget: W) -> glory_core::web::holders::BrowerHolder
where
    W: glory_core::Widget,
{
    use glory_core::Holder;
    glory_core::web::holders::BrowerHolder::new().mount(widget)
}

/// Mount a root widget under a specific host element.
///
/// Useful when the page is already partially rendered and the Glory
/// app should live inside a particular `<div id="...">` rather than
/// take over the whole `<body>`.
///
/// `host` is anything that `AsRef`s into the CSR `Node` (which is
/// re-exported from `glory_core` as the WASM/`web-sys` element type).
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub fn launch_with_host<W>(host: impl AsRef<glory_core::Node>, widget: W) -> glory_core::web::holders::BrowerHolder
where
    W: glory_core::Widget,
{
    use glory_core::Holder;
    glory_core::web::holders::BrowerHolder::with_host_node(host).mount(widget)
}

/// Server-side launch is intentionally not a single function: the
/// `ServerHolder` needs the request URL and renders into a per-request
/// `Truck`, so it's constructed inside the HTTP handler rather than
/// at app startup. With the `salvo` feature, use
/// [`glory_core::web::holders::SalvoHandler`] (a ready-made
/// `salvo::Handler`) and pass a factory closure that returns a freshly
/// configured `ServerHolder` per request. Manual integrations follow
/// the same pattern — see the `ssr-simple-salvo` and
/// `hackernews-salvo` examples.
#[cfg(all(not(target_arch = "wasm32"), feature = "web-ssr"))]
pub mod ssr {
    pub use glory_core::web::holders::ServerHolder;
}
