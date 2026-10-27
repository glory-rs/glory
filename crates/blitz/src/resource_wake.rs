//! Resource handlers may start before a window installs its shell provider.
//! Wake the current host through a stable relay, not the provider captured then.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::task::Waker;

use blitz_traits::net::{Bytes, NetHandler, NetProvider, NetReady, Request};

#[derive(Default)]
pub(super) struct ResourceWake {
    pending: AtomicBool,
    closed: AtomicBool,
    waker: Mutex<Option<Waker>>,
}
impl ResourceWake {
    pub(super) fn is_pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }
    pub(super) fn acknowledge(&self) {
        self.pending.store(false, Ordering::Release);
    }
    pub fn observe(&self, waker: &Waker) {
        let previous = {
            let mut current = self.waker.lock().expect("resource waker");
            if current
                .as_ref()
                .is_some_and(|current| current.will_wake(waker))
            {
                None
            } else {
                current.replace(waker.clone())
            }
        };
        drop(previous);
        self.pending.store(false, Ordering::Release);
    }
    pub(super) fn wake(&self) {
        if self.closed.load(Ordering::Acquire) || self.pending.swap(true, Ordering::AcqRel) {
            return;
        }
        let waker = self.waker.lock().expect("resource waker").clone();
        if let Some(waker) = waker {
            waker.wake();
        }
    }
    pub(super) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        let previous = self.waker.lock().expect("resource waker").take();
        drop(previous);
    }
}
impl std::task::Wake for ResourceWake {
    fn wake(self: Arc<Self>) {
        self.as_ref().wake();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.as_ref().wake();
    }
}
pub(super) struct Provider {
    pub inner: Arc<dyn NetProvider>,
    pub wake: Arc<ResourceWake>,
}
impl NetProvider for Provider {
    fn fetch(&self, doc: usize, request: Request, handler: Box<dyn NetHandler>) {
        if self.wake.closed.load(Ordering::Acquire) {
            return;
        }
        self.inner.fetch(
            doc,
            request,
            Box::new(Handler {
                inner: handler,
                wake: self.wake.clone(),
            }),
        );
    }
    fn is_noop(&self) -> bool {
        self.inner.is_noop()
    }
    fn cancel(&self, doc: usize, url: &str) {
        self.inner.cancel(doc, url);
    }
    fn close(&self, doc: usize) {
        self.wake.close();
        self.inner.close(doc);
    }
}
struct Handler {
    inner: Box<dyn NetHandler>,
    wake: Arc<ResourceWake>,
}
impl NetHandler for Handler {
    fn prepare<'a>(&'a mut self, url: &'a str, bytes: &'a Bytes) -> NetReady<'a> {
        if self.wake.closed.load(Ordering::Acquire) {
            return Box::pin(std::future::ready(Err("closed document".into())));
        }
        self.inner.prepare(url, bytes)
    }
    fn bytes(self: Box<Self>, url: String, bytes: Bytes) {
        if self.wake.closed.load(Ordering::Acquire) {
            return;
        }
        self.inner.bytes(url, bytes);
        self.wake.wake();
    }
    fn error(self: Box<Self>, url: String, error: String) {
        if self.wake.closed.load(Ordering::Acquire) {
            return;
        }
        self.inner.error(url, error);
        self.wake.wake();
    }
}
