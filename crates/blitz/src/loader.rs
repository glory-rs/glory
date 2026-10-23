//! Document-owned desktop transport. Only encoded bytes and handlers cross the
//! worker boundary; no Runtime, Cage, App or native window lives on that thread.
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use blitz_traits::net::{Body, Bytes, Method, NetHandler, NetProvider, Request, Url};
use tokio::sync::Notify;
use tokio::task::JoinSet;

mod files;

/// Independent of the document's decoded-image and registered-font budgets.
#[derive(Clone, Debug)]
pub struct LoadLimits {
    pub workers: usize,
    pub queued: usize,
    pub body_bytes: usize,
    /// Includes time spent waiting in the queue and all redirect hops.
    pub timeout: Duration,
    pub redirects: usize,
    pub url_bytes: usize,
    pub package_entries: usize,
    pub package_bytes: usize,
}
impl Default for LoadLimits {
    fn default() -> Self {
        Self {
            workers: 4,
            queued: 64,
            body_bytes: 8 * 1024 * 1024,
            timeout: Duration::from_secs(15),
            redirects: 4,
            url_bytes: 4096,
            package_entries: 256,
            package_bytes: 32 * 1024 * 1024,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct LoadConfig {
    /// Exact HTTP(S) origins, including effective port. Entries must be origin
    /// URLs with path `/`, no credentials, query or fragment. Empty denies HTTP.
    pub origins: Vec<Url>,
    /// Encoded paths such as `/theme.css` served under `asset://app/`.
    /// Immutable owned bytes, not a history cache or filesystem mount.
    pub package: BTreeMap<String, Bytes>,
    /// Read-only directory capabilities exposed as `asset://name/path`.
    /// Names are lowercase ASCII labels (not `app`); roots must be absolute.
    /// Handles are opened at construction, not re-resolved for each request.
    pub mounts: BTreeMap<String, PathBuf>,
    pub limits: LoadLimits,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadStats {
    pub queued: usize,
    pub active: usize,
    pub peak_queued: usize,
    pub peak_active: usize,
    pub completed: u64,
    pub failed: u64,
    pub rejected: u64,
    pub cancelled: u64,
    /// Actual blocking file operations, including cancelled reads that have
    /// not returned from the OS yet. Independent of transport active jobs.
    pub file_active: usize,
    pub file_peak_active: usize,
    /// No new jobs are accepted after close, including for another document.
    pub closed: bool,
    /// The dedicated runtime and its sockets have been dropped.
    pub stopped: bool,
}
#[derive(Default)]
struct Cancel {
    cancelled: AtomicBool,
    notify: Notify,
}
impl Cancel {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.notify.notify_one();
    }
    async fn wait(&self) {
        loop {
            let notified = self.notify.notified();
            if self.cancelled.load(Ordering::Acquire) {
                return;
            }
            notified.await;
        }
    }
}
struct Job {
    id: u64,
    request: Request,
    handler: Box<dyn NetHandler>,
    cancel: Arc<Cancel>,
    deadline: Instant,
}
struct Running {
    url: String,
    cancel: Arc<Cancel>,
}
#[derive(Default)]
struct State {
    doc: Option<usize>,
    next: u64,
    queue: VecDeque<Job>,
    running: BTreeMap<u64, Running>,
    stats: LoadStats,
}
struct Inner {
    config: LoadConfig,
    files: files::Files,
    state: Mutex<State>,
    notify: Notify,
    stopped: AtomicBool,
}

/// One Loader belongs to one document. Create another Loader for another
/// Renderer/WindowApp. Sharing an `Arc<Loader>` permits inspection, not reuse
/// after document close. Drop/close never joins a worker on the UI thread.
pub struct Loader {
    inner: Arc<Inner>,
}
impl Loader {
    pub fn new(config: LoadConfig) -> io::Result<Self> {
        validate(&config)?;
        let files = files::Files::new(&config.mounts, config.limits.workers)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
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
        let inner = Arc::new(Inner {
            config,
            files,
            state: Mutex::new(State::default()),
            notify: Notify::new(),
            stopped: AtomicBool::new(false),
        });
        let worker = inner.clone();
        std::thread::Builder::new()
            .name("glory-assets".into())
            .spawn(move || {
                runtime.block_on(run(worker.clone(), client));
                // OS getaddrinfo is not abortable. Resolution is single-flight
                // once per allowed host, and never retains UI/document state.
                runtime.shutdown_timeout(Duration::ZERO);
                worker.stopped.store(true, Ordering::Release);
            })?;
        Ok(Self { inner })
    }
    pub fn stats(&self) -> LoadStats {
        let state = self.inner.state.lock().expect("asset state");
        LoadStats {
            queued: state.queue.len(),
            active: state.running.len(),
            stopped: self.inner.stopped.load(Ordering::Acquire),
            file_active: self.inner.files.active(),
            file_peak_active: self.inner.files.peak(),
            ..state.stats
        }
    }
    /// Revoke queued/in-flight requests and close sockets asynchronously.
    pub fn shutdown(&self) {
        let (removed, running) = {
            let mut state = self.inner.state.lock().expect("asset state");
            state.stats.closed = true;
            let removed = std::mem::take(&mut state.queue);
            state.stats.cancelled = state.stats.cancelled.saturating_add(removed.len() as u64);
            let running: Vec<_> = state
                .running
                .values()
                .map(|job| job.cancel.clone())
                .collect();
            (removed, running)
        };
        drop(removed);
        self.inner.files.close();
        for cancel in running {
            cancel.cancel();
        }
        self.inner.notify.notify_one();
    }
}
impl Drop for Loader {
    fn drop(&mut self) {
        self.shutdown();
    }
}
impl NetProvider for Loader {
    fn fetch(&self, doc: usize, request: Request, handler: Box<dyn NetHandler>) {
        let url = request.url.to_string();
        let invalid = self.inner.config.check(&request.url).err().or_else(|| {
            (request.method != Method::GET
                || !matches!(request.body, Body::Empty)
                || !request.headers.is_empty())
            .then(|| "assets require GET with empty body/headers".into())
        });
        let mut job = Some(Job {
            id: 0,
            request,
            handler,
            cancel: Arc::new(Cancel::default()),
            deadline: Instant::now() + self.inner.config.limits.timeout,
        });
        let error = {
            let mut state = self.inner.state.lock().expect("asset state");
            if state.stats.closed {
                // Closed documents must not receive new callbacks.
                None
            } else if let Some(error) = invalid {
                state.stats.rejected = state.stats.rejected.saturating_add(1);
                Some(error)
            } else if state.doc.is_some_and(|current| current != doc) {
                state.stats.rejected = state.stats.rejected.saturating_add(1);
                Some("asset loader already belongs to another document".into())
            } else if state.queue.len() >= self.inner.config.limits.queued || state.next == u64::MAX
            {
                state.stats.rejected = state.stats.rejected.saturating_add(1);
                Some("asset queue/identity budget".into())
            } else {
                let mut admitted = job.take().expect("new asset job");
                state.doc = Some(doc);
                admitted.id = state.next;
                state.next += 1;
                state.queue.push_back(admitted);
                state.stats.peak_queued = state.stats.peak_queued.max(state.queue.len());
                None
            }
        };
        if let Some(error) = error {
            job.take()
                .expect("rejected asset job")
                .handler
                .error(url, error);
        }
        drop(job);
        self.inner.notify.notify_one();
    }
    fn cancel(&self, doc: usize, url: &str) {
        let (removed, running) = {
            let mut state = self.inner.state.lock().expect("asset state");
            if state.doc != Some(doc) {
                return;
            }
            let mut removed = Vec::new();
            let mut kept = VecDeque::new();
            while let Some(job) = state.queue.pop_front() {
                if job.request.url.as_str() == url {
                    removed.push(job);
                } else {
                    kept.push_back(job);
                }
            }
            state.queue = kept;
            state.stats.cancelled = state.stats.cancelled.saturating_add(removed.len() as u64);
            let running: Vec<_> = state
                .running
                .values()
                .filter(|job| job.url == url)
                .map(|job| job.cancel.clone())
                .collect();
            (removed, running)
        };
        // Handler destructors and notification code execute outside the lock.
        drop(removed);
        for cancel in running {
            cancel.cancel();
        }
        self.inner.notify.notify_one();
    }
    fn close(&self, doc: usize) {
        let matches = self
            .inner
            .state
            .lock()
            .expect("asset state")
            .doc
            .is_none_or(|current| current == doc);
        if matches {
            self.shutdown();
        }
    }
}

fn validate(config: &LoadConfig) -> io::Result<()> {
    let limits = &config.limits;
    if limits.workers == 0
        || limits.workers > 32
        || limits.queued == 0
        || limits.queued > 4096
        || limits.body_bytes == 0
        || limits.body_bytes > 64 * 1024 * 1024
        || limits.timeout < Duration::from_millis(20)
        || limits.timeout > Duration::from_secs(120)
        || limits.redirects > 16
        || !(256..=8192).contains(&limits.url_bytes)
        || limits.package_entries > 4096
        || limits.package_bytes > 128 * 1024 * 1024
        || config.origins.len() > 64
        || config.package.len() > limits.package_entries
        || config.mounts.len() > 32
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "asset limits"));
    }
    for origin in &config.origins {
        if origin.as_str().len() > limits.url_bytes
            || !matches!(origin.scheme(), "http" | "https")
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.path() != "/"
            || origin.query().is_some()
            || origin.fragment().is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected HTTP(S) origin",
            ));
        }
    }
    let base = Url::parse("asset://app/").expect("asset base");
    let mut total = 0usize;
    for (path, bytes) in &config.package {
        let url = base.join(path).map_err(io::Error::other)?;
        if !path.starts_with('/')
            || url.host_str() != Some("app")
            || url.path() != path
            || url.query().is_some()
            || url.fragment().is_some()
            || path.len() > limits.url_bytes
            || bytes.len() > limits.body_bytes
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "asset package path/body",
            ));
        }
        total = total
            .checked_add(path.len())
            .and_then(|total| total.checked_add(bytes.len()))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "asset package overflow"))?;
    }
    if total > limits.package_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "asset package budget",
        ));
    }
    Ok(())
}
impl LoadConfig {
    fn check(&self, url: &Url) -> Result<(), String> {
        if url.as_str().len() > self.limits.url_bytes
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("asset URL length/credentials".into());
        }
        if url.scheme() == "asset"
            && url.port().is_none()
            && url
                .host_str()
                .is_some_and(|host| host == "app" || self.mounts.contains_key(host))
        {
            return Ok(());
        }
        if matches!(url.scheme(), "http" | "https")
            && self
                .origins
                .iter()
                .any(|origin| origin.origin() == url.origin())
        {
            return Ok(());
        }
        Err("asset URL outside allowed origins/package".into())
    }
}

async fn run(inner: Arc<Inner>, client: reqwest::Client) {
    let mut tasks = JoinSet::new();
    loop {
        while tasks.try_join_next().is_some() {}
        // One receiver: notify_one stores a permit across the state inspection.
        let notified = inner.notify.notified();
        let (launch, expired, closed) = {
            let mut state = inner.state.lock().expect("asset state");
            let mut expired = Vec::new();
            let mut kept = VecDeque::new();
            while let Some(job) = state.queue.pop_front() {
                if job.deadline <= Instant::now()
                    || job
                        .request
                        .signal
                        .as_ref()
                        .is_some_and(|signal| signal.aborted())
                {
                    expired.push(job);
                } else {
                    kept.push_back(job);
                }
            }
            state.queue = kept;
            let mut launch = Vec::new();
            while !state.stats.closed && state.running.len() < inner.config.limits.workers {
                let Some(job) = state.queue.pop_front() else {
                    break;
                };
                state.running.insert(
                    job.id,
                    Running {
                        url: job.request.url.to_string(),
                        cancel: job.cancel.clone(),
                    },
                );
                launch.push(job);
            }
            state.stats.peak_active = state.stats.peak_active.max(state.running.len());
            (launch, expired, state.stats.closed)
        };
        for job in expired {
            let aborted = job
                .request
                .signal
                .as_ref()
                .is_some_and(|signal| signal.aborted());
            {
                let mut state = inner.state.lock().expect("asset state");
                if aborted {
                    state.stats.cancelled = state.stats.cancelled.saturating_add(1);
                } else {
                    state.stats.failed = state.stats.failed.saturating_add(1);
                }
            }
            job.handler.error(
                job.request.url.to_string(),
                if aborted {
                    "asset cancelled"
                } else {
                    "asset queue deadline"
                }
                .into(),
            );
        }
        for job in launch {
            tasks.spawn(work(inner.clone(), client.clone(), job));
        }
        if closed && tasks.is_empty() {
            break;
        }
        tokio::select! {
            _ = notified => {},
            _ = tasks.join_next(), if !tasks.is_empty() => {},
            // Observe external AbortSignals and expire queued work even while
            // all I/O slots are occupied. Work depends on bounded jobs only.
            _ = tokio::time::sleep(Duration::from_millis(20)), if !tasks.is_empty() => {},
        }
    }
}
struct RunningGuard {
    inner: Arc<Inner>,
    id: u64,
}
impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.inner
            .state
            .lock()
            .expect("asset state")
            .running
            .remove(&self.id);
        self.inner.notify.notify_one();
    }
}
async fn work(inner: Arc<Inner>, client: reqwest::Client, mut job: Job) {
    let _guard = RunningGuard {
        inner: inner.clone(),
        id: job.id,
    };
    let cancelled = async {
        loop {
            if job
                .request
                .signal
                .as_ref()
                .is_some_and(|signal| signal.aborted())
            {
                return;
            }
            tokio::select! {
                _ = job.cancel.wait() => return,
                _ = tokio::time::sleep(Duration::from_millis(20)) => {},
            }
        }
    };
    let mut result = tokio::select! {
        biased;
        _ = cancelled => None,
        result = tokio::time::timeout_at(job.deadline.into(), async {
            let (url, bytes) = load(&inner, &client, job.request.url.clone(), job.deadline).await?;
            job.handler.prepare(&url, &bytes).await?;
            Ok((url, bytes))
        }) => {
            Some(result.unwrap_or_else(|_| Err("asset deadline".into())))
        },
    };
    let signal_aborted = job
        .request
        .signal
        .as_ref()
        .is_some_and(|signal| signal.aborted());
    if signal_aborted {
        result = Some(Err("asset cancelled".into()));
    }
    let publish = {
        let mut state = inner.state.lock().expect("asset state");
        let publish = !state.stats.closed
            && !job.cancel.cancelled.load(Ordering::Acquire)
            && result.is_some();
        if !publish || signal_aborted {
            state.stats.cancelled = state.stats.cancelled.saturating_add(1);
        } else if result.as_ref().is_some_and(Result::is_ok) {
            state.stats.completed = state.stats.completed.saturating_add(1);
        } else {
            state.stats.failed = state.stats.failed.saturating_add(1);
        }
        publish
    };
    // Publication's acceptance point is above. An already-running handler
    // cannot be interrupted; the document's request IDs reject obsolete data.
    // Keep the worker slot until parsing/decode ends, including recursive CSS.
    if publish {
        match result.expect("published resource") {
            Ok((url, bytes)) => job.handler.bytes(url, bytes),
            Err(error) => job.handler.error(job.request.url.to_string(), error),
        }
    }
}
async fn load(
    inner: &Inner,
    client: &reqwest::Client,
    mut url: Url,
    deadline: Instant,
) -> Result<(String, Bytes), String> {
    let config = &inner.config;
    for hop in 0..=config.limits.redirects {
        config.check(&url)?;
        if url.scheme() == "asset" {
            if url.host_str() != Some("app") {
                let bytes = inner
                    .files
                    .read(&url, config.limits.body_bytes, deadline)
                    .await?;
                return Ok((url.to_string(), bytes));
            }
            return config
                .package
                .get(url.path())
                .cloned()
                .map(|bytes| (url.to_string(), bytes))
                .ok_or_else(|| "asset package miss".into());
        }
        let mut response = client
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
