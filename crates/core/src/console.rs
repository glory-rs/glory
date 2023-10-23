use crate::is_server;
use cfg_if;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::JsValue;

/// Log a string to the console (in the browser)
/// or via `println!()` (if not in the browser).
pub fn log(s: &str) {
    if is_server() {
        println!("{s}");
    } else {
        web_sys::console::log_1(&JsValue::from_str(s));
    }
}

/// Log a warning to the console (in the browser)
/// or via `println!()` (if not in the browser).
pub fn warn(s: &str) {
    if is_server() {
        eprintln!("{s}");
    } else {
        web_sys::console::warn_1(&JsValue::from_str(s));
    }
}

/// Log an error to the console (in the browser)
/// or via `println!()` (if not in the browser).
pub fn error(s: &str) {
    if is_server() {
        eprintln!("{s}");
    } else {
        web_sys::console::error_1(&JsValue::from_str(s));
    }
}

/// Log an error to the console (in the browser)
/// or via `println!()` (if not in the browser), but only in a debug build.
pub fn debug_warn(s: &str) {
    cfg_if! {
        if #[cfg(debug_assertions)] {
            if is_server() {
                eprintln!("{s}");
            } else {
                web_sys::console::warn_1(&JsValue::from_str(s));
            }
        } else {
          let _ = s;
        }
    }
}
