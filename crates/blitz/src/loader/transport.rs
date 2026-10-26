use super::{Bytes, LoadConfig, Url};
use std::{io, time::Instant};

#[cfg(all(
    feature = "network",
    any(
        target_os = "windows",
        target_os = "linux",
        target_os = "macos",
        target_os = "android",
        target_os = "ios"
    )
))]
#[path = "http.rs"]
mod http;

pub(super) struct Transport {
    #[cfg(all(feature = "android-assets", target_os = "android"))]
    android: Option<super::android::Android>,
    #[cfg(all(
        feature = "assets",
        any(target_os = "windows", target_os = "linux", target_os = "macos")
    ))]
    files: super::files::Files,
    #[cfg(all(
        feature = "network",
        any(
            target_os = "windows",
            target_os = "linux",
            target_os = "macos",
            target_os = "android",
            target_os = "ios"
        )
    ))]
    http: Option<http::Http>,
}
impl Transport {
    pub fn new(config: &LoadConfig, runtime: &tokio::runtime::Runtime) -> io::Result<Self> {
        let _ = runtime;
        if !config.mounts.is_empty()
            && !cfg!(all(
                feature = "assets",
                any(
                    target_os = "windows",
                    target_os = "linux",
                    target_os = "macos"
                )
            ))
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "file mounts require desktop assets",
            ));
        }
        if !config.origins.is_empty()
            && !cfg!(all(
                feature = "network",
                any(
                    target_os = "windows",
                    target_os = "linux",
                    target_os = "macos",
                    target_os = "android",
                    target_os = "ios"
                )
            ))
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "HTTP(S) requires native network transport",
            ));
        }
        #[cfg(all(feature = "network", target_os = "android"))]
        if !config.origins.is_empty() && config.android_tls.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Android HTTP(S) requires explicit AndroidTls initialization",
            ));
        }
        Ok(Self {
            #[cfg(all(feature = "android-assets", target_os = "android"))]
            android: config
                .android
                .clone()
                .map(|assets| super::android::Android::new(assets, config.limits.workers)),
            #[cfg(all(
                feature = "assets",
                any(target_os = "windows", target_os = "linux", target_os = "macos")
            ))]
            files: super::files::Files::new(&config.mounts, config.limits.workers)?,
            #[cfg(all(
                feature = "network",
                any(
                    target_os = "windows",
                    target_os = "linux",
                    target_os = "macos",
                    target_os = "android",
                    target_os = "ios"
                )
            ))]
            http: if config.origins.is_empty() {
                None
            } else {
                Some(http::Http::new(config, runtime)?)
            },
        })
    }
    pub fn file_counts(&self) -> (usize, usize) {
        #[cfg(all(feature = "android-assets", target_os = "android"))]
        if let Some(android) = &self.android {
            return android.counts();
        }
        #[cfg(all(
            feature = "assets",
            any(target_os = "windows", target_os = "linux", target_os = "macos")
        ))]
        {
            (self.files.active(), self.files.peak())
        }
        #[cfg(not(all(
            feature = "assets",
            any(target_os = "windows", target_os = "linux", target_os = "macos")
        )))]
        {
            (0, 0)
        }
    }
    pub fn close(&self) {
        #[cfg(all(feature = "android-assets", target_os = "android"))]
        if let Some(android) = &self.android {
            android.close();
        }
        #[cfg(all(
            feature = "assets",
            any(target_os = "windows", target_os = "linux", target_os = "macos")
        ))]
        self.files.close();
    }
    pub async fn load(
        &self,
        config: &LoadConfig,
        url: Url,
        deadline: Instant,
    ) -> Result<(String, Bytes), String> {
        config.check(&url)?;
        let _ = deadline;
        if url.scheme() == "asset" && url.host_str() == Some("app") {
            return config
                .package
                .get(url.path())
                .cloned()
                .map(|bytes| (url.to_string(), bytes))
                .ok_or_else(|| "asset package miss".into());
        }
        #[cfg(all(feature = "android-assets", target_os = "android"))]
        if url.scheme() == "asset" && url.host_str() == Some("android") {
            let android = self
                .android
                .as_ref()
                .ok_or("Android AssetManager not configured")?;
            let bytes = android
                .read(&url, config.limits.body_bytes, deadline)
                .await?;
            return Ok((url.to_string(), bytes));
        }
        #[cfg(all(
            feature = "assets",
            any(target_os = "windows", target_os = "linux", target_os = "macos")
        ))]
        if url.scheme() == "asset" {
            let bytes = self
                .files
                .read(&url, config.limits.body_bytes, deadline)
                .await?;
            return Ok((url.to_string(), bytes));
        }
        #[cfg(all(
            feature = "network",
            any(
                target_os = "windows",
                target_os = "linux",
                target_os = "macos",
                target_os = "android",
                target_os = "ios"
            )
        ))]
        if matches!(url.scheme(), "http" | "https") {
            return self
                .http
                .as_ref()
                .ok_or("HTTP(S) not configured")?
                .load(config, url)
                .await;
        }
        Err("transport unavailable for this URL".into())
    }
}
