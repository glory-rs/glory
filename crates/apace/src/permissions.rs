//! App-owned permission requests. Futures work on the existing ready-only Host;
//! dropping an Owner's future revokes delivery, not the operating system dialog.
use futures_timer::Delay;
use std::{
    cell::Cell,
    fmt,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
    time::{Duration, Instant},
};

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::AndroidPermissions;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Permission {
    Camera = 1,
    Microphone,
    PhotosRead,
    PhotosAdd,
    LocationWhenInUse,
    Notifications,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Status {
    Undetermined = 1,
    Granted,
    Denied,
    Restricted,
    Limited,
    Provisional,
    Ephemeral,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermissionState {
    pub status: Status,
    /// Android's current shouldShowRequestPermissionRationale value. False
    /// does not distinguish an initial request from a permanent refusal.
    pub rationale: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionError {
    Unsupported,
    Busy,
    Closed,
    MissingDeclaration,
    Unavailable,
    Backend,
    Exhausted,
    InvalidReply,
    InvalidLimits,
    Deadline,
}
impl fmt::Display for PermissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for PermissionError {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Action {
    Query = 0,
    Request = 1,
}

/// Explicit native host contract. begin/poll must not block for user input or
/// call business code. IDs belong to this driver instance. A cancelled active
/// OS prompt retains native admission until the actual native operation ends.
pub trait PermissionDriver {
    fn begin(&self, id: u64, kind: Permission, action: Action) -> Result<(), PermissionError>;
    fn poll(&self, id: u64) -> Result<Option<PermissionState>, PermissionError>;
    fn cancel(&self, id: u64);
    fn close(&self);
    fn stats(&self) -> Result<NativeStats, PermissionError> {
        Err(PermissionError::Unsupported)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeStats {
    pub records: usize,
    pub active_prompts: usize,
    pub active_queries: usize,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PermissionStats {
    pub waiting: usize,
    pub peak_waiting: usize,
    pub closed: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct PermissionLimits {
    pub pending: usize,
    pub timeout: Duration,
    pub poll_interval: Duration,
}
impl Default for PermissionLimits {
    fn default() -> Self {
        Self {
            pending: 8,
            timeout: Duration::from_secs(120),
            poll_interval: Duration::from_millis(20),
        }
    }
}
struct Inner {
    driver: Box<dyn PermissionDriver>,
    limits: PermissionLimits,
    next: Cell<u64>,
    waiting: Cell<usize>,
    peak: Cell<usize>,
    closed: Cell<bool>,
}
impl Inner {
    fn close(&self) {
        if !self.closed.replace(true) {
            self.driver.close();
        }
    }
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.close();
    }
}

/// Clone within one app/host; create another service for an independent host.
/// This is local (!Send/!Sync). It does not retain a reactive Owner or AndroidApp.
#[derive(Clone)]
pub struct Permissions(Rc<Inner>);
impl Permissions {
    pub fn new(
        driver: impl PermissionDriver + 'static,
        limits: PermissionLimits,
    ) -> Result<Self, PermissionError> {
        if !(1..=32).contains(&limits.pending)
            || !(Duration::from_millis(20)..=Duration::from_secs(600)).contains(&limits.timeout)
            || !(Duration::from_millis(5)..=Duration::from_secs(1)).contains(&limits.poll_interval)
        {
            driver.close();
            return Err(PermissionError::InvalidLimits);
        }
        Ok(Self(Rc::new(Inner {
            driver: Box::new(driver),
            limits,
            next: Cell::new(1),
            waiting: Cell::new(0),
            peak: Cell::new(0),
            closed: Cell::new(false),
        })))
    }
    /// Creating a future does not ask the OS. Its first poll starts the query.
    pub fn query(&self, kind: Permission) -> PermissionFuture {
        self.future(kind, Action::Query)
    }
    /// Call from the application's user action, after explaining its purpose.
    /// No rationale dialog, settings launch or permission grant is synthesized.
    pub fn request(&self, kind: Permission) -> PermissionFuture {
        self.future(kind, Action::Request)
    }
    fn future(&self, kind: Permission, action: Action) -> PermissionFuture {
        PermissionFuture {
            inner: self.0.clone(),
            kind,
            action,
            id: None,
            deadline: None,
            delay: None,
            finished: false,
        }
    }
    pub fn close(&self) {
        self.0.close();
    }
    pub fn stats(&self) -> PermissionStats {
        PermissionStats {
            waiting: self.0.waiting.get(),
            peak_waiting: self.0.peak.get(),
            closed: self.0.closed.get(),
        }
    }
    pub fn native_stats(&self) -> Result<NativeStats, PermissionError> {
        self.0.driver.stats()
    }
}
pub struct PermissionFuture {
    inner: Rc<Inner>,
    kind: Permission,
    action: Action,
    id: Option<u64>,
    deadline: Option<Instant>,
    delay: Option<Delay>,
    finished: bool,
}
impl PermissionFuture {
    fn release(&mut self, cancel: bool) {
        if let Some(id) = self.id.take() {
            self.inner.waiting.set(self.inner.waiting.get() - 1);
            if cancel {
                self.inner.driver.cancel(id);
            }
        }
        self.delay = None;
        self.finished = true;
    }
}
impl Future for PermissionFuture {
    type Output = Result<PermissionState, PermissionError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        assert!(!this.finished, "permission future already completed");
        if this.inner.closed.get() {
            this.release(true);
            return Poll::Ready(Err(PermissionError::Closed));
        }
        if this.id.is_none() {
            if this.inner.waiting.get() >= this.inner.limits.pending {
                this.release(false);
                return Poll::Ready(Err(PermissionError::Busy));
            }
            let id = this.inner.next.get();
            if id > i64::MAX as u64 {
                this.release(false);
                return Poll::Ready(Err(PermissionError::Exhausted));
            }
            this.inner.next.set(id + 1);
            this.id = Some(id);
            let waiting = this.inner.waiting.get() + 1;
            this.inner.waiting.set(waiting);
            this.inner.peak.set(this.inner.peak.get().max(waiting));
            if let Err(error) = this.inner.driver.begin(id, this.kind, this.action) {
                this.release(true);
                return Poll::Ready(Err(error));
            }
            if this.inner.closed.get() {
                this.release(true);
                return Poll::Ready(Err(PermissionError::Closed));
            }
            this.deadline = Some(Instant::now() + this.inner.limits.timeout);
        }
        if Instant::now() >= this.deadline.expect("accepted deadline") {
            this.release(true);
            return Poll::Ready(Err(PermissionError::Deadline));
        }
        if let Some(delay) = this.delay.as_mut()
            && Pin::new(delay).poll(cx).is_pending()
        {
            return Poll::Pending;
        }
        let reply = this.inner.driver.poll(this.id.expect("accepted request"));
        if this.inner.closed.get() {
            this.release(true);
            return Poll::Ready(Err(PermissionError::Closed));
        }
        if Instant::now() >= this.deadline.expect("accepted deadline") {
            this.release(true);
            return Poll::Ready(Err(PermissionError::Deadline));
        }
        match reply {
            Ok(Some(state)) => {
                this.release(false);
                Poll::Ready(Ok(state))
            }
            Err(error) => {
                this.release(true);
                Poll::Ready(Err(error))
            }
            Ok(None) => {
                let remaining = this
                    .deadline
                    .expect("accepted deadline")
                    .saturating_duration_since(Instant::now());
                let mut delay = Delay::new(remaining.min(this.inner.limits.poll_interval));
                if Pin::new(&mut delay).poll(cx).is_ready() {
                    cx.waker().wake_by_ref();
                }
                this.delay = Some(delay);
                Poll::Pending
            }
        }
    }
}

#[cfg(test)]
#[path = "permissions/tests.rs"]
mod tests;
impl Drop for PermissionFuture {
    fn drop(&mut self) {
        self.release(true);
    }
}

/// Scalar bridge protocol shared by application-owned Java/Swift hosts. Native
/// functions accept integers only and validate ranges; no pointer crosses it.
pub mod bridge {
    use super::*;
    pub fn error(code: i64) -> PermissionError {
        match code {
            -1 => PermissionError::Unsupported,
            -2 => PermissionError::Busy,
            -3 => PermissionError::Closed,
            -4 => PermissionError::MissingDeclaration,
            -5 => PermissionError::Unavailable,
            -6 => PermissionError::Backend,
            -7 => PermissionError::Exhausted,
            _ => PermissionError::InvalidReply,
        }
    }
    pub fn reply(code: i64) -> Result<Option<PermissionState>, PermissionError> {
        if code < 0 {
            return Err(error(code));
        }
        if code == 0 {
            return Ok(None);
        }
        if code & !0x127 != 0 || code & 0x100 == 0 {
            return Err(PermissionError::InvalidReply);
        }
        let status = match code & 7 {
            1 => Status::Undetermined,
            2 => Status::Granted,
            3 => Status::Denied,
            4 => Status::Restricted,
            5 => Status::Limited,
            6 => Status::Provisional,
            7 => Status::Ephemeral,
            _ => return Err(PermissionError::InvalidReply),
        };
        Ok(Some(PermissionState {
            status,
            rationale: code & 0x20 != 0,
        }))
    }
    pub fn accepted(code: i64) -> Result<(), PermissionError> {
        if code == 0 { Ok(()) } else { Err(error(code)) }
    }
    pub fn stats(code: i64) -> Result<NativeStats, PermissionError> {
        if code < 0 {
            return Err(error(code));
        }
        if code & !0x7fffff != 0 || code & 0xffff > 32 || ((code >> 17) & 63) > 32 {
            return Err(PermissionError::InvalidReply);
        }
        Ok(NativeStats {
            records: (code & 0xffff) as usize,
            active_prompts: ((code >> 16) & 1) as usize,
            active_queries: ((code >> 17) & 63) as usize,
        })
    }
}
