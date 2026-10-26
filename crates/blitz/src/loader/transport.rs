use super::{Bytes, LoadConfig, Url};
use std::{io, time::Instant};

#[cfg(all(
    feature = "assets",
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]
#[path = "desktop.rs"]
mod desktop;

pub(super) struct Transport {
    #[cfg(all(feature = "android-assets", target_os = "android"))]
    android: Option<super::android::Android>,
    #[cfg(all(
        feature = "assets",
        any(target_os = "windows", target_os = "linux", target_os = "macos")
    ))]
    desktop: desktop::Desktop,
}
impl Transport {
    pub fn new(config: &LoadConfig, runtime: &tokio::runtime::Runtime) -> io::Result<Self> {
        #[cfg(all(
            feature = "assets",
            any(target_os = "windows", target_os = "linux", target_os = "macos")
        ))]
        {
            Ok(Self {
                desktop: desktop::Desktop::new(config, runtime)?,
            })
        }
        #[cfg(not(all(
            feature = "assets",
            any(target_os = "windows", target_os = "linux", target_os = "macos")
        )))]
        {
            let _ = runtime;
            if !config.origins.is_empty() || !config.mounts.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "HTTP and file mounts require desktop assets; this target supports package assets only",
                ));
            }
            Ok(Self {
                #[cfg(all(feature = "android-assets", target_os = "android"))]
                android: config
                    .android
                    .clone()
                    .map(|assets| super::android::Android::new(assets, config.limits.workers)),
            })
        }
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
            self.desktop.file_counts()
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
        self.desktop.close();
    }
    pub async fn load(
        &self,
        config: &LoadConfig,
        url: Url,
        deadline: Instant,
    ) -> Result<(String, Bytes), String> {
        config.check(&url)?;
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
        {
            self.desktop.load(config, url, deadline).await
        }
        #[cfg(not(all(
            feature = "assets",
            any(target_os = "windows", target_os = "linux", target_os = "macos")
        )))]
        {
            let _ = deadline;
            Err("transport unavailable for package-only loader".into())
        }
    }
}
