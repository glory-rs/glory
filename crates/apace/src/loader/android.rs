//! Explicit APK asset capability; all NDK calls use its safe streaming API.
use super::Url;
use std::ffi::CString;

fn valid_component(part: &str) -> bool {
    !part.is_empty()
        && !matches!(part, "." | "..")
        && !part
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\'))
}

fn validate_prefix(prefix: &str) -> Result<(), &'static str> {
    if prefix.len() > 4096 || (!prefix.is_empty() && !prefix.split('/').all(valid_component)) {
        return Err("Android asset prefix");
    }
    Ok(())
}

fn asset_path(prefix: &str, url: &Url) -> Result<CString, String> {
    validate_prefix(prefix)?;
    if url.scheme() != "asset"
        || url.host_str() != Some("android")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Android asset URL".into());
    }
    let mut path = prefix.to_owned();
    for raw in url
        .path()
        .strip_prefix('/')
        .ok_or("Android asset path")?
        .split('/')
    {
        let part = percent_encoding::percent_decode_str(raw)
            .decode_utf8()
            .map_err(|_| "Android asset path UTF-8")?;
        if !valid_component(&part) {
            return Err("Android asset path component".into());
        }
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(&part);
    }
    CString::new(path).map_err(|_| "Android asset path NUL".into())
}

#[cfg(target_os = "android")]
mod native {
    use super::super::{
        Bytes,
        blocking::{Blocking, read_bytes},
    };
    use super::*;
    use ndk::asset::AssetManager;
    use std::{
        io,
        sync::{Arc, atomic::Ordering},
        time::Instant,
    };

    /// Read-only APK assets below an explicit prefix. Obtain the manager from
    /// this invocation's AndroidApp::asset_manager(); that application-scoped
    /// manager remains valid across Activity recreation. No Activity is retained.
    #[derive(Clone, Debug)]
    pub struct AndroidAssets {
        manager: Arc<AssetManager>,
        prefix: String,
    }
    impl AndroidAssets {
        /// Empty prefix grants all APK assets; `glory` grants only that subtree.
        pub fn new(manager: AssetManager, prefix: &str) -> io::Result<Self> {
            validate_prefix(prefix)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
            Ok(Self {
                manager: Arc::new(manager),
                prefix: prefix.into(),
            })
        }
    }

    pub(in super::super) struct Android {
        assets: AndroidAssets,
        blocking: Blocking,
    }
    impl Android {
        pub fn new(assets: AndroidAssets, workers: usize) -> Self {
            Self {
                assets,
                blocking: Blocking::new(workers),
            }
        }
        pub fn counts(&self) -> (usize, usize) {
            (self.blocking.active(), self.blocking.peak())
        }
        pub fn close(&self) {
            self.blocking.close();
        }
        pub async fn read(
            &self,
            url: &Url,
            limit: usize,
            deadline: Instant,
        ) -> Result<Bytes, String> {
            let path = asset_path(&self.assets.prefix, url)?;
            let manager = self.assets.manager.clone();
            self.blocking
                .run(move |cancel| {
                    if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
                        return Err("Android asset cancelled/deadline".into());
                    }
                    // Open and close on this worker; Asset itself never crosses
                    // threads. Do not map the entire potentially compressed body.
                    let asset = manager.open(&path).ok_or("Android asset missing")?;
                    let length = asset.length() as u64;
                    read_bytes(asset, length, limit, deadline, &cancel)
                })
                .await
        }
    }
}
#[cfg(target_os = "android")]
pub(super) use native::Android;
#[cfg(target_os = "android")]
pub use native::AndroidAssets;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apk_paths_are_decoded_once_and_stay_below_the_explicit_prefix() {
        for (url, expected) in [
            ("asset://android/theme.css?v=2#part", "glory/theme.css"),
            (
                "asset://android/%E4%B8%AD%E6%96%87/a%20b.png",
                "glory/中文/a b.png",
            ),
            ("asset://android/%252e%252e/name", "glory/%2e%2e/name"),
            // URL normalization cannot escape the separately joined prefix.
            ("asset://android/../outside", "glory/outside"),
            ("asset://android/CON", "glory/CON"),
        ] {
            assert_eq!(
                asset_path("glory", &Url::parse(url).unwrap())
                    .unwrap()
                    .to_str()
                    .unwrap(),
                expected
            );
        }
        assert_eq!(
            asset_path("", &Url::parse("asset://android/a").unwrap())
                .unwrap()
                .to_str()
                .unwrap(),
            "a"
        );
        for url in [
            "asset://app/a",
            "https://android/a",
            "asset://user@android/a",
            "asset://android:12/a",
            "asset://android/",
            "asset://android/a//b",
            "asset://android/a/",
            "asset://android/%2fescape",
            "asset://android/%5cescape",
            "asset://android/%00",
            "asset://android/%0A",
            "asset://android/%FF",
        ] {
            assert!(
                asset_path("glory", &Url::parse(url).unwrap()).is_err(),
                "{url}"
            );
        }
        for prefix in [
            "/root", "root/", "../root", "root/./x", "a//b", "a\\b", "a\0b",
        ] {
            assert!(validate_prefix(prefix).is_err(), "{prefix:?}");
        }
        assert!(validate_prefix(&"x".repeat(4097)).is_err());
    }
}
