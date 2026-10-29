use super::*;
use jni::{
    Env, JavaVM, jni_sig, jni_str,
    objects::{Global, JObject, JValue},
};

/// A single application-owned Java permission host. It must implement the
/// scalar v1 contract used by examples/mobile's GloryPermissions.Host. That
/// host holds the Activity weakly and closes when its Activity is destroyed.
pub struct AndroidPermissions {
    vm: JavaVM,
    host: Global<JObject<'static>>,
    closed: Cell<bool>,
}
impl AndroidPermissions {
    pub fn new(env: &mut Env<'_>, host: JObject<'_>) -> Result<Self, PermissionError> {
        if host.is_null() {
            return Err(PermissionError::Unavailable);
        }
        let version = env
            .call_method(&host, jni_str!("protocolVersion"), jni_sig!(() -> i64), &[])
            .and_then(|value| value.j())
            .map_err(|_| PermissionError::Backend)?;
        if version != 1 {
            return Err(PermissionError::InvalidReply);
        }
        Ok(Self {
            vm: env.get_java_vm().map_err(|_| PermissionError::Backend)?,
            host: env
                .new_global_ref(host)
                .map_err(|_| PermissionError::Backend)?,
            closed: Cell::new(false),
        })
    }
    fn read(&self, id: Option<i64>) -> Result<i64, PermissionError> {
        self.vm
            .attach_current_thread(|env| -> jni::errors::Result<i64> {
                let value = if let Some(id) = id {
                    env.call_method(
                        &self.host,
                        jni_str!("poll"),
                        jni_sig!((i64) -> i64),
                        &[JValue::Long(id)],
                    )?
                } else {
                    env.call_method(&self.host, jni_str!("stats"), jni_sig!(() -> i64), &[])?
                };
                value.j()
            })
            .map_err(|_| PermissionError::Backend)
    }
}
impl PermissionDriver for AndroidPermissions {
    fn begin(&self, id: u64, kind: Permission, action: Action) -> Result<(), PermissionError> {
        if self.closed.get() {
            return Err(PermissionError::Closed);
        }
        let id = i64::try_from(id).map_err(|_| PermissionError::Exhausted)?;
        let code = self
            .vm
            .attach_current_thread(|env| -> jni::errors::Result<i64> {
                env.call_method(
                    &self.host,
                    jni_str!("begin"),
                    jni_sig!((i64, i64, i64) -> i64),
                    &[
                        JValue::Long(id),
                        JValue::Long(kind as i64),
                        JValue::Long(action as i64),
                    ],
                )?
                .j()
            })
            .map_err(|_| PermissionError::Backend)?;
        bridge::accepted(code)
    }
    fn poll(&self, id: u64) -> Result<Option<PermissionState>, PermissionError> {
        if self.closed.get() {
            return Err(PermissionError::Closed);
        }
        bridge::reply(self.read(Some(
            i64::try_from(id).map_err(|_| PermissionError::Exhausted)?,
        ))?)
    }
    fn cancel(&self, id: u64) {
        if self.closed.get() {
            return;
        }
        if let Ok(id) = i64::try_from(id) {
            let _ = self
                .vm
                .attach_current_thread(|env| -> jni::errors::Result<()> {
                    env.call_method(
                        &self.host,
                        jni_str!("cancel"),
                        jni_sig!((i64) -> ()),
                        &[JValue::Long(id)],
                    )?;
                    Ok(())
                });
        }
    }
    fn close(&self) {
        if self.closed.replace(true) {
            return;
        }
        let _ = self
            .vm
            .attach_current_thread(|env| -> jni::errors::Result<()> {
                env.call_method(&self.host, jni_str!("close"), jni_sig!(() -> ()), &[])?;
                Ok(())
            });
    }
    fn stats(&self) -> Result<NativeStats, PermissionError> {
        if self.closed.get() {
            return Err(PermissionError::Closed);
        }
        bridge::stats(self.read(None)?)
    }
}
impl Drop for AndroidPermissions {
    fn drop(&mut self) {
        self.close();
    }
}
