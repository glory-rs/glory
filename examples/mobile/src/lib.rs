//! Mobile application host; the component remains shared with Web/desktop.
use glory::blitz::{
    Config,
    assets::{Bytes, LoadConfig, Loader, Url},
    shell::WindowApp,
};
use glory::prelude::{group, image};
use std::{collections::BTreeMap, sync::Arc};

#[cfg(all(feature = "android-assets-probe", target_os = "android"))]
mod android_assets_probe;
#[cfg(feature = "network-probe")]
mod network_probe;
#[cfg(feature = "permissions-probe")]
mod permissions_probe;
#[cfg(all(feature = "permissions-probe", not(target_os = "android")))]
compile_error!(
    "permissions-probe currently targets Android; permissions-host-probe checks the Swift iOS host"
);
#[cfg(all(feature = "permissions-host-probe", not(target_os = "ios")))]
compile_error!("permissions-host-probe runs the Swift iOS host; Android uses permissions-probe");
#[cfg(all(
    feature = "permissions-probe",
    any(feature = "ime-probe", feature = "keyboard-probe")
))]
compile_error!("select one mobile acceptance view");
#[cfg(all(
    target_os = "android",
    any(feature = "network", feature = "permissions"),
    not(feature = "android-game-activity")
))]
compile_error!(
    "mobile JNI services require the example's GameActivity host; select --no-default-features --features android-game-activity"
);
#[cfg(feature = "ime-probe")]
mod ime_probe;
#[cfg(feature = "keyboard-probe")]
mod keyboard_probe;
#[cfg(all(feature = "ime-probe", feature = "keyboard-probe"))]
compile_error!("select one mobile acceptance view");

/// One package worker per document, including every recreated Android Activity.
pub fn counter_window() -> std::result::Result<(WindowApp, Arc<Loader>), Box<dyn std::error::Error>>
{
    asset_window(
        LoadConfig {
            package: BTreeMap::from([
                (
                    "/theme.css".into(),
                    Bytes::from_static(include_bytes!("../assets/theme.css")),
                ),
                (
                    "/badge.png".into(),
                    Bytes::from_static(include_bytes!("../assets/badge.png")),
                ),
            ]),
            ..Default::default()
        },
        "asset://app/",
    )
}

fn asset_window(
    config: LoadConfig,
    base: &str,
) -> std::result::Result<(WindowApp, Arc<Loader>), Box<dyn std::error::Error>> {
    let loader = Arc::new(Loader::new(config)?);
    let window = WindowApp::with_config(Config {
        base_url: Url::parse(base)?,
        provider: Some(loader.clone()),
        ..Default::default()
    });
    window.renderer().set_css("@import url('theme.css');");
    #[cfg(not(any(
        feature = "ime-probe",
        feature = "keyboard-probe",
        feature = "permissions-probe"
    )))]
    let content = counter::Counter;
    #[cfg(feature = "ime-probe")]
    let content = ime_probe::fixture();
    #[cfg(feature = "keyboard-probe")]
    let content = keyboard_probe::fixture();
    #[cfg(all(feature = "permissions-probe", target_os = "android"))]
    let content = permissions_probe::fixture(android_permissions()?);
    window.mount(
        group().fill((
            content,
            image()
                .class("package-badge")
                .source("badge.png")
                .decorative(),
        )),
    )?;
    Ok((window, loader))
}

#[cfg(target_os = "android")]
// Explicitly authorized exception: the application ABI entry needs this
// symbol. Runtime crates and the body remain safe Rust.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub fn android_main(app: glory::blitz::shell::AndroidApp) {
    if let Err(error) = run_android(app) {
        eprintln!("Glory mobile: {error}");
    }
}

#[cfg(target_os = "android")]
fn run_android(app: glory::blitz::shell::AndroidApp) -> Result<(), Box<dyn std::error::Error>> {
    use glory::blitz::shell::{Shell, WindowAttributes};
    #[cfg(feature = "network")]
    let tls = android_tls()?;
    #[cfg(all(
        feature = "android-assets",
        any(not(feature = "network-probe"), feature = "android-assets-probe")
    ))]
    let assets = glory::blitz::assets::AndroidAssets::new(app.asset_manager(), "glory")?;
    #[cfg(feature = "android-assets-probe")]
    android_assets_probe::run(assets.clone())?;
    eprintln!("Glory mobile: creating Activity event loop");
    let mut shell = Shell::android(app)?;
    eprintln!("Glory mobile: creating semantic document");
    #[cfg(all(not(feature = "android-assets"), not(feature = "network-probe")))]
    let (window, loader) = counter_window()?;
    #[cfg(all(feature = "android-assets", not(feature = "network-probe")))]
    let (window, loader) = {
        // No embedded fallback: CSS and image acceptance must traverse the
        // application AssetManager and the same bounded Loader pipeline.
        eprintln!("Glory mobile: APK AssetManager prefix=glory, embedded entries=0");
        asset_window(
            LoadConfig {
                android: Some(assets),
                ..Default::default()
            },
            "asset://android/",
        )?
    };
    #[cfg(feature = "network-probe")]
    let (window, loader) = network_window(Some(tls))?;
    #[cfg(all(feature = "network", not(feature = "network-probe")))]
    let _ = tls;
    eprintln!("Glory mobile: entering native window loop");
    shell.add_window(
        window,
        WindowAttributes::default().with_title("Glory Counter"),
    );
    shell.run()?;
    eprintln!("Glory mobile assets: {:?}", loader.stats());
    Ok(())
}

#[cfg(all(target_os = "android", feature = "network"))]
fn android_tls() -> Result<glory::blitz::assets::AndroidTls, Box<dyn std::error::Error>> {
    #[cfg(feature = "network-probe")]
    use jni::objects::JObject;
    use jni::{JavaVM, jni_sig, jni_str};
    // android-activity initializes this same jni version and the Rust thread's
    // application class loader before calling android_main. No raw VM/Activity
    // pointer is reconstructed here.
    JavaVM::singleton()?.attach_current_thread(|env| -> Result<_, Box<dyn std::error::Error>> {
        #[cfg(feature = "network-probe")]
        {
            assert!(glory::blitz::assets::AndroidTls::new(env, JObject::null()).is_err());
            let wrong = env.new_string("not an Application")?;
            assert!(glory::blitz::assets::AndroidTls::new(env, wrong.into()).is_err());
            assert_eq!(
                Loader::new(LoadConfig {
                    origins: vec![Url::parse("https://example.com/")?],
                    ..Default::default()
                })
                .err()
                .expect("explicit TLS capability required")
                .kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
        let class = env.load_class(jni_str!("rs.glory.counter.GloryGameActivity"))?;
        let context = env
            .call_static_method(
                class,
                jni_str!("gloryApplicationContext"),
                jni_sig!(() -> JObject),
                &[],
            )?
            .l()?;
        Ok(glory::blitz::assets::AndroidTls::new(env, context)?)
    })
}

#[cfg(all(feature = "permissions-probe", target_os = "android"))]
fn android_permissions()
-> Result<glory::blitz::permissions::Permissions, Box<dyn std::error::Error>> {
    use glory::blitz::permissions::{AndroidPermissions, PermissionLimits, Permissions};
    use jni::{JavaVM, jni_sig, jni_str};
    JavaVM::singleton()?.attach_current_thread(|env| -> Result<_, Box<dyn std::error::Error>> {
        let class = env.load_class(jni_str!("rs.glory.counter.GloryGameActivity"))?;
        let host = env
            .call_static_method(
                class,
                jni_str!("gloryPermissionHost"),
                jni_sig!(() -> JObject),
                &[],
            )?
            .l()?;
        Ok(Permissions::new(
            AndroidPermissions::new(env, host)?,
            PermissionLimits::default(),
        )?)
    })
}

#[cfg(feature = "network-probe")]
pub fn network_window(
    #[cfg(target_os = "android")] tls: Option<glory::blitz::assets::AndroidTls>,
) -> std::result::Result<(WindowApp, Arc<Loader>), Box<dyn std::error::Error>> {
    let origin = option_env!("GLORY_MOBILE_NETWORK_ORIGIN")
        .ok_or("network-probe needs GLORY_MOBILE_NETWORK_ORIGIN at build time")?;
    let https = option_env!("GLORY_MOBILE_TLS_ORIGIN").unwrap_or("https://example.com/");
    let reject = option_env!("GLORY_MOBILE_REJECT_TLS_ORIGIN")
        .ok_or("network-probe needs GLORY_MOBILE_REJECT_TLS_ORIGIN at build time")?;
    let config = LoadConfig {
        origins: vec![Url::parse(origin)?, Url::parse(https)?, Url::parse(reject)?],
        #[cfg(target_os = "android")]
        android_tls: tls,
        ..Default::default()
    };
    network_probe::spawn(
        config.clone(),
        origin.to_owned(),
        https.to_owned(),
        reject.to_owned(),
    );
    eprintln!("Glory mobile: HTTP assets origin={origin}, embedded entries=0");
    asset_window(config, origin)
}
