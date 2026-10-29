//! Explicit safe-JNI connection to the application's bounded clipboard host.
use super::*;
use futures_timer::Delay;
use jni::{
    Env, JavaVM, jni_sig, jni_str,
    objects::{Global, JByteArray, JObject, JValue},
};
use std::{
    sync::atomic::{AtomicI64, Ordering},
    task::{Context, Poll},
};

struct Inner {
    vm: JavaVM,
    host: Global<JObject<'static>>,
    next: AtomicI64,
}
/// A protocol-v1 host, such as the mobile example's GloryClipboard Coordinator.
/// Java schedules actual ClipboardManager work on its main thread. The host
/// holds Activity weakly and must close on destruction; no JNI callback/export
/// or UI state crosses threads. Clone shares one host and non-wrapping IDs.
#[derive(Clone)]
pub struct AndroidClipboard(Arc<Inner>);
impl AndroidClipboard {
    pub fn new(env: &mut Env<'_>, host: JObject<'_>) -> Result<Self, ClipboardError> {
        if host.is_null() {
            return Err(ClipboardError::Unavailable);
        }
        let result = env
            .call_method(&host, jni_str!("protocolVersion"), jni_sig!(() -> i64), &[])
            .and_then(|v| v.j());
        if result.is_err() {
            env.exception_clear();
        }
        if result.map_err(|_| ClipboardError::Failed)? != 1 {
            return Err(ClipboardError::Failed);
        }
        let claimed = env
            .call_method(&host, jni_str!("claim"), jni_sig!(() -> bool), &[])
            .and_then(|v| v.z());
        if claimed.is_err() {
            env.exception_clear();
        }
        if !claimed.map_err(|_| ClipboardError::Failed)? {
            return Err(ClipboardError::Busy);
        }
        Ok(Self(Arc::new(Inner {
            vm: env.get_java_vm().map_err(|_| ClipboardError::Failed)?,
            host: env
                .new_global_ref(host)
                .map_err(|_| ClipboardError::Failed)?,
            next: AtomicI64::new(1),
        })))
    }
    /// Low 16 bits: current result records (0/1). Bit 16: physical admission.
    pub fn stats(&self) -> Result<u64, ClipboardError> {
        self.0
            .call(|env, host| {
                env.call_method(host, jni_str!("stats"), jni_sig!(() -> i64), &[])?
                    .j()
            })
            .and_then(|v| u64::try_from(v).map_err(|_| ClipboardError::Failed))
    }
}
impl Inner {
    fn call<T>(
        &self,
        f: impl FnOnce(&mut Env<'_>, &Global<JObject<'static>>) -> jni::errors::Result<T>,
    ) -> Result<T, ClipboardError> {
        self.vm
            .attach_current_thread(|env| {
                let result = f(env, &self.host);
                if result.is_err() {
                    env.exception_clear();
                }
                result
            })
            .map_err(|_| ClipboardError::Failed)
    }
    fn cancel(&self, id: i64) {
        let _ = self.call(|env, host| {
            env.call_method(
                host,
                jni_str!("cancel"),
                jni_sig!((i64) -> ()),
                &[JValue::Long(id)],
            )?;
            Ok(())
        });
    }
}
impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.call(|env, host| {
            env.call_method(host, jni_str!("close"), jni_sig!(() -> ()), &[])?;
            Ok(())
        });
    }
}
enum Command {
    Write(String),
    Read(usize),
}
struct Job {
    inner: Arc<Inner>,
    command: Option<Command>,
    id: Option<i64>,
    limit: usize,
    timer: Option<Delay>,
    done: bool,
}
fn code(value: i64) -> Result<(), ClipboardError> {
    match value {
        0..=2 => Ok(()),
        -1 => Err(ClipboardError::Unavailable),
        -2 => Err(ClipboardError::Denied),
        -4 => Err(ClipboardError::Busy),
        -5 => Err(ClipboardError::TooLong),
        -6 => Err(ClipboardError::InvalidLimits),
        _ => Err(ClipboardError::Failed),
    }
}
impl Future for Job {
    type Output = Result<Option<String>, ClipboardError>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.done {
            return Poll::Ready(Err(ClipboardError::Failed));
        }
        if let Some(timer) = &mut self.timer {
            if Pin::new(timer).poll(cx).is_pending() {
                return Poll::Pending;
            }
            self.timer = None;
        }
        let result = (|| {
            if let Some(command) = self.command.take() {
                let id = self
                    .inner
                    .next
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
                    .map_err(|_| ClipboardError::Failed)?;
                self.id = Some(id);
                let status = self.inner.call(|env, host| {
                    let bytes = match command {
                        Command::Write(text) => env.byte_array_from_slice(text.as_bytes())?,
                        Command::Read(_) => JByteArray::null(),
                    };
                    env.call_method(
                        host,
                        jni_str!("begin"),
                        jni_sig!((i64,[i8],i64) -> i64),
                        &[
                            JValue::Long(id),
                            JValue::Object(bytes.as_ref()),
                            JValue::Long(self.limit as i64),
                        ],
                    )?
                    .j()
                })?;
                code(status)?;
            }
            let id = self.id.ok_or(ClipboardError::Failed)?;
            let status = self.inner.call(|env, host| {
                env.call_method(
                    host,
                    jni_str!("poll"),
                    jni_sig!((i64) -> i64),
                    &[JValue::Long(id)],
                )?
                .j()
            })?;
            code(status)?;
            if status == 0 {
                return Ok(None);
            }
            if status == 1 {
                return Ok(Some(Ok(None)));
            }
            let result = self.inner.call(|env, host| {
                let object = env
                    .call_method(
                        host,
                        jni_str!("take"),
                        jni_sig!((i64) -> [i8]),
                        &[JValue::Long(id)],
                    )?
                    .l()?;
                let bytes = JByteArray::cast_local(env, object)?;
                if bytes.is_null() {
                    return Ok(Err(ClipboardError::Unavailable));
                }
                let length = bytes.len(env)?;
                if length > self.limit {
                    return Ok(Err(ClipboardError::TooLong));
                }
                Ok(Ok(env.convert_byte_array(&bytes)?))
            })??;
            let text = String::from_utf8(result).map_err(|_| ClipboardError::Failed)?;
            let canonical = glory_render::normalize_text(&text, true);
            if canonical.len() > self.limit {
                return Err(ClipboardError::TooLong);
            }
            Ok(Some(Ok(Some(canonical.into_owned()))))
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
            Ok(Some(result)) => {
                self.done = true;
                Poll::Ready(result)
            }
            Err(error) => {
                self.done = true;
                Poll::Ready(Err(error))
            }
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        if let Some(id) = self.id {
            self.inner.cancel(id);
        }
    }
}
impl AndroidClipboard {
    fn job(&self, command: Command) -> Job {
        let limit = match command {
            Command::Write(_) => glory_render::MAX_CLIPBOARD_BYTES,
            Command::Read(limit) => limit,
        };
        Job {
            inner: self.0.clone(),
            command: Some(command),
            id: None,
            limit,
            timer: None,
            done: false,
        }
    }
}
impl ClipboardDriver for AndroidClipboard {
    fn write(&self, text: String) -> Reply<()> {
        if text.len() > glory_render::MAX_CLIPBOARD_BYTES {
            return Box::pin(std::future::ready(Err(ClipboardError::TooLong)));
        }
        let job = self.job(Command::Write(text));
        Box::pin(async move {
            match job.await? {
                None => Ok(()),
                Some(_) => Err(ClipboardError::Failed),
            }
        })
    }
    fn read(&self, limit: usize) -> Reply<String> {
        if limit == 0 || limit > glory_render::MAX_CLIPBOARD_BYTES {
            return Box::pin(std::future::ready(Err(ClipboardError::InvalidLimits)));
        }
        let job = self.job(Command::Read(limit));
        Box::pin(async move { job.await?.ok_or(ClipboardError::Failed) })
    }
}
