//! Glory's reactive web framework core.
//!
//! # The six types every Glory app touches
//!
//! ```text
//!   ┌──────────┐   .map()    ┌──────────┐  .read()  ┌──────────┐
//!   │  Cage<T> │ ─────────►  │  Bond<T> │ ────────► │ Lotus<T> │
//!   └──────────┘             └──────────┘           └──────────┘
//!        ▲                                              │
//!        │ revise(...)                                  │ bind_view
//!        │                                              ▼
//!   user code                                       ┌──────────┐
//!                                                   │  Widget  │
//!                                                   │  build / │
//!                                                   │  patch   │
//!                                                   └──────────┘
//!                                                        │
//!                                                        ▼
//!                                                   ┌──────────┐
//!                                                   │  Scope   │ ──► child_views,
//!                                                   │          │     visible_views,
//!                                                   │          │     parent_node,
//!                                                   │          │     truck (ctx)
//!                                                   └──────────┘
//! ```
//!
//! - [`Cage<T>`][reflow::Cage] — mutable reactive cell. Reading it inside
//!   a tracking context (a `Bond` mapper or a `Widget` build/patch)
//!   subscribes the caller. Writing via [`Cage::revise`][reflow::Cage::revise]
//!   schedules re-renders for all subscribed views.
//! - [`Bond<T>`][reflow::Bond] — derived value. Re-runs its mapper when
//!   any of its tracked dependencies' `(id, version)` pair changes. Use
//!   [`Bond::with_eq`][reflow::Bond::with_eq] / `with_partial_eq()` to
//!   gate version bumps on actual output change.
//! - [`Lotus<T>`][reflow::Lotus] — read-only union of "any reactive
//!   value or bare T". Pass this when you accept "anything observable".
//! - [`Widget`] — a component. Implements `build` (initial layout),
//!   `patch` (re-render after signal change), `attach` / `flood` /
//!   `detach` for lifecycle hooks. Created in [`Widget::build`] by
//!   chaining HTML element factories like `div().class("..").show_in(ctx)`.
//! - [`Scope`] — the local context passed to every `build` / `patch`.
//!   Holds the component's `child_views`, current `visible_views`, the
//!   parent DOM node, and a shared [`Truck`] for app-wide state.
//! - [`Truck`] — typed key-value bag for app-level context (URL,
//!   config, anything you'd put in a React context). Cloned by `Rc<RefCell<_>>`
//!   into each [`Scope`].
//!
//! See [`reflow::batch`] / [`reflow::untrack`] /
//! [`reflow::untracked_read`] for the write- vs read-side scheduling
//! controls.

#[macro_use]
mod cfg;

#[macro_use]
extern crate cfg_if;

// Since the SSR convergence (`web-ssr` renders by replaying the command
// stream) every non-browser target shares the CommandNode backend, so
// `backend-command` + `web-ssr` may coexist in one build (a fullstack
// desktop binary, or a workspace-wide build). Browser CSR is still an
// exclusive target: it owns the browser DOM Node type and implies the
// single-app scheduler.
#[cfg(all(feature = "web-csr", feature = "web-ssr"))]
compile_error!("features `web-csr` and `web-ssr` are mutually exclusive; build browser CSR and server SSR as separate targets");
#[cfg(all(feature = "backend-command", feature = "web-csr", target_arch = "wasm32"))]
compile_error!("feature `backend-command` cannot be combined with `web-csr` on wasm32: the browser DOM owns the Node type there");
#[cfg(all(feature = "backend-command", feature = "single-app"))]
compile_error!("feature `backend-command` requires the multi-holder scheduler; do not combine it with `single-app`");

pub mod assets;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub mod console;
pub mod reflow;
pub mod renderer;
pub mod scope;
pub mod view;
pub mod web;
mod widget;
pub use scope::Scope;
pub use view::{View, ViewFactory, ViewId};
pub use widget::{BoundaryError, Filler, IntoFiller, Widget};
pub mod node;
pub use node::{Node, NodeRef};
pub mod widgets;

pub mod config;
pub use config::GloryConfig;
pub mod devtools;

mod truck;
pub use truck::Truck;

pub mod holder;
pub use holder::Holder;
#[cfg(not(feature = "single-app"))]
pub use holder::HolderId;

pub mod spawn;

pub use reflow::Cage;

use std::cell::RefCell;

#[cfg(not(feature = "single-app"))]
use indexmap::IndexMap;

thread_local! {
    #[cfg(feature = "single-app")]
    pub(crate) static ROOT_VIEWS: RefCell<view::ViewTree> = RefCell::default();
    #[cfg(not(feature = "single-app"))]
    pub(crate) static ROOT_VIEWS: RefCell<IndexMap<HolderId, view::ViewTree>> = RefCell::default();
}

/// Returns true if running on the server (SSR).
///
/// In the past, this was implemented by checking whether `not(target_arch = "wasm32")`.
/// Now that some cloud platforms are moving to run Wasm on the edge, we really can't
/// guarantee that compiling to Wasm means browser APIs are available, or that not compiling
/// to Wasm means we're running on the server.
///
/// ```
/// # use glory_core::is_server;
/// let todos = if is_server() {
///   // if on the server, load from DB
/// } else {
///   // if on the browser, do something else
/// };
/// ```
pub const fn is_server() -> bool {
    !is_browser()
}

/// Returns true if running on the browser (CSR).
///
/// ```
/// # use glory_core::is_browser;
/// let todos = if is_browser() {
///   // if on the browser, call `wasm_bindgen` methods
/// } else {
///   // if on the server, do something else
/// };
/// ```
pub const fn is_browser() -> bool {
    cfg!(all(target_arch = "wasm32", feature = "web-csr"))
}

/// Returns true if `debug_assertions` are enabled.
/// ```
/// # use glory_core::is_dev;
/// if is_dev() {
///   // log something or whatever
/// }
/// ```
pub const fn is_dev() -> bool {
    cfg!(debug_assertions)
}

/// Returns true if `debug_assertions` are disabled.
pub const fn is_release() -> bool {
    !is_dev()
}

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub use tracing::{error, info, warn};

/// Uses `println!()`-style formatting to log something to the console (in the browser)
/// or via `println!()` (if not in the browser).
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
#[macro_export]
macro_rules! info {
    ($($t:tt)*) => ($crate::console::log(&format_args!($($t)*).to_string()))
}

/// Uses `println!()`-style formatting to log warnings to the console (in the browser)
/// or via `eprintln!()` (if not in the browser).
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
#[macro_export]
macro_rules! warn {
    ($($t:tt)*) => ($crate::console::warn(&format_args!($($t)*).to_string()))
}

/// Uses `println!()`-style formatting to log errors to the console (in the browser)
/// or via `eprintln!()` (if not in the browser).
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
#[macro_export]
macro_rules! error {
    ($($t:tt)*) => ($crate::console::error(&format_args!($($t)*).to_string()))
}

/// Debug-only diagnostic warning. In a release build this expands to nothing —
/// no log call and no formatting cost — so hot paths that want to surface a
/// "this shouldn't normally happen" note (e.g. the scheduler visiting a
/// detached view during a large list replace) don't pay for it in production
/// or flood the browser console during benchmarks.
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
#[macro_export]
macro_rules! debug_warn {
    ($($t:tt)*) => {{
        #[cfg(debug_assertions)]
        $crate::console::debug_warn(&format_args!($($t)*).to_string());
    }};
}

/// Off-browser (SSR / native) build: no-op. `format_args!` keeps the arguments
/// "used" so callers don't trip unused-variable lints, and is fully optimised
/// away (no allocation, no output).
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
#[macro_export]
macro_rules! debug_warn {
    ($($t:tt)*) => {{
        let _ = format_args!($($t)*);
    }};
}
