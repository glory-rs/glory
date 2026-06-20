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

/// Reads (and consumes) a server-streamed resource value for `token`, when the
/// SSR page embedded one in `window.__gloryResource`.
///
/// [`resource_hydratable_in`](crate::reflow::resource_hydratable_in) uses this
/// to adopt the value the server already computed and skip the client refetch.
/// The entry is deleted on read so a later re-render does not reuse stale data.
pub fn take_hydrated_resource<T>(token: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    let window = window();
    let store = js_sys::Reflect::get(&window, &JsValue::from_str("__gloryResource")).ok()?;
    let store: &js_sys::Object = store.dyn_ref()?;
    let key = JsValue::from_str(token);
    let value = js_sys::Reflect::get(store, &key).ok()?;
    if value.is_undefined() {
        return None;
    }
    let _ = js_sys::Reflect::delete_property(store, &key);
    let json = js_sys::JSON::stringify(&value).ok()?.as_string()?;
    serde_json::from_str(&json).ok()
}
