//! Bounded range reads from explicit host-owned capabilities, outside callbacks.
use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
};
use wry::{
    RequestAsyncResponder,
    http::{Request, Response},
};
pub const MAX_RANGE_BYTES: usize = 1024 * 1024;
const MAX_WHOLE_BYTES: usize = 8 * MAX_RANGE_BYTES;

pub trait AssetSource: Send + Sync {
    fn len(&self) -> u64;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn mime(&self) -> &str;
    fn current(&self) -> bool;
    /// Must return exactly the requested bytes; executed on an owned IO worker.
    fn read_range(&self, offset: u64, length: usize) -> Result<Vec<u8>, String>;
}
pub trait AssetProvider: Send + Sync {
    /// Exact opaque URL path only. Never resolve ambient files or remote URLs.
    fn get(&self, path: &str) -> Option<Arc<dyn AssetSource>>;
}
struct Job {
    request: Request<Vec<u8>>,
    responder: RequestAsyncResponder,
}
pub(super) struct Pool {
    sender: Option<mpsc::SyncSender<Job>>,
    workers: Vec<thread::JoinHandle<()>>,
}
impl Pool {
    pub fn new(provider: Arc<dyn AssetProvider>) -> Result<Self, super::ShellError> {
        let (sender, receiver) = mpsc::sync_channel::<Job>(16);
        let receiver = Arc::new(Mutex::new(receiver));
        let mut pool = Self {
            sender: Some(sender),
            workers: vec![],
        };
        for index in 0..4 {
            let receiver = receiver.clone();
            let provider = provider.clone();
            pool.workers.push(
                thread::Builder::new()
                    .name(format!("glory-local-asset-{index}"))
                    .spawn(move || {
                        loop {
                            let job = receiver.lock().unwrap().recv();
                            let Ok(job) = job else { break };
                            job.responder
                                .respond(response(provider.as_ref(), &job.request));
                        }
                    })
                    .map_err(|e| super::ShellError(e.to_string()))?,
            );
        }
        Ok(pool)
    }
    pub fn submit(&self, request: Request<Vec<u8>>, responder: RequestAsyncResponder) {
        let job = Job { request, responder };
        if let Err(error) = self
            .sender
            .as_ref()
            .expect("live local asset pool")
            .try_send(job)
        {
            let (mpsc::TrySendError::Full(job) | mpsc::TrySendError::Disconnected(job)) = error;
            job.responder.respond(empty(503));
        }
    }
}
impl Drop for Pool {
    fn drop(&mut self) {
        // No UI-thread blocking join of foreign OS IO. Started work retains its
        // source and permit until settlement. Providers revoke current on close.
        self.sender.take();
        for worker in self.workers.drain(..) {
            if worker.is_finished() {
                let _ = worker.join();
            }
        }
    }
}
fn empty(status: u16) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header("Cache-Control", "no-store")
        .body(vec![])
        .unwrap()
}
fn range(value: &str, len: u64) -> Option<(u64, usize)> {
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    if start.is_empty() {
        let suffix = end
            .parse::<u64>()
            .ok()?
            .min(len)
            .min(MAX_RANGE_BYTES as u64);
        return (suffix > 0).then_some((len - suffix, suffix as usize));
    }
    let start = start.parse::<u64>().ok()?;
    if start >= len {
        return None;
    }
    let end = if end.is_empty() {
        len - 1
    } else {
        end.parse::<u64>().ok()?.min(len - 1)
    };
    if end < start {
        return None;
    }
    Some((
        start,
        (end - start + 1).min(MAX_RANGE_BYTES as u64) as usize,
    ))
}
pub(super) fn response(
    provider: &dyn AssetProvider,
    request: &Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    if request.method() != "GET" && request.method() != "HEAD" {
        return empty(405);
    }
    let path = request.uri().path();
    if !crate::local_media::valid_path(path) || request.uri().query().is_some() {
        return empty(403);
    }
    let Some(source) = provider.get(path) else {
        return empty(404);
    };
    if !source.current() {
        return empty(410);
    }
    let len = source.len();
    let requested = request.headers().get("Range");
    let (offset, length, status) = if let Some(value) = requested {
        let Some((offset, length)) = value.to_str().ok().and_then(|v| range(v, len)) else {
            let mut response = empty(416);
            response
                .headers_mut()
                .insert("Content-Range", format!("bytes */{len}").parse().unwrap());
            return response;
        };
        (offset, length, 206)
    } else {
        if request.method() != "HEAD" && len > MAX_WHOLE_BYTES as u64 {
            return empty(413);
        }
        (0, len.min(MAX_WHOLE_BYTES as u64) as usize, 200)
    };
    let mime = source.mime();
    if wry::http::HeaderValue::from_str(mime).is_err() {
        return empty(500);
    }
    let mut builder = Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Accept-Ranges", "bytes")
        .header("Cache-Control", "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .header(
            "Content-Length",
            if request.method() == "HEAD" && requested.is_none() {
                len
            } else {
                length as u64
            },
        );
    if status == 206 {
        builder = builder.header(
            "Content-Range",
            format!("bytes {offset}-{}/{len}", offset + length as u64 - 1),
        );
    }
    let body = if request.method() == "HEAD" {
        vec![]
    } else {
        match source.read_range(offset, length) {
            Ok(bytes) if bytes.len() == length => bytes,
            _ => return empty(500),
        }
    };
    if !source.current() {
        return empty(410);
    }
    builder.body(body).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Source(std::sync::atomic::AtomicBool);
    impl AssetSource for Source {
        fn len(&self) -> u64 {
            3 * MAX_RANGE_BYTES as u64
        }
        fn mime(&self) -> &str {
            "audio/wav"
        }
        fn current(&self) -> bool {
            self.0.load(std::sync::atomic::Ordering::Acquire)
        }
        fn read_range(&self, offset: u64, length: usize) -> Result<Vec<u8>, String> {
            Ok(vec![(offset % 251) as u8; length])
        }
    }
    struct Provider(Arc<Source>);
    impl AssetProvider for Provider {
        fn get(&self, path: &str) -> Option<Arc<dyn AssetSource>> {
            (path == "/media/test").then(|| self.0.clone() as Arc<dyn AssetSource>)
        }
    }
    #[test]
    fn ranges_are_bounded_and_recheck_revocation() {
        let provider = Provider(Arc::new(Source(true.into())));
        let request = Request::builder()
            .uri("glory://localhost/media/test")
            .header("Range", "bytes=1048576-")
            .body(vec![])
            .unwrap();
        let response = response(&provider, &request);
        assert_eq!(response.status(), 206);
        assert_eq!(response.body().len(), MAX_RANGE_BYTES);
        assert_eq!(
            response.headers()["Content-Range"],
            "bytes 1048576-2097151/3145728"
        );
        provider
            .0
            .0
            .store(false, std::sync::atomic::Ordering::Release);
        assert_eq!(super::response(&provider, &request).status(), 410);
        assert_eq!(range("bytes=2-1", 4), None);
        assert_eq!(range("bytes=1-2,4-8", 10), None);
        assert_eq!(range("bytes=-2", 8), Some((6, 2)));
        assert_eq!(range("bytes=8-", 8), None);
    }
}
