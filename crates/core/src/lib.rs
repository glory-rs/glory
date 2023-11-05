#[macro_use]
mod cfg;

#[macro_use]
extern crate cfg_if;

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub mod console;
pub mod reflow;
pub mod scope;
pub mod view;
pub mod web;
mod widget;
pub use scope::Scope;
pub use view::{View, ViewFactory, ViewId, ViewMap};
pub use widget::{Filler, IntoFiller, Widget};
pub mod node;
pub use node::{Node, NodeRef};
pub mod widgets;

pub mod config;
pub use config::GloryConfig;

mod truck;
pub use truck::Truck;

pub mod holder;
pub use holder::Holder;
#[cfg(not(feature = "__single_holder"))]
pub use holder::HolderId;

pub mod spawn;

pub use reflow::Cage;

use std::cell::RefCell;

#[cfg(not(feature = "__single_holder"))]
use indexmap::IndexMap;

thread_local! {
    #[cfg(feature = "__single_holder")]
    pub(crate) static ROOT_VIEWS: RefCell<ViewMap> = RefCell::default();
    #[cfg(not(feature = "__single_holder"))]
    pub(crate) static ROOT_VIEWS: RefCell<IndexMap<HolderId, ViewMap>> = RefCell::default();
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
