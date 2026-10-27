//! Current document-private preview bodies; no file:// or path fallback.
//! Encoded bodies stay charged while a cancelled decoder still holds them.
use super::{PreviewCharge, worker::Worker};
use blitz_traits::net::{Bytes, Method, NetHandler, NetProvider, Request};
use glory_render::{FilePreview, FileRejection, FileToken};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::Notify;
pub(crate) struct Body {
    pub bytes: Bytes,
    pub _charge: PreviewCharge,
}
#[derive(Default)]
struct Cancel {
    flag: AtomicBool,
    notify: Notify,
}
impl Cancel {
    fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }
    fn cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
    async fn wait(&self) {
        // Register before checking, so cancellation cannot be lost.
        let notified = self.notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if !self.cancelled() {
            notified.await;
        }
    }
}
struct Entry {
    preview: FilePreview,
    body: Arc<Body>,
    life: Arc<Cancel>,
}
struct Fetch {
    doc: usize,
    url: String,
    cancel: Arc<Cancel>,
}
#[derive(Default)]
struct State {
    entries: BTreeMap<FileToken, Entry>,
    active: BTreeMap<u64, Fetch>,
    next: u64,
    closed: bool,
}
#[derive(Default)]
pub(crate) struct Registry {
    state: Mutex<State>,
    worker: OnceLock<Result<Worker, FileRejection>>,
}
impl Registry {
    pub fn get(&self, token: FileToken) -> Option<FilePreview> {
        self.state
            .lock()
            .expect("file previews")
            .entries
            .get(&token)
            .map(|e| e.preview.clone())
    }
    pub fn publish(
        &self,
        preview: FilePreview,
        body: Arc<Body>,
    ) -> Result<FilePreview, FileRejection> {
        let mut state = self.state.lock().expect("file previews");
        if state.closed {
            return Err(FileRejection::Unavailable);
        }
        if let Some(entry) = state.entries.get(&preview.file) {
            return Ok(entry.preview.clone());
        }
        // The selection budget already bounds live tokens to 128.
        if state.entries.len() >= 128 {
            return Err(FileRejection::Busy);
        }
        state.entries.insert(
            preview.file,
            Entry {
                preview: preview.clone(),
                body,
                life: Arc::new(Cancel::default()),
            },
        );
        Ok(preview)
    }
    pub fn retain(&self, mut valid: impl FnMut(FileToken) -> bool) -> Vec<String> {
        let mut revoked = Vec::new();
        self.state
            .lock()
            .expect("file previews")
            .entries
            .retain(|file, entry| {
                if valid(*file) {
                    true
                } else {
                    entry.life.cancel();
                    revoked.push(entry.preview.source.clone());
                    false
                }
            });
        revoked
    }
    pub fn shutdown(&self) {
        let mut state = self.state.lock().expect("file previews");
        state.closed = true;
        for entry in state.entries.values() {
            entry.life.cancel();
        }
        for fetch in state.active.values() {
            fetch.cancel.cancel();
        }
        state.entries.clear();
    }
}
struct Flight {
    registry: Weak<Registry>,
    id: u64,
}
impl Drop for Flight {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade() {
            registry
                .state
                .lock()
                .expect("file previews")
                .active
                .remove(&self.id);
        }
    }
}
/// Composes an optional application's provider with one private registry.
/// Unknown private URLs are rejected here, never delegated to another transport.
pub(crate) struct Provider {
    pub registry: Arc<Registry>,
    pub fallback: Option<Arc<dyn NetProvider>>,
}
impl NetProvider for Provider {
    fn fetch(&self, doc: usize, request: Request, handler: Box<dyn NetHandler>) {
        if request.url.scheme() != "file-preview" {
            if let Some(fallback) = &self.fallback {
                fallback.fetch(doc, request, handler);
            } else {
                handler.error(
                    request.url.to_string(),
                    "no resource transport configured".into(),
                );
            }
            return;
        }
        let url = request.url.to_string();
        let admission = (|| {
            let mut state = self.registry.state.lock().expect("file previews");
            if state.closed || request.method != Method::GET || state.active.len() >= 4 {
                return Err("file preview unavailable");
            }
            let entry = state
                .entries
                .values()
                .find(|entry| entry.preview.source == url)
                .ok_or("file preview grant revoked")?;
            let body = entry.body.clone();
            let life = entry.life.clone();
            let id = state
                .next
                .checked_add(1)
                .ok_or("file preview ticket exhausted")?;
            let cancel = Arc::new(Cancel::default());
            state.next = id;
            state.active.insert(
                id,
                Fetch {
                    doc,
                    url: url.clone(),
                    cancel: cancel.clone(),
                },
            );
            Ok((
                body,
                life,
                cancel,
                Flight {
                    registry: Arc::downgrade(&self.registry),
                    id,
                },
            ))
        })();
        let (body, life, cancel, flight) = match admission {
            Ok(admission) => admission,
            Err(error) => {
                handler.error(url, error.into());
                return;
            }
        };
        let worker = match self.registry.worker.get_or_init(Worker::new) {
            Ok(worker) => worker,
            Err(_) => {
                drop(flight);
                handler.error(url, "file preview worker unavailable".into());
                return;
            }
        };
        let holder = Arc::new(Mutex::new(Some(handler)));
        let task_handler = holder.clone();
        let sent = worker.send(Box::pin(async move {
            let mut handler = task_handler.lock().expect("file preview handler").take().expect("owned handler");
            let _flight = flight;
            if life.cancelled() || cancel.cancelled() { return; }
            let ready = tokio::select! {
                biased;
                _ = life.wait() => return,
                _ = cancel.wait() => return,
                _ = tokio::time::sleep(Duration::from_secs(15)) => Err("file preview preparation timed out".into()),
                result = handler.prepare(&url, &body.bytes) => result,
            };
            if life.cancelled() || cancel.cancelled() { return; }
            match ready {
                Ok(()) => handler.bytes(url, body.bytes.clone()),
                Err(error) => handler.error(url, error),
            }
        }));
        let rejected = if sent.is_err() {
            holder.lock().expect("file preview handler").take()
        } else {
            None
        };
        if let Some(handler) = rejected {
            handler.error(
                request.url.to_string(),
                "file preview queue unavailable".into(),
            );
        }
    }
    fn cancel(&self, doc: usize, url: &str) {
        if url.starts_with("file-preview:") {
            let state = self.registry.state.lock().expect("file previews");
            for fetch in state
                .active
                .values()
                .filter(|f| f.doc == doc && f.url == url)
            {
                fetch.cancel.cancel();
            }
        } else if let Some(fallback) = &self.fallback {
            fallback.cancel(doc, url);
        }
    }
    fn close(&self, doc: usize) {
        self.registry.shutdown();
        if let Some(fallback) = &self.fallback {
            fallback.close(doc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::Shared;
    use glory_render::{FileDocumentId, FileImageFormat, FileImageInfo, NodeIds, RuntimeId};
    use std::{sync::atomic::AtomicUsize, time::Instant};
    #[derive(Default)]
    struct Observed {
        prepared: AtomicUsize,
        dropped: AtomicUsize,
        values: Mutex<Vec<Result<Bytes, String>>>,
    }
    struct Handler {
        observed: Arc<Observed>,
        pending: bool,
    }
    impl NetHandler for Handler {
        fn prepare<'a>(&'a mut self, _: &'a str, _: &'a Bytes) -> blitz_traits::net::NetReady<'a> {
            self.observed.prepared.fetch_add(1, Ordering::AcqRel);
            if self.pending {
                Box::pin(std::future::pending())
            } else {
                Box::pin(std::future::ready(Ok(())))
            }
        }
        fn bytes(self: Box<Self>, _: String, bytes: Bytes) {
            self.observed.values.lock().unwrap().push(Ok(bytes));
        }
        fn error(self: Box<Self>, _: String, error: String) {
            self.observed.values.lock().unwrap().push(Err(error));
        }
    }
    impl Drop for Handler {
        fn drop(&mut self) {
            self.observed.dropped.fetch_add(1, Ordering::AcqRel);
        }
    }
    fn wait(mut done: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(10);
        while !done() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn fixture() -> (Provider, Arc<Shared>, FilePreview) {
        let shared = Arc::new(Shared::default());
        let file = FileToken {
            document: FileDocumentId::new().unwrap(),
            node: NodeIds::new(RuntimeId::new()).allocate(),
            revision: 0,
            serial: 1,
        };
        let preview = FilePreview {
            file,
            image: FileImageInfo {
                format: FileImageFormat::Png,
                width: 1,
                height: 1,
            },
            source: FilePreview::native_source(file),
        };
        let registry = Arc::new(Registry::default());
        registry
            .publish(
                preview.clone(),
                Arc::new(Body {
                    bytes: Bytes::from_static(b"owned"),
                    _charge: shared.preview(5).unwrap(),
                }),
            )
            .unwrap();
        (
            Provider {
                registry,
                fallback: None,
            },
            shared,
            preview,
        )
    }
    fn fetch(provider: &Provider, doc: usize, url: &str, observed: &Arc<Observed>, pending: bool) {
        provider.fetch(
            doc,
            Request::get(url.parse().unwrap()),
            Box::new(Handler {
                observed: observed.clone(),
                pending,
            }),
        );
    }
    #[test]
    fn revoking_a_grant_wakes_all_four_decoders_and_releases_their_encoded_leases() {
        let (provider, shared, preview) = fixture();
        let observed = Arc::new(Observed::default());
        for doc in 1..=4 {
            fetch(&provider, doc, &preview.source, &observed, true);
        }
        wait(|| observed.prepared.load(Ordering::Acquire) == 4);
        fetch(&provider, 5, &preview.source, &observed, true);
        assert_eq!(
            observed.values.lock().unwrap().len(),
            1,
            "fifth decoder is refused"
        );
        assert_eq!(shared.0.lock().unwrap().stats.preview_bytes, 5);
        assert_eq!(
            provider.registry.retain(|_| false),
            vec![preview.source.clone()]
        );
        wait(|| observed.dropped.load(Ordering::Acquire) == 5);
        wait(|| provider.registry.state.lock().unwrap().active.is_empty());
        wait(|| shared.0.lock().unwrap().stats.preview_bytes == 0);
        assert_eq!(
            observed.values.lock().unwrap().len(),
            1,
            "cancelled preparation never publishes"
        );
        fetch(&provider, 6, &preview.source, &observed, false);
        assert_eq!(observed.prepared.load(Ordering::Acquire), 4);
        assert_eq!(observed.values.lock().unwrap().len(), 2);
    }
    #[test]
    fn cancelling_a_fetch_does_not_revoke_the_selected_grant_and_private_uris_never_fall_through() {
        #[derive(Default)]
        struct Fallback(AtomicUsize);
        impl NetProvider for Fallback {
            fn fetch(&self, _: usize, request: Request, handler: Box<dyn NetHandler>) {
                self.0.fetch_add(1, Ordering::AcqRel);
                handler.error(request.url.to_string(), "external transport".into());
            }
        }
        let (mut provider, shared, preview) = fixture();
        let fallback = Arc::new(Fallback::default());
        provider.fallback = Some(fallback.clone());
        let cancelled = Arc::new(Observed::default());
        fetch(&provider, 1, &preview.source, &cancelled, true);
        wait(|| cancelled.prepared.load(Ordering::Acquire) == 1);
        provider.cancel(1, &preview.source);
        wait(|| cancelled.dropped.load(Ordering::Acquire) == 1);
        assert!(provider.registry.get(preview.file).is_some());
        let result = Arc::new(Observed::default());
        fetch(&provider, 2, &preview.source, &result, false);
        wait(|| result.dropped.load(Ordering::Acquire) == 1);
        assert_eq!(
            result.values.lock().unwrap().as_slice(),
            &[Ok(Bytes::from_static(b"owned"))]
        );
        fetch(
            &provider,
            2,
            "file-preview://document-0/ungranted",
            &result,
            false,
        );
        assert_eq!(fallback.0.load(Ordering::Acquire), 0);
        fetch(
            &provider,
            2,
            "https://example.test/theme.css",
            &result,
            false,
        );
        assert_eq!(fallback.0.load(Ordering::Acquire), 1);
        provider.close(2);
        wait(|| shared.0.lock().unwrap().stats.preview_bytes == 0);
        assert!(provider.registry.get(preview.file).is_none());
    }
}
