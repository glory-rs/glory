//! Safe JNI connection to an app-owned protocol-v1 FilesLedger Host.
use super::{android_protocol as protocol, *};
use futures_timer::Delay;
use jni::{
    Env, JavaVM, jni_sig, jni_str,
    objects::{Global, JByteArray, JLongArray, JObject, JValue},
};
use std::{
    sync::atomic::{AtomicI64, Ordering},
    task::{Context, Poll},
};
struct Inner {
    vm: JavaVM,
    host: Global<JObject<'static>>,
    next: AtomicI64,
    submit: Mutex<()>,
}
/// Clone one driver/service to share monotonic request IDs. A Host may be
/// claimed once; it must belong to a live Activity session in the native host.
#[derive(Clone)]
pub struct AndroidFiles(Arc<Inner>);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AndroidFileStats {
    pub picker: usize,
    pub read_records: usize,
    pub handles: usize,
    pub jobs: usize,
    pub clients: usize,
    pub active_reads: usize,
    pub metadata_bytes: usize,
    pub declared_bytes: u64,
}
impl AndroidFiles {
    pub fn new(env: &mut Env<'_>, host: JObject<'_>) -> Result<Self, FileRejection> {
        if host.is_null() {
            return Err(FileRejection::Unavailable);
        }
        let version = env
            .call_method(&host, jni_str!("protocolVersion"), jni_sig!(() -> i64), &[])
            .and_then(|v| v.j());
        if version.is_err() {
            env.exception_clear();
        }
        if version.map_err(|_| FileRejection::Failed)? != 1 {
            return Err(FileRejection::Failed);
        }
        let claimed = env
            .call_method(&host, jni_str!("claim"), jni_sig!(() -> bool), &[])
            .and_then(|v| v.z());
        if claimed.is_err() {
            env.exception_clear();
        }
        if !claimed.map_err(|_| FileRejection::Failed)? {
            return Err(FileRejection::Busy);
        }
        Ok(Self(Arc::new(Inner {
            vm: env.get_java_vm().map_err(|_| FileRejection::Failed)?,
            host: env
                .new_global_ref(host)
                .map_err(|_| FileRejection::Failed)?,
            next: AtomicI64::new(1),
            submit: Mutex::new(()),
        })))
    }
    pub fn stats(&self) -> Result<AndroidFileStats, FileRejection> {
        self.0.call(|env, host| {
            let value = env
                .call_method(host, jni_str!("stats"), jni_sig!(() -> [i64]), &[])?
                .l()?;
            let array = JLongArray::cast_local(env, value)?;
            if array.is_null() || array.len(env)? != 8 {
                return Ok(Err(FileRejection::Failed));
            }
            let mut values = [0i64; 8];
            array.get_region(env, 0, &mut values)?;
            if values.iter().any(|v| *v < 0)
                || values[0] > 1
                || values[1] > 4
                || values[2] > 160
                || values[3] > 170
                || values[4] > 32
                || values[5] > 4
                || values[6] > 160 * 1024
                || values[7] > 2 * (1i64 << 30)
            {
                return Ok(Err(FileRejection::Failed));
            }
            Ok(Ok(AndroidFileStats {
                picker: values[0] as usize,
                read_records: values[1] as usize,
                handles: values[2] as usize,
                jobs: values[3] as usize,
                clients: values[4] as usize,
                active_reads: values[5] as usize,
                metadata_bytes: values[6] as usize,
                declared_bytes: values[7] as u64,
            }))
        })?
    }
}
impl Inner {
    fn call<T>(
        &self,
        f: impl FnOnce(&mut Env<'_>, &Global<JObject<'static>>) -> jni::errors::Result<T>,
    ) -> Result<T, FileRejection> {
        self.vm
            .attach_current_thread(|env| {
                let result = f(env, &self.host);
                if result.is_err() {
                    env.exception_clear();
                }
                result
            })
            .map_err(|_| FileRejection::Failed)
    }
    fn command(&self, method: &jni::strings::JNIStr, id: i64) {
        let _ = self.call(|env, host| {
            env.call_method(host, method, jni_sig!((i64) -> ()), &[JValue::Long(id)])?;
            Ok(())
        });
    }
    fn id(&self) -> Result<i64, FileRejection> {
        self.next
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            .map_err(|_| FileRejection::Failed)
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
struct Reader {
    inner: Arc<Inner>,
    file: i64,
}
impl Drop for Reader {
    fn drop(&mut self) {
        self.inner.command(jni_str!("release"), self.file);
    }
}
struct Pick {
    inner: Arc<Inner>,
    packet: Option<Vec<u8>>,
    id: Option<i64>,
    timer: Option<Delay>,
    done: bool,
}
fn timer(timer: &mut Option<Delay>, cx: &mut Context<'_>) -> bool {
    if let Some(timer) = timer
        && Pin::new(timer).poll(cx).is_pending()
    {
        return false;
    }
    *timer = None;
    true
}
fn wait(timer: &mut Option<Delay>, cx: &mut Context<'_>) {
    let mut delay = Delay::new(Duration::from_millis(8));
    if Pin::new(&mut delay).poll(cx).is_ready() {
        cx.waker().wake_by_ref();
    }
    *timer = Some(delay);
}
impl Future for Pick {
    type Output = Result<Option<Vec<SelectedFile>>, FileRejection>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.done {
            return Poll::Ready(Err(FileRejection::Failed));
        }
        if !timer(&mut self.timer, cx) {
            return Poll::Pending;
        }
        let result = (|| {
            if let Some(packet) = self.packet.take() {
                // Preserve native arrival order across concurrent callers while
                // keeping only a numeric watermark, never completed-ID history.
                let inner = self.inner.clone();
                let _submission = inner.submit.lock().map_err(|_| FileRejection::Failed)?;
                let id = inner.id()?;
                self.id = Some(id);
                let code = inner.call(|env, host| {
                    let bytes = env.byte_array_from_slice(&packet)?;
                    env.call_method(
                        host,
                        jni_str!("beginPick"),
                        jni_sig!((i64,[i8]) -> i64),
                        &[JValue::Long(id), JValue::Object(bytes.as_ref())],
                    )?
                    .j()
                })?;
                protocol::status(code)?;
            }
            let id = self.id.ok_or(FileRejection::Failed)?;
            let code = self.inner.call(|env, host| {
                env.call_method(
                    host,
                    jni_str!("pollPick"),
                    jni_sig!((i64) -> i64),
                    &[JValue::Long(id)],
                )?
                .j()
            })?;
            protocol::status(code)?;
            if code == 0 {
                return Ok(None);
            }
            if code == 1 {
                return Ok(Some(None));
            }
            let bytes = self.inner.call(|env, host| {
                let object = env
                    .call_method(
                        host,
                        jni_str!("metadata"),
                        jni_sig!((i64) -> [i8]),
                        &[JValue::Long(id)],
                    )?
                    .l()?;
                let bytes = JByteArray::cast_local(env, object)?;
                if bytes.is_null() || bytes.len(env)? > protocol::MAX_REPLY {
                    return Ok(Err(FileRejection::Metadata));
                }
                Ok(Ok(env.convert_byte_array(&bytes)?))
            })??;
            let metadata = protocol::decode(&bytes)?;
            let adopted = self.inner.call(|env, host| {
                env.call_method(
                    host,
                    jni_str!("adopt"),
                    jni_sig!((i64) -> bool),
                    &[JValue::Long(id)],
                )?
                .z()
            })?;
            if !adopted {
                return Err(FileRejection::Unavailable);
            }
            self.id = None;
            Ok(Some(Some(
                metadata
                    .into_iter()
                    .map(|file| SelectedFile {
                        name: file.name,
                        media_type: file.media_type,
                        size: file.size,
                        reader: Arc::new(Reader {
                            inner: self.inner.clone(),
                            file: file.id,
                        }),
                    })
                    .collect(),
            )))
        })();
        match result {
            Ok(None) => {
                wait(&mut self.timer, cx);
                Poll::Pending
            }
            Ok(Some(files)) => {
                self.done = true;
                if let Some(id) = self.id.take() {
                    self.inner.command(jni_str!("cancelPick"), id);
                }
                Poll::Ready(Ok(files))
            }
            Err(error) => {
                self.done = true;
                if let Some(id) = self.id.take() {
                    self.inner.command(jni_str!("cancelPick"), id);
                }
                Poll::Ready(Err(error))
            }
        }
    }
}
impl Drop for Pick {
    fn drop(&mut self) {
        if let Some(id) = self.id {
            self.inner.command(jni_str!("cancelPick"), id);
        }
    }
}
struct Read {
    inner: Arc<Inner>,
    file: i64,
    offset: u64,
    length: u32,
    id: Option<i64>,
    timer: Option<Delay>,
    done: bool,
}
impl Future for Read {
    type Output = Result<Vec<u8>, FileRejection>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.done {
            return Poll::Ready(Err(FileRejection::Failed));
        }
        if !timer(&mut self.timer, cx) {
            return Poll::Pending;
        }
        let result = (|| {
            if self.id.is_none() {
                // Preserve native arrival order across concurrent callers while
                // keeping only a numeric watermark, never completed-ID history.
                let inner = self.inner.clone();
                let _submission = inner.submit.lock().map_err(|_| FileRejection::Failed)?;
                let id = inner.id()?;
                self.id = Some(id);
                let code = inner.call(|env, host| {
                    env.call_method(
                        host,
                        jni_str!("beginRead"),
                        jni_sig!((i64,i64,i64,i64) -> i64),
                        &[
                            JValue::Long(id),
                            JValue::Long(self.file),
                            JValue::Long(self.offset as i64),
                            JValue::Long(i64::from(self.length)),
                        ],
                    )?
                    .j()
                })?;
                protocol::status(code)?;
            }
            let id = self.id.ok_or(FileRejection::Failed)?;
            let code = self.inner.call(|env, host| {
                env.call_method(
                    host,
                    jni_str!("pollRead"),
                    jni_sig!((i64) -> i64),
                    &[JValue::Long(id)],
                )?
                .j()
            })?;
            protocol::status(code)?;
            if code == 0 {
                return Ok(None);
            }
            if code != 2 {
                return Err(FileRejection::Failed);
            }
            self.inner.call(|env, host| {
                let object = env
                    .call_method(
                        host,
                        jni_str!("takeRead"),
                        jni_sig!((i64) -> [i8]),
                        &[JValue::Long(id)],
                    )?
                    .l()?;
                let bytes = JByteArray::cast_local(env, object)?;
                if bytes.is_null() || bytes.len(env)? != self.length as usize {
                    return Ok(Err(FileRejection::Bytes));
                }
                Ok(Ok(Some(env.convert_byte_array(&bytes)?)))
            })?
        })();
        match result {
            Ok(None) => {
                wait(&mut self.timer, cx);
                Poll::Pending
            }
            Ok(Some(bytes)) => {
                self.done = true;
                self.id = None; // takeRead already transferred/released the record.
                Poll::Ready(Ok(bytes))
            }
            Err(error) => {
                self.done = true;
                if let Some(id) = self.id.take() {
                    self.inner.command(jni_str!("cancelRead"), id);
                }
                Poll::Ready(Err(error))
            }
        }
    }
}
impl Drop for Read {
    fn drop(&mut self) {
        if let Some(id) = self.id {
            self.inner.command(jni_str!("cancelRead"), id);
        }
    }
}
impl FileReader for Reader {
    fn read(&self, offset: u64, length: u32) -> Reply<Vec<u8>> {
        if length > glory_render::MAX_FILE_CHUNK_BYTES
            || offset
                .checked_add(u64::from(length))
                .is_none_or(|end| end > glory_render::MAX_FILE_BYTES)
        {
            return Box::pin(std::future::ready(Err(FileRejection::Bytes)));
        }
        Box::pin(Read {
            inner: self.inner.clone(),
            file: self.file,
            offset,
            length,
            id: None,
            timer: None,
            done: false,
        })
    }
}
impl FilePicker for AndroidFiles {
    fn pick(&self, config: FileConfig) -> Reply<Option<Vec<SelectedFile>>> {
        match protocol::encode(&config) {
            Ok(packet) => Box::pin(Pick {
                inner: self.0.clone(),
                packet: Some(packet),
                id: None,
                timer: None,
                done: false,
            }),
            Err(error) => Box::pin(std::future::ready(Err(error))),
        }
    }
}
