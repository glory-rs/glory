#[cfg(not(target_arch = "wasm32"))]
mod ssr;
#[cfg(not(target_arch = "wasm32"))]
pub use ssr::*;

#[cfg(target_arch = "wasm32")]
mod csr;
#[cfg(target_arch = "wasm32")]
pub use csr::*;
