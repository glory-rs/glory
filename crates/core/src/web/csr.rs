use std::sync::atomic::{AtomicBool, Ordering};

pub use wasm_bindgen::*;

pub(crate) static HYDRATING: AtomicBool = AtomicBool::new(false);
thread_local! {
    pub(crate) static WINDOW: web_sys::Window = web_sys::window().unwrap_throw();
    pub(crate) static DOCUMENT: web_sys::Document = web_sys::window().unwrap_throw().document().unwrap_throw();
}

/// Returns the [`Window`](https://developer.mozilla.org/en-US/docs/Web/API/Window).
///
/// This is cached as a thread-local variable, so calling `window()` multiple times
/// requires only one call out to JavaScript.
pub fn window() -> web_sys::Window {
    WINDOW.with(|window| window.clone())
}

/// Returns the [`Document`](https://developer.mozilla.org/en-US/docs/Web/API/Document).
///
/// This is cached as a thread-local variable, so calling `document()` multiple times
/// requires only one call out to JavaScript.
pub fn document() -> web_sys::Document {
    DOCUMENT.with(|document| document.clone())
}

pub fn is_hydrating() -> bool {
    HYDRATING.load(Ordering::Relaxed)
}
