//! Static asset declarations.
//!
//! Use [`asset!`] to declare an asset path once and let each backend ask
//! for the representation it needs. Web/SSR code typically uses
//! [`Asset::public_path`], while tooling and native backends can use
//! [`Asset::absolute_path`].

use std::borrow::Cow;
use std::fmt;

/// A statically declared asset path.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Asset {
    logical_path: &'static str,
    absolute_path: &'static str,
}

impl Asset {
    #[doc(hidden)]
    pub const fn from_static(logical_path: &'static str, absolute_path: &'static str) -> Self {
        Self { logical_path, absolute_path }
    }

    /// Path exactly as written in [`asset!`].
    pub const fn logical_path(self) -> &'static str {
        self.logical_path
    }

    /// Absolute filesystem path resolved relative to the declaring
    /// crate's `CARGO_MANIFEST_DIR`.
    pub const fn absolute_path(self) -> &'static str {
        self.absolute_path
    }

    /// URL path suitable for HTML attributes such as `src` / `href`.
    ///
    /// Relative declarations are normalized to absolute web paths:
    /// `asset!("assets/logo.png").public_path()` returns
    /// `/assets/logo.png`.
    pub fn public_path(self) -> Cow<'static, str> {
        public_path(self.logical_path)
    }

    /// URL path under an explicit mount prefix.
    ///
    /// `asset!("icons/a.svg").public_path_with_base("/static")`
    /// returns `/static/icons/a.svg`.
    pub fn public_path_with_base(self, base: &str) -> String {
        let base = base.trim_end_matches('/');
        let path = self.public_path();
        if base.is_empty() || base == "/" {
            path.into_owned()
        } else {
            format!("{base}{path}")
        }
    }
}

impl fmt::Debug for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Asset")
            .field("logical_path", &self.logical_path)
            .field("absolute_path", &self.absolute_path)
            .field("public_path", &self.public_path())
            .finish()
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.public_path())
    }
}

fn public_path(path: &'static str) -> Cow<'static, str> {
    let trimmed = path.strip_prefix("./").unwrap_or(path);
    if trimmed.starts_with('/') {
        Cow::Borrowed(trimmed)
    } else {
        Cow::Owned(format!("/{trimmed}"))
    }
}

/// Declare a static asset path.
///
/// ```ignore
/// use glory::asset;
///
/// let logo = asset!("assets/logo.png");
/// assert_eq!(logo.public_path(), "/assets/logo.png");
/// ```
#[macro_export]
macro_rules! asset {
    ($path:literal) => {
        $crate::assets::Asset::from_static($path, concat!(env!("CARGO_MANIFEST_DIR"), "/", $path))
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn asset_public_path_normalizes_relative_paths() {
        let asset = crate::asset!("assets/logo.png");

        assert_eq!(asset.logical_path(), "assets/logo.png");
        assert_eq!(asset.public_path(), "/assets/logo.png");
        assert!(asset.absolute_path().ends_with("/assets/logo.png") || asset.absolute_path().ends_with("\\assets/logo.png"));
    }

    #[test]
    fn asset_public_path_strips_dot_slash() {
        let asset = crate::asset!("./logo.png");

        assert_eq!(asset.public_path(), "/logo.png");
        assert_eq!(asset.public_path_with_base("/static/"), "/static/logo.png");
    }

    #[test]
    fn asset_public_path_keeps_absolute_paths() {
        let asset = crate::asset!("/favicon.ico");

        assert_eq!(asset.public_path(), "/favicon.ico");
        assert_eq!(asset.public_path_with_base("/static"), "/static/favicon.ico");
    }
}
