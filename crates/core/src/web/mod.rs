pub mod events;
pub mod helpers;
pub mod holders;
pub mod widgets;

mod attr;
mod class;
mod prop;
pub mod utils;

pub use attr::AttrValue;
pub use class::{ClassPart, Classes};
pub use helpers::*;
pub use prop::PropValue;
pub use widgets::Element;

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
mod csr;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub use csr::*;

#[cfg(feature = "web-ssr")]
pub fn escape(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}
#[cfg(feature = "web-ssr")]
pub fn unescape<'a>(s: &'a str) -> std::borrow::Cow<'a, str> {
    percent_encoding::percent_decode_str(s).decode_utf8_lossy()
}

