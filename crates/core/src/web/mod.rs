pub mod events;
pub mod helpers;
pub mod holders;
pub mod widgets;

mod attr;
mod class;
mod prop;
pub mod utils;

pub use attr::AttrValue;
pub use class::{Classes, ClassPart};
pub use helpers::*;
pub use prop::PropValue;
pub use widgets::Element;

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
mod csr;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub use csr::*;

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub fn escape(s: &str) -> String {
    js_sys::encode_uri(s).as_string().unwrap()
}
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub fn escape(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}


#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub fn unescape<'a>(s: &'a str) -> String {
    js_sys::decode_uri(s).unwrap().into()
}
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub fn unescape<'a>(s: &'a str) -> String {
    percent_encoding::percent_decode_str(s).decode_utf8_lossy().to_string()
}

