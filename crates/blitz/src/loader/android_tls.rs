//! Safe, explicit initialization of the platform verifier's application context.
use jni::{
    Env, jni_sig, jni_str,
    objects::{JClassLoader, JObject},
};
use std::io;

/// A configured Android platform certificate verifier. This contains no Activity
/// or UI state. The verifier library keeps the process's Application context.
#[derive(Clone, Debug)]
pub struct AndroidTls {
    _ready: (),
}
impl AndroidTls {
    /// Pass an application-owned JNI Application object (not an Activity).
    /// Include the matching rustls-platform-verifier Android AAR in the APK.
    /// No raw-pointer conversion or global AndroidApp is used by this bridge.
    pub fn new(env: &mut Env<'_>, application: JObject<'_>) -> io::Result<Self> {
        fn initialize(env: &mut Env<'_>, application: JObject<'_>) -> jni::errors::Result<()> {
            if application.is_null()
                || !env.is_instance_of(&application, jni_str!("android/app/Application"))?
            {
                return Err(jni::errors::Error::NullPtr(
                    "expected Android Application context",
                ));
            }
            // Fail at construction if the app omitted the matching Java code,
            // instead of deferring a missing-class error to the first TLS request.
            let loader = env
                .call_method(
                    &application,
                    jni_str!("getClassLoader"),
                    jni_sig!(() -> JClassLoader),
                    &[],
                )?
                .l()?;
            let loader = env.cast_local::<JClassLoader>(loader)?;
            let name = env.new_string("org.rustls.platformverifier.CertificateVerifier")?;
            loader.load_class(env, name)?;
            rustls_platform_verifier::android::init_with_env(env, application)
        }
        initialize(env, application)
            .map_err(|error| io::Error::other(format!("Android TLS initialization: {error}")))?;
        Ok(Self { _ready: () })
    }
}
