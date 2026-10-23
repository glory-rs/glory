//! Incremental semantic-to-Blitz document adapter. Optional paint/raster record
//! scenes and CPU images; optional shell adds a desktop window/event-loop host.
//! Headless, window, editor/IME and accessibility evidence remain distinct.
#[cfg(all(
    feature = "accesskit",
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]
pub use ::accesskit;
#[cfg(not(target_arch = "wasm32"))]
mod document;
#[cfg(not(target_arch = "wasm32"))]
pub use document::*;

#[cfg(all(
    feature = "assets",
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]
mod loader;

/// Transport hooks stay in this adapter; core/UI do not depend on engine types.
#[cfg(not(target_arch = "wasm32"))]
pub mod assets {
    #[cfg(all(
        feature = "assets",
        any(target_os = "windows", target_os = "linux", target_os = "macos")
    ))]
    pub use crate::loader::{LoadConfig, LoadLimits, LoadStats, Loader};
    pub use blitz_dom::{ResourceLimits, ResourceStats};
    pub use blitz_traits::net::{
        AbortSignal, Body, Bytes, Method, NetHandler, NetProvider, NetReady, Request, Url,
    };
}
