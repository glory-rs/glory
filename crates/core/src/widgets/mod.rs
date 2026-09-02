mod each;
mod error_boundary;
mod suspense;
pub mod switch;

pub use each::Each;
pub use error_boundary::ErrorBoundary;
pub use suspense::Suspense;
pub use switch::Switch;

mod loader;
// `LoadState` is what `Loader::state()` returns, so callers need to be able to
// name and match it.
pub use loader::{LoadState, Loader, OnceLoader};

#[cfg(all(test, feature = "web-ssr", not(feature = "single-app")))]
mod snapshot_tests;
