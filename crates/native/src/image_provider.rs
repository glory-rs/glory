//! Image / sub-resource loading for the Blitz native backend (NT5).
//!
//! Blitz drives image loading itself: when an `<img src>` enters the document
//! it resolves the raw `src` against the document's `base_url`
//! ([`blitz_dom::BaseDocument::set_base_url`]) and then calls
//! [`NetProvider::fetch`](blitz_traits::net::NetProvider) with a
//! [`Request`](blitz_traits::net::Request) and an internal `ImageHandler` that
//! decodes the returned bytes (the `image` crate, plus SVG when enabled).
//! There is no public hook to intercept that per-`<img>` resolution
//! (`BaseDocument::resolve_url` is `pub(crate)`), so Glory's role is twofold:
//!
//! 1. Choose the document `base_url` (the assets root) so that relative
//!    `<img src>` values resolve into the asset tree, and
//! 2. Provide a [`NetProvider`] that turns the *already resolved* `Url` into
//!    bytes — reading from disk for `file://`, accepting inline `data:` URIs,
//!    and (compile-only here) leaving `http(s)://` to a real async fetcher.
//!
//! The interesting, testable logic — deciding *where a URL loads from* and how
//! a `glory://` / relative / absolute reference maps onto the assets root — is
//! factored into the dependency-free pure functions [`classify_asset_url`] and
//! [`resolve_asset_url`], which are exercised by unit tests without any Blitz
//! runtime.
//!
//! ## Blitz alpha limitations (compile-only paths)
//! - **Network fetch**: `http(s)://` images need an async HTTP client and a
//!   runtime to drive it. Wiring that to a real reactor needs a live event
//!   loop, so [`GloryImageProvider`] only *classifies* network URLs and skips
//!   them with a log line (the `fetch` signature and `NetHandler` plumbing are
//!   `cargo check`-covered). A host that wants network images can supply its
//!   own `NetProvider` (e.g. `blitz-net`) via `set_net_provider`.
//! - **Decode/layout**: turning decoded bytes into a painted image happens
//!   inside blitz-dom's layout/damage pass and needs the full document +
//!   (eventually) GPU paint, so it is not unit-testable here.

use std::path::{Path, PathBuf};

/// Where a resolved `<img src>` ultimately loads from.
///
/// Produced by the pure [`classify_asset_url`] so routing decisions can be
/// unit-tested without a Blitz document or a network stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetSource {
    /// A local file to read from disk (resolved `file://` or a bare path).
    File(PathBuf),
    /// A remote resource to fetch over the network (`http`/`https`).
    Network(String),
    /// An inline `data:` URI — the payload travels in the URL itself.
    Data(String),
    /// A scheme we do not know how to load.
    Unsupported(String),
}

/// Glory's custom asset scheme. `glory://logo.png` (or `glory:///logo.png`)
/// refers to a file *relative to the assets root*, regardless of the document
/// base URL — the native counterpart of the web build's public asset path.
pub const GLORY_SCHEME: &str = "glory://";

/// Resolve a raw `<img src>` reference against an assets root into a concrete,
/// loadable URL string — pure, no I/O, no Blitz types.
///
/// This mirrors how the web backend resolves public asset paths, and is what a
/// host can use to seed the document `base_url` or to pre-resolve sources:
///
/// - `glory://foo/bar.png` / `glory:///foo/bar.png` → joined onto `assets_root`
///   as a `file://` URL (the leading slashes and scheme are stripped first).
/// - Absolute URLs (`http://`, `https://`, `data:`, `file://`) pass through
///   unchanged — they already name a concrete location.
/// - Anything else is treated as a path relative to `assets_root`.
///
/// `assets_root` is interpreted as a filesystem directory; the returned value
/// for local references is a `file://` URL so it round-trips through Blitz's
/// `Url`-based resolution.
pub fn resolve_asset_url(raw: &str, assets_root: &Path) -> String {
    let raw = raw.trim();

    if let Some(rest) = strip_glory_scheme(raw) {
        return file_url_for(&join_relative(assets_root, rest));
    }

    if has_absolute_scheme(raw) {
        return raw.to_owned();
    }

    // Relative reference: resolve against the assets root on disk.
    file_url_for(&join_relative(assets_root, raw))
}

/// Classify a *resolved* URL (what Blitz hands to the [`NetProvider`]) into the
/// [`AssetSource`] that says how to load it. Pure and Blitz-free.
///
/// `http`/`https` → [`AssetSource::Network`], `file` → [`AssetSource::File`]
/// (percent-decoded back to a path), `data` → [`AssetSource::Data`], everything
/// else → [`AssetSource::Unsupported`]. A bare path with no scheme is treated
/// as a local file so direct filesystem URLs still work.
pub fn classify_asset_url(url: &str) -> AssetSource {
    let url = url.trim();
    match scheme_of(url) {
        Some("http") | Some("https") => AssetSource::Network(url.to_owned()),
        Some("data") => AssetSource::Data(url.to_owned()),
        Some("file") => AssetSource::File(file_url_to_path(url)),
        Some(other) => AssetSource::Unsupported(other.to_owned()),
        // No scheme at all: treat as a local filesystem path.
        None => AssetSource::File(PathBuf::from(url)),
    }
}

/// `glory://x`/`glory:///x` → `Some("x")` (leading slashes stripped), else `None`.
fn strip_glory_scheme(raw: &str) -> Option<&str> {
    raw.strip_prefix(GLORY_SCHEME).map(|rest| rest.trim_start_matches('/'))
}

/// Whether `raw` already carries an absolute URL scheme we should not rewrite.
fn has_absolute_scheme(raw: &str) -> bool {
    matches!(scheme_of(raw), Some("http" | "https" | "data" | "file"))
}

/// The scheme of `url` (lowercased, ASCII only) if it has one.
///
/// A "scheme" here is `ALPHA *( ALPHA / DIGIT / "+" / "-" / "." ) ":"` per
/// RFC 3986, which keeps Windows drive letters (`C:\...`) from being mistaken
/// for single-letter schemes — those are reported as having no scheme.
fn scheme_of(url: &str) -> Option<&str> {
    let colon = url.find(':')?;
    let scheme = &url[..colon];
    if scheme.len() < 2 {
        // Reject empty and single-char "schemes" (e.g. drive letters `C:`).
        return None;
    }
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        Some(scheme.split(':').next().unwrap_or(scheme))
    } else {
        None
    }
}

/// Join a relative reference onto a root directory, ignoring any leading slash
/// on the reference so it cannot escape to the filesystem root.
fn join_relative(root: &Path, rel: &str) -> PathBuf {
    let rel = rel.trim_start_matches(['/', '\\']);
    root.join(rel)
}

/// Build a `file://` URL string for a local path (forward slashes, leading `/`).
fn file_url_for(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.starts_with('/') {
        format!("file://{text}")
    } else {
        // Absolute Windows paths (`C:/...`) and bare relatives both get a
        // leading slash so the result is `file:///C:/...` / `file:///rel`.
        format!("file:///{text}")
    }
}

/// Inverse of [`file_url_for`] (best effort): `file:///C:/a.png` → `C:/a.png`.
fn file_url_to_path(url: &str) -> PathBuf {
    let rest = url.strip_prefix("file://").map(|r| r.strip_prefix('/').unwrap_or(r)).unwrap_or(url);
    PathBuf::from(decode_percent(rest))
}

/// Minimal percent-decoding for `file://` paths (`%20` → space, etc.). Enough to
/// round-trip the paths [`file_url_for`] produces; not a general URL decoder.
fn decode_percent(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push(hi * 16 + lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(feature = "blitz")]
pub use provider::GloryImageProvider;

#[cfg(feature = "blitz")]
mod provider {
    use std::path::PathBuf;
    use std::sync::Arc;

    use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request};

    use super::{AssetSource, classify_asset_url};

    /// A [`NetProvider`] that serves `<img src>` (and other sub-resource)
    /// requests for the native backend.
    ///
    /// Local files (`file://` after Blitz resolves a relative `src` against the
    /// document base URL, or a bare path) are read synchronously and handed to
    /// blitz-dom's decoder. `data:` URIs are decoded inline. `http(s)://` is
    /// recognised but **not fetched here** — see the module docs: it needs an
    /// async client + runtime, so it is left for a host-provided network
    /// provider and is `cargo check`-covered only.
    #[derive(Clone, Default)]
    pub struct GloryImageProvider {
        /// Optional directory prepended to bare relative `file` paths that are
        /// not already absolute. Mostly redundant with the document base URL,
        /// but lets a headless consumer load assets without a base URL set.
        assets_root: Option<PathBuf>,
    }

    impl GloryImageProvider {
        pub fn new() -> Self {
            Self::default()
        }

        /// Set a fallback assets root used for bare relative file paths.
        pub fn with_assets_root(mut self, root: impl Into<PathBuf>) -> Self {
            self.assets_root = Some(root.into());
            self
        }

        /// Convenience: wrap in the `Arc` that
        /// [`set_net_provider`](blitz_dom::BaseDocument::set_net_provider)
        /// expects.
        pub fn shared(self) -> Arc<dyn NetProvider> {
            Arc::new(self)
        }

        /// Resolve a classified file path against the fallback assets root when
        /// it is relative; absolute paths are used as-is.
        fn resolve_file(&self, path: PathBuf) -> PathBuf {
            if path.is_absolute() {
                return path;
            }
            match &self.assets_root {
                Some(root) => root.join(path),
                None => path,
            }
        }
    }

    impl NetProvider for GloryImageProvider {
        fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
            let url = request.url.as_str().to_owned();
            match classify_asset_url(&url) {
                AssetSource::File(path) => {
                    let path = self.resolve_file(path);
                    match std::fs::read(&path) {
                        Ok(bytes) => handler.bytes(url, Bytes::from(bytes)),
                        Err(err) => {
                            log::warn!("glory image provider: failed to read {}: {err}", path.display());
                        }
                    }
                }
                AssetSource::Data(_) => match decode_data_uri(&url) {
                    Some(bytes) => handler.bytes(url, Bytes::from(bytes)),
                    None => log::warn!("glory image provider: malformed data URI"),
                },
                AssetSource::Network(target) => {
                    // Compile-only on this backend: a real fetch needs an async
                    // HTTP client + runtime. Hosts that need network images
                    // should install their own NetProvider (e.g. blitz-net).
                    log::info!("glory image provider: network fetch not handled in-process: {target}");
                }
                AssetSource::Unsupported(scheme) => {
                    log::warn!("glory image provider: unsupported URL scheme '{scheme}'");
                }
            }
        }
    }

    /// Decode a `data:[<mediatype>][;base64],<data>` URI into raw bytes.
    /// Returns `None` for non-`data:` or malformed input. Only the `;base64`
    /// form is decoded to bytes; plain (percent-encoded text) payloads are
    /// returned as their UTF-8 bytes after percent-decoding.
    fn decode_data_uri(url: &str) -> Option<Vec<u8>> {
        let rest = url.strip_prefix("data:")?;
        let comma = rest.find(',')?;
        let (meta, payload) = rest.split_at(comma);
        let payload = &payload[1..];
        if meta.rsplit(';').any(|part| part.eq_ignore_ascii_case("base64")) {
            base64_decode(payload)
        } else {
            Some(super::decode_percent(payload).into_bytes())
        }
    }

    /// Standard-alphabet base64 decoder (no external dep), tolerant of padding
    /// and embedded whitespace. Returns `None` on invalid input.
    fn base64_decode(input: &str) -> Option<Vec<u8>> {
        fn val(c: u8) -> Option<u32> {
            match c {
                b'A'..=b'Z' => Some((c - b'A') as u32),
                b'a'..=b'z' => Some((c - b'a' + 26) as u32),
                b'0'..=b'9' => Some((c - b'0' + 52) as u32),
                b'+' => Some(62),
                b'/' => Some(63),
                _ => None,
            }
        }
        let mut out = Vec::new();
        let mut buf = 0u32;
        let mut bits = 0u32;
        for &c in input.as_bytes() {
            if c == b'=' || c.is_ascii_whitespace() {
                continue;
            }
            let v = val(c)?;
            buf = (buf << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buf >> bits) as u8);
            }
        }
        Some(out)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn data_uri_base64_round_trips() {
            // "Hi" base64 = "SGk="
            let bytes = decode_data_uri("data:text/plain;base64,SGk=").unwrap();
            assert_eq!(bytes, b"Hi");
        }

        #[test]
        fn data_uri_plain_is_percent_decoded() {
            let bytes = decode_data_uri("data:text/plain,a%20b").unwrap();
            assert_eq!(bytes, b"a b");
        }

        #[test]
        fn data_uri_rejects_non_data_scheme() {
            assert!(decode_data_uri("http://example.com").is_none());
        }

        #[test]
        fn base64_decode_handles_whitespace_and_padding() {
            assert_eq!(base64_decode("SG ks=").unwrap(), base64_decode("SGks").unwrap());
        }

        #[test]
        fn relative_file_resolves_against_assets_root() {
            let provider = GloryImageProvider::new().with_assets_root("/assets");
            let resolved = provider.resolve_file(PathBuf::from("logo.png"));
            assert_eq!(resolved, PathBuf::from("/assets").join("logo.png"));
        }

        #[test]
        fn absolute_file_path_is_left_untouched() {
            let provider = GloryImageProvider::new().with_assets_root("/assets");
            #[cfg(windows)]
            let abs = PathBuf::from(r"C:\img\a.png");
            #[cfg(not(windows))]
            let abs = PathBuf::from("/img/a.png");
            assert_eq!(provider.resolve_file(abs.clone()), abs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/srv/assets")
    }

    #[test]
    fn glory_scheme_maps_onto_assets_root() {
        assert_eq!(resolve_asset_url("glory://logo.png", &root()), "file:///srv/assets/logo.png");
        // Triple-slash form (authority-less) resolves the same way.
        assert_eq!(resolve_asset_url("glory:///icons/a.svg", &root()), "file:///srv/assets/icons/a.svg");
    }

    #[test]
    fn relative_reference_resolves_under_assets_root() {
        assert_eq!(resolve_asset_url("img/cat.png", &root()), "file:///srv/assets/img/cat.png");
        // A leading slash must not escape the assets root.
        assert_eq!(resolve_asset_url("/evil.png", &root()), "file:///srv/assets/evil.png");
    }

    #[test]
    fn absolute_urls_pass_through_unchanged() {
        for url in [
            "http://example.com/a.png",
            "https://cdn.test/b.jpg",
            "data:image/png;base64,AAAA",
            "file:///etc/x.png",
        ] {
            assert_eq!(resolve_asset_url(url, &root()), url);
        }
    }

    #[test]
    fn whitespace_is_trimmed_before_resolution() {
        assert_eq!(resolve_asset_url("  glory://a.png  ", &root()), "file:///srv/assets/a.png");
    }

    #[test]
    fn classify_routes_network_file_data_and_unsupported() {
        assert_eq!(classify_asset_url("http://x/y.png"), AssetSource::Network("http://x/y.png".into()));
        assert_eq!(classify_asset_url("https://x/y.png"), AssetSource::Network("https://x/y.png".into()));
        assert_eq!(
            classify_asset_url("data:image/png;base64,AAAA"),
            AssetSource::Data("data:image/png;base64,AAAA".into())
        );
        assert_eq!(classify_asset_url("ftp://x/y"), AssetSource::Unsupported("ftp".into()));
    }

    #[test]
    fn classify_file_url_decodes_back_to_path() {
        assert_eq!(
            classify_asset_url("file:///srv/assets/a%20b.png"),
            AssetSource::File(PathBuf::from("srv/assets/a b.png"))
        );
    }

    #[test]
    fn classify_bare_path_is_a_file() {
        assert_eq!(classify_asset_url("relative/a.png"), AssetSource::File(PathBuf::from("relative/a.png")));
    }

    #[test]
    fn windows_drive_letter_is_not_a_scheme() {
        // `C:` must not be read as a single-letter URL scheme.
        assert_eq!(scheme_of(r"C:\img\a.png"), None);
        assert_eq!(scheme_of("http://x"), Some("http"));
        assert_eq!(scheme_of("nocolon"), None);
    }
}
