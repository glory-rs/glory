//! Safe application drivers for the bounded, integer-only Swift host protocol.
//! The host owns UIKit objects, security scopes and physical native admission.
use futures_timer::Delay;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};
#[cfg(feature = "clipboard")]
mod clipboard;
#[cfg(feature = "files")]
mod files;
#[cfg(all(target_os = "ios", feature = "ios-io-probe"))]
pub(crate) mod probe;
#[cfg(all(target_os = "ios", feature = "ios-files-ui-probe"))]
pub(crate) mod system_files_probe;
#[cfg(feature = "clipboard")]
pub use clipboard::IosClipboard;
#[cfg(feature = "files")]
pub use files::IosFiles;

type Host = fn(i64, i64, i64, i64, i64, i64) -> i64;
#[cfg(target_os = "ios")]
// User-authorized application-only exception (2026-10-27). Swift validates
// all integer arguments and owns every native object/thread/lifetime. Packet
// words are values, never pointers. No caller safety preconditions or unsafe
// call blocks; runtime crates retain unsafe_code = deny.
#[allow(unsafe_code)]
unsafe extern "C" {
    safe fn glory_mobile_io(
        service: i64,
        client: i64,
        operation: i64,
        request: i64,
        a: i64,
        b: i64,
    ) -> i64;
}
#[cfg(target_os = "ios")]
fn native_host(service: i64, client: i64, operation: i64, request: i64, a: i64, b: i64) -> i64 {
    glory_mobile_io(service, client, operation, request, a, b)
}
/// Open an explicit app-owned client. Construction never accesses pasteboard
/// contents. Clone this driver to share request IDs and native admission.
#[cfg(all(target_os = "ios", feature = "clipboard"))]
pub fn clipboard() -> Result<Arc<IosClipboard>, glory::blitz::clipboard::ClipboardError> {
    IosClipboard::open(native_host).map(Arc::new)
}
/// Open an explicit app-owned picker/reader client. Construction never presents
/// a picker or accesses files; selected readers retain their client and scope.
#[cfg(all(target_os = "ios", feature = "files"))]
pub fn files() -> Result<Arc<IosFiles>, glory::render::FileRejection> {
    IosFiles::open(native_host).map(Arc::new)
}
#[cfg(all(target_os = "ios", feature = "ios-io-probe"))]
fn audit_integer_abi() {
    for service in [i64::MIN, -1, 0, 3, i64::MAX] {
        assert_eq!(native_host(service, 0, 12, 0, 0, 0), UNAVAILABLE);
    }
    for service in [1, 2] {
        for client in [i64::MIN, -1, 0, i64::MAX] {
            assert_eq!(native_host(service, client, 5, i64::MIN, 0, 0), UNAVAILABLE);
        }
    }
    let client = Client::open(native_host, 1).expect("integer ABI audit client");
    assert!(client.call(2, 0, 0, 0) < 0);
    assert!(client.call(2, 1, i64::MIN, 0) < 0);
    assert!(client.call(2, 2, 0, i64::MAX) < 0);
    assert_eq!(client.call(11, 0, 0, 0), 0);
    eprintln!("Glory iOS IO integer ABI validation passed");
}
const UNAVAILABLE: i64 = -1;
const FAILED: i64 = -3;
struct Client {
    host: Host,
    service: i64,
    id: i64,
    next: AtomicI64,
    submit: Mutex<()>,
    closed: AtomicBool,
}
impl Client {
    fn open(host: Host, service: i64) -> Result<Arc<Self>, i64> {
        if host(service, 0, 12, 0, 0, 0) != 1 {
            return Err(FAILED);
        }
        let id = host(service, 0, 0, 0, 0, 0);
        if id <= 0 {
            return Err(if id == 0 { FAILED } else { id });
        }
        Ok(Arc::new(Self {
            host,
            service,
            id,
            next: AtomicI64::new(1),
            submit: Mutex::new(()),
            closed: AtomicBool::new(false),
        }))
    }
    fn call(&self, op: i64, id: i64, a: i64, b: i64) -> i64 {
        if self.closed.load(Ordering::Acquire) {
            return UNAVAILABLE;
        }
        (self.host)(self.service, self.id, op, id, a, b)
    }
    fn close(&self) {
        if !self.closed.swap(true, Ordering::AcqRel) {
            (self.host)(self.service, self.id, 1, 0, 0, 0);
        }
    }
    fn job(self: &Arc<Self>, kind: i64, input: Vec<u8>, capacity: usize) -> Job {
        Job {
            client: self.clone(),
            input: Some(input),
            kind,
            id: None,
            timer: None,
            capacity,
            done: false,
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.close();
    }
}
struct Receipt {
    client: Arc<Client>,
    id: Option<i64>,
    code: i64,
    bytes: Vec<u8>,
}
impl Receipt {
    #[cfg(feature = "files")]
    fn adopt(&mut self) -> Result<(), i64> {
        accepted(self.client.call(9, self.id.ok_or(FAILED)?, 0, 0))?;
        self.id = None;
        Ok(())
    }
}
impl Drop for Receipt {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            self.client.call(8, id, 0, 0);
        }
    }
}
fn accepted(code: i64) -> Result<(), i64> {
    if code == 0 {
        Ok(())
    } else {
        Err(if code < 0 { code } else { FAILED })
    }
}
struct Job {
    client: Arc<Client>,
    input: Option<Vec<u8>>,
    kind: i64,
    id: Option<i64>,
    timer: Option<Delay>,
    capacity: usize,
    done: bool,
}
impl Job {
    fn cancel(&mut self) {
        if let Some(id) = self.id.take() {
            self.client.call(8, id, 0, 0);
        }
        self.timer = None;
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl Future for Job {
    type Output = Result<Receipt, i64>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.done {
            return Poll::Ready(Err(FAILED));
        }
        if let Some(timer) = &mut self.timer {
            if Pin::new(timer).poll(cx).is_pending() {
                return Poll::Pending;
            }
            self.timer = None;
        }
        let result = (|| {
            if let Some(input) = self.input.take() {
                let client = self.client.clone();
                let _submission = client.submit.lock().map_err(|_| FAILED)?;
                let id = client
                    .next
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
                    .map_err(|_| FAILED)?;
                self.id = Some(id);
                accepted(client.call(2, id, self.kind, input.len() as i64))?;
                for (index, chunk) in input.chunks(8).enumerate() {
                    let mut word = [0; 8];
                    word[..chunk.len()].copy_from_slice(chunk);
                    accepted(client.call(3, id, (index * 8) as i64, i64::from_le_bytes(word)))?;
                }
                accepted(client.call(4, id, 0, 0))?;
            }
            let id = self.id.ok_or(FAILED)?;
            let code = self.client.call(5, id, 0, 0);
            if code == 0 {
                return Ok(None);
            }
            if !matches!(code, 1 | 2) {
                return Err(if code < 0 { code } else { FAILED });
            }
            let mut bytes = Vec::new();
            if code == 2 {
                let length = self.client.call(6, id, 0, 0);
                let length = usize::try_from(length).map_err(|_| FAILED)?;
                if length > self.capacity {
                    return Err(FAILED);
                }
                bytes.reserve_exact(length);
                for offset in (0..length).step_by(8) {
                    let word = self.client.call(7, id, offset as i64, 0).to_le_bytes();
                    bytes.extend_from_slice(&word[..(length - offset).min(8)]);
                }
            }
            if self.client.closed.load(Ordering::Acquire) {
                return Err(UNAVAILABLE);
            }
            Ok(Some((code, bytes)))
        })();
        match result {
            Ok(None) => {
                let mut timer = Delay::new(Duration::from_millis(8));
                if Pin::new(&mut timer).poll(cx).is_ready() {
                    cx.waker().wake_by_ref();
                }
                self.timer = Some(timer);
                Poll::Pending
            }
            Ok(Some((code, bytes))) => {
                self.done = true;
                self.timer = None;
                Poll::Ready(Ok(Receipt {
                    client: self.client.clone(),
                    id: self.id.take(),
                    code,
                    bytes,
                }))
            }
            Err(code) => {
                self.done = true;
                self.cancel();
                Poll::Ready(Err(code))
            }
        }
    }
}
#[cfg(test)]
mod tests;
