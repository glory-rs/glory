//! Static asset declarations.
//!
//! Use [`asset!`] to declare an asset path once and let each backend ask
//! for the representation it needs. The macro verifies the file exists at
//! compile time relative to the declaring crate's `CARGO_MANIFEST_DIR`.
//! Web/SSR code typically uses [`Asset::public_path`], while tooling and
//! native backends can use [`Asset::absolute_path`].

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::Deserialize;

static INSTALLED_MANIFEST: Lazy<RwLock<AssetManifest>> = Lazy::new(|| RwLock::new(AssetManifest::default()));

/// A statically declared asset path.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Asset {
    logical_path: &'static str,
    absolute_path: &'static str,
}

/// A statically enumerated folder of assets.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetFolder {
    logical_path: &'static str,
    assets: &'static [Asset],
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
    /// `/assets/logo.png`. When an [`AssetManifest`] is installed, this
    /// resolves to the hashed bundle path recorded for that public path.
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

impl AssetFolder {
    #[doc(hidden)]
    pub const fn from_static(logical_path: &'static str, assets: &'static [Asset]) -> Self {
        Self { logical_path, assets }
    }

    /// Folder path exactly as written in `asset_folder!`.
    pub const fn logical_path(self) -> &'static str {
        self.logical_path
    }

    /// Statically enumerated files under this folder, in stable path order.
    pub const fn assets(self) -> &'static [Asset] {
        self.assets
    }

    pub const fn len(self) -> usize {
        self.assets.len()
    }

    pub const fn is_empty(self) -> bool {
        self.assets.is_empty()
    }

    pub fn iter(self) -> std::slice::Iter<'static, Asset> {
        self.assets.iter()
    }

    pub fn get(self, logical_path: &str) -> Option<Asset> {
        self.assets.iter().copied().find(|asset| asset.logical_path() == logical_path)
    }
}

/// Runtime mapping from declared public paths to bundle-produced hashed
/// paths.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetManifest {
    paths: BTreeMap<String, String>,
}

impl AssetManifest {
    pub fn from_mappings<I, K, V>(paths: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        Self {
            paths: paths
                .into_iter()
                .map(|(from, to)| (normalize_public_path(from.as_ref()), normalize_public_path(to.as_ref())))
                .collect(),
        }
    }

    /// Parses the `asset_map` object written by `glory bundle` into a
    /// runtime manifest. Unknown fields in the bundle manifest are ignored.
    pub fn from_bundle_json(json: &str) -> serde_json::Result<Self> {
        #[derive(Deserialize)]
        struct BundleManifest {
            #[serde(default)]
            asset_map: BTreeMap<String, String>,
        }

        let manifest = serde_json::from_str::<BundleManifest>(json)?;
        Ok(Self::from_mappings(manifest.asset_map))
    }

    pub fn resolve_public_path(&self, public_path: &str) -> Option<&str> {
        let normalized = normalize_public_path(public_path);
        self.paths.get(&normalized).map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
}

/// Installs a bundle asset manifest for subsequent [`Asset::public_path`]
/// calls in this process.
pub fn install_asset_manifest(manifest: AssetManifest) {
    *INSTALLED_MANIFEST.write() = manifest;
}

/// Clears the process-wide asset manifest. Mainly useful for tests and
/// long-lived hosts that switch between apps.
pub fn clear_asset_manifest() {
    *INSTALLED_MANIFEST.write() = AssetManifest::default();
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

impl fmt::Debug for AssetFolder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetFolder")
            .field("logical_path", &self.logical_path)
            .field("assets", &self.assets)
            .finish()
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.public_path())
    }
}

fn public_path(path: &'static str) -> Cow<'static, str> {
    let normalized = normalize_public_path(path);
    if let Some(mapped) = INSTALLED_MANIFEST.read().resolve_public_path(&normalized) {
        return Cow::Owned(mapped.to_owned());
    }
    if normalized == path {
        Cow::Borrowed(path)
    } else {
        Cow::Owned(normalized)
    }
}

fn normalize_public_path(path: &str) -> String {
    let trimmed = path.strip_prefix("./").unwrap_or(path).replace('\\', "/");
    if trimmed.starts_with('/') { trimmed } else { format!("/{trimmed}") }
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
    ($path:literal) => {{
        const _GLORY_ASSET_EXISTS: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $path));
        $crate::assets::Asset::from_static($path, concat!(env!("CARGO_MANIFEST_DIR"), "/", $path))
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn asset_public_path_normalizes_relative_paths() {
        let asset = crate::asset!("src/assets.rs");

        assert_eq!(asset.logical_path(), "src/assets.rs");
        assert_eq!(asset.public_path(), "/src/assets.rs");
        assert!(asset.absolute_path().ends_with("/src/assets.rs") || asset.absolute_path().ends_with("\\src/assets.rs"));
    }

    #[test]
    fn asset_public_path_strips_dot_slash() {
        let asset = crate::asset!("./src/assets.rs");

        assert_eq!(asset.public_path(), "/src/assets.rs");
        assert_eq!(asset.public_path_with_base("/static/"), "/static/src/assets.rs");
    }

    #[test]
    fn asset_public_path_keeps_absolute_paths() {
        let asset = crate::asset!("/src/assets.rs");

        assert_eq!(asset.public_path(), "/src/assets.rs");
        assert_eq!(asset.public_path_with_base("/static"), "/static/src/assets.rs");
    }

    #[test]
    fn asset_macro_is_const_compatible() {
        const ASSET: crate::assets::Asset = crate::asset!("src/assets.rs");
        assert_eq!(ASSET.public_path(), "/src/assets.rs");
    }

    #[test]
    fn installed_manifest_rewrites_public_path() {
        crate::assets::install_asset_manifest(crate::assets::AssetManifest::from_mappings([(
            "/assets/logo.png",
            "/assets/logo.12345678.png",
        )]));

        let asset = crate::assets::Asset::from_static("assets/logo.png", "assets/logo.png");
        assert_eq!(asset.public_path(), "/assets/logo.12345678.png");
        crate::assets::clear_asset_manifest();
    }

    #[test]
    fn manifest_parses_bundle_asset_map() {
        let manifest = crate::assets::AssetManifest::from_bundle_json(
            r#"{
                "name": "demo",
                "asset_map": {
                    "assets/app.css": "assets/app.abcdef12.css"
                }
            }"#,
        )
        .unwrap();

        assert_eq!(manifest.resolve_public_path("/assets/app.css"), Some("/assets/app.abcdef12.css"));
        assert_eq!(manifest.resolve_public_path("/assets/missing.css"), None);
    }

    #[test]
    fn asset_folder_exposes_static_assets() {
        const ASSET: crate::assets::Asset = crate::assets::Asset::from_static("src/assets.rs", "src/assets.rs");
        const FOLDER: crate::assets::AssetFolder = crate::assets::AssetFolder::from_static("src", &[ASSET]);

        assert_eq!(FOLDER.logical_path(), "src");
        assert_eq!(FOLDER.len(), 1);
        assert_eq!(FOLDER.assets()[0].logical_path(), "src/assets.rs");
        assert_eq!(FOLDER.get("src/assets.rs"), Some(ASSET));
        assert_eq!(FOLDER.get("src/missing.rs"), None);
    }
}
