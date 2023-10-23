#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
mod csr;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub use csr::*;

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
mod ssr;
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub use ssr::*;
