pub mod events;
pub mod helpers;
pub mod holders;
pub mod widgets;

mod attr;
mod class;
mod prop;
mod style;
pub mod utils;

pub use attr::AttrValue;
pub use class::{ClassPart, Classes};
pub use helpers::*;
pub use prop::PropValue;
pub use style::{ScopedStyle, scoped_css};
pub use widgets::Element;

#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
mod csr;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub use csr::*;

/// Characters left as-is when percent-encoding one URL path segment: the
/// RFC 3986 "unreserved" set (`ALPHA / DIGIT / - . _ ~`). Everything else,
/// `/ ? # & = +` included, is encoded — a route parameter is one segment and
/// must not be able to introduce another one, a query, or a fragment.
const PATH_SEGMENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC.remove(b'-').remove(b'.').remove(b'_').remove(b'~');

/// Percent-encodes one URL path segment.
///
/// Both targets use `percent_encoding` so CSR and SSR produce byte-identical
/// URLs. CSR previously used `encodeURI`, which leaves `/ ? # & = +` alone: a
/// route parameter of `"a/b"` silently became two path segments in the
/// browser (and `"x?y=1"` injected a query), while the server encoded them.
/// The mismatch also made every `<a href>` differ between the SSR markup and
/// the hydrated client.
pub fn escape(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, PATH_SEGMENT).to_string()
}

/// Reverses [`escape`].
///
/// Also uniform across targets: CSR used `decodeURI`, which refuses to decode
/// `%2F` (so a slash-bearing parameter arrived as the literal text `%2F`) and
/// throws on a malformed sequence — one bad `<a href>` on the page was enough
/// to trap the whole wasm app.
pub fn unescape(s: &str) -> String {
    percent_encoding::percent_decode_str(s).decode_utf8_lossy().to_string()
}
