//! Desktop HTTP and mounted-file transport; package workers do not compile this module.
use super::super::{Bytes, LoadConfig, Url, files};
use std::{
    collections::BTreeMap,
    io,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::sync::Notify;

pub(super) struct Desktop {
    files: files::Files,
    client: reqwest::Client,
}
impl Desktop {
    pub fn new(config: &LoadConfig, runtime: &tokio::runtime::Runtime) -> io::Result<Self> {
        let files = files::Files::new(&config.mounts, config.limits.workers)?;
        // An explicit provider avoids changing Rustls's process-global default.
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier =
            rustls_platform_verifier::Verifier::new(provider.clone()).map_err(io::Error::other)?;
        let tls = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(io::Error::other)?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(verifier))
            .with_no_client_auth();
        // Use the platform certificate/hostname verifier; no process-global
        // provider installation or accept-invalid-certificates switch.
        let client = {
            let _guard = runtime.enter();
            reqwest::Client::builder()
                .tls_backend_preconfigured(tls)
                .redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never())
                .no_proxy()
                .no_gzip()
                .no_brotli()
                .no_deflate()
                .no_zstd()
                .referer(false)
                .pool_max_idle_per_host(0)
                .dns_resolver(Arc::new(Resolver::new(&config.origins)))
                .connect_timeout(config.limits.timeout)
                .read_timeout(config.limits.timeout)
                .build()
                .map_err(io::Error::other)?
        };
        Ok(Self { files, client })
    }
    pub fn file_counts(&self) -> (usize, usize) {
        (self.files.active(), self.files.peak())
    }
    pub fn close(&self) {
        self.files.close();
    }
    pub async fn load(
        &self,
        config: &LoadConfig,
        mut url: Url,
        deadline: Instant,
    ) -> Result<(String, Bytes), String> {
        for hop in 0..=config.limits.redirects {
            config.check(&url)?;
            if url.scheme() == "asset" {
                let bytes = self
                    .files
                    .read(&url, config.limits.body_bytes, deadline)
                    .await?;
                return Ok((url.to_string(), bytes));
            }
            let mut response = self
                .client
                .get(url.clone())
                .header("accept-encoding", "identity")
                .send()
                .await
                .map_err(http_error)?;
            if matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
                if hop == config.limits.redirects {
                    return Err("asset redirect limit".into());
                }
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|value| value.to_str().ok())
                    .ok_or("asset redirect location")?;
                if location.len() > config.limits.url_bytes {
                    return Err("asset redirect URL length".into());
                }
                let next = url.join(location).map_err(|_| "asset redirect URL")?;
                if url.scheme() == "https" && next.scheme() != "https" {
                    return Err("asset redirect TLS downgrade".into());
                }
                url = next;
                // HTTP cannot redirect into the embedded package namespace.
                if !matches!(url.scheme(), "http" | "https") {
                    return Err("asset redirect scheme".into());
                }
                continue;
            }
            if !response.status().is_success() {
                return Err("asset HTTP status".into());
            }
            if response
                .headers()
                .get("content-encoding")
                .is_some_and(|value| value.as_bytes() != b"identity")
            {
                return Err("asset encoded content unsupported".into());
            }
            if response
                .content_length()
                .is_some_and(|size| size > config.limits.body_bytes as u64)
            {
                return Err("asset body budget".into());
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(http_error)? {
                if chunk.len() > config.limits.body_bytes.saturating_sub(bytes.len()) {
                    return Err("asset body budget".into());
                }
                bytes
                    .try_reserve(chunk.len())
                    .map_err(|_| "asset body allocation")?;
                bytes.extend_from_slice(&chunk);
            }
            return Ok((url.to_string(), bytes.into()));
        }
        Err("asset redirect limit".into())
    }
}

fn http_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "asset deadline"
    } else {
        "asset HTTP request/body"
    }
    .into()
}

// Repeated cancellation must not spawn an unbounded queue of uncancellable
// system resolver calls. At most one lookup per configured host per Loader;
// positive and negative results remain fixed until this document closes.
#[derive(Default)]
struct LookupState {
    started: bool,
    result: Option<Result<Vec<SocketAddr>, String>>,
}
#[derive(Default)]
struct Lookup {
    state: Mutex<LookupState>,
    notify: Notify,
}
struct Resolver(BTreeMap<String, Arc<Lookup>>);
impl Resolver {
    fn new(origins: &[Url]) -> Self {
        Self(
            origins
                .iter()
                .filter_map(|origin| origin.host_str())
                .map(|host| (host.to_owned(), Arc::new(Lookup::default())))
                .collect(),
        )
    }
}
impl reqwest::dns::Resolve for Resolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        let lookup = self.0.get(&host).cloned();
        Box::pin(async move {
            let lookup = lookup.ok_or_else(|| {
                io::Error::new(io::ErrorKind::PermissionDenied, "asset DNS origin")
            })?;
            loop {
                let notified = lookup.notify.notified();
                let (start, result) = {
                    let mut state = lookup.state.lock().expect("asset DNS");
                    let start = !state.started;
                    state.started = true;
                    (start, state.result.clone())
                };
                if let Some(result) = result {
                    let addresses = result.map_err(io::Error::other)?;
                    return Ok(Box::new(addresses.into_iter()) as reqwest::dns::Addrs);
                }
                if start {
                    let (lookup, host) = (lookup.clone(), host.clone());
                    tokio::spawn(async move {
                        let result = tokio::net::lookup_host((host.as_str(), 0))
                            .await
                            .map(|addresses| addresses.take(16).collect::<Vec<_>>())
                            .map_err(|_| "asset DNS lookup".to_owned())
                            .and_then(|addresses| {
                                if addresses.is_empty() {
                                    Err("asset DNS empty".into())
                                } else {
                                    Ok(addresses)
                                }
                            });
                        lookup.state.lock().expect("asset DNS").result = Some(result);
                        lookup.notify.notify_waiters();
                    });
                }
                notified.await;
            }
        })
    }
}
