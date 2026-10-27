//! Mobile application host; the component remains shared with Web/desktop.
use glory::blitz::{
    Config,
    assets::{Bytes, LoadConfig, Loader, Url},
    shell::WindowApp,
};
use glory::prelude::{group, image};
use std::{collections::BTreeMap, sync::Arc};
#[cfg(all(feature = "files-probe", target_os = "android"))]
mod files_probe;
#[cfg(all(feature = "files-probe", not(target_os = "android")))]
compile_error!("files-probe currently requires the Android system host");
#[cfg(all(
    feature = "files-probe",
    any(
        feature = "clipboard-probe",
        feature = "network-probe",
        feature = "permissions-probe",
        feature = "permissions-host-probe",
        feature = "android-assets-probe",
        feature = "ime-probe",
        feature = "keyboard-probe",
        feature = "controls-probe",
        feature = "radio-probe",
        feature = "placeholder-probe",
        feature = "choice-probe",
        feature = "select-probe",
        feature = "menu-probe",
        feature = "tags-probe"
    )
))]
compile_error!("select one mobile acceptance view");
#[cfg(all(feature = "clipboard-probe", target_os = "android"))]
mod clipboard_probe;
#[cfg(all(feature = "clipboard-probe", not(target_os = "android")))]
compile_error!("clipboard-probe currently requires the Android system host");
#[cfg(all(
    feature = "clipboard-probe",
    any(
        feature = "network-probe",
        feature = "permissions-probe",
        feature = "permissions-host-probe",
        feature = "android-assets-probe",
        feature = "ime-probe",
        feature = "keyboard-probe",
        feature = "controls-probe",
        feature = "radio-probe",
        feature = "placeholder-probe",
        feature = "choice-probe",
        feature = "select-probe",
        feature = "menu-probe",
        feature = "tags-probe"
    )
))]
compile_error!("select one mobile acceptance view");

#[cfg(all(feature = "android-assets-probe", target_os = "android"))]
mod android_assets_probe;
#[cfg(feature = "choice-probe")]
mod choice_probe;
#[cfg(feature = "tags-probe")]
mod tags_probe;
#[cfg(all(
    feature = "tags-probe",
    any(
        feature = "menu-probe",
        feature = "select-probe",
        feature = "choice-probe",
        feature = "placeholder-probe",
        feature = "radio-probe",
        feature = "controls-probe",
        feature = "ime-probe",
        feature = "keyboard-probe",
        feature = "permissions-probe"
    )
))]
compile_error!("select one mobile acceptance view");
#[cfg(feature = "menu-probe")]
mod menu_probe;
#[cfg(feature = "select-probe")]
mod select_probe;
#[cfg(all(
    feature = "menu-probe",
    any(
        feature = "select-probe",
        feature = "choice-probe",
        feature = "placeholder-probe",
        feature = "radio-probe",
        feature = "controls-probe",
        feature = "ime-probe",
        feature = "keyboard-probe",
        feature = "permissions-probe"
    )
))]
compile_error!("select one mobile acceptance view");
#[cfg(all(
    feature = "select-probe",
    any(
        feature = "choice-probe",
        feature = "placeholder-probe",
        feature = "radio-probe",
        feature = "controls-probe",
        feature = "ime-probe",
        feature = "keyboard-probe",
        feature = "permissions-probe"
    )
))]
compile_error!("select one mobile acceptance view");
#[cfg(feature = "controls-probe")]
mod controls_probe;
#[cfg(all(
    feature = "choice-probe",
    any(
        feature = "placeholder-probe",
        feature = "radio-probe",
        feature = "controls-probe",
        feature = "ime-probe",
        feature = "keyboard-probe",
        feature = "permissions-probe"
    )
))]
compile_error!("select one mobile acceptance view");
#[cfg(feature = "placeholder-probe")]
mod placeholder_probe;
#[cfg(feature = "radio-probe")]
mod radio_probe;
#[cfg(all(
    feature = "placeholder-probe",
    any(
        feature = "radio-probe",
        feature = "controls-probe",
        feature = "ime-probe",
        feature = "keyboard-probe",
        feature = "permissions-probe"
    )
))]
compile_error!("select one mobile acceptance view");
#[cfg(all(
    feature = "radio-probe",
    any(
        feature = "controls-probe",
        feature = "ime-probe",
        feature = "keyboard-probe",
        feature = "permissions-probe"
    )
))]
compile_error!("select one mobile acceptance view");
#[cfg(all(
    any(feature = "clipboard", feature = "files"),
    any(target_os = "ios", test)
))]
pub mod ios_io;
#[cfg(all(feature = "permissions", any(target_os = "ios", test)))]
pub mod ios_permissions;
#[cfg(feature = "network-probe")]
mod network_probe;
#[cfg(feature = "permissions-probe")]
mod permissions_probe;
#[cfg(all(
    feature = "controls-probe",
    any(
        feature = "ime-probe",
        feature = "keyboard-probe",
        feature = "permissions-probe"
    )
))]
compile_error!("select one mobile acceptance view");
#[cfg(all(
    feature = "permissions-probe",
    not(any(target_os = "android", target_os = "ios"))
))]
compile_error!("permissions-probe requires the Android or iOS application host");
#[cfg(all(feature = "permissions-probe", feature = "permissions-host-probe"))]
compile_error!("select the Rust permission view or the isolated Swift host probe");
#[cfg(all(feature = "permissions-host-probe", not(target_os = "ios")))]
compile_error!("permissions-host-probe runs the Swift iOS host; Android uses permissions-probe");
#[cfg(all(
    feature = "permissions-probe",
    any(feature = "ime-probe", feature = "keyboard-probe")
))]
compile_error!("select one mobile acceptance view");
#[cfg(all(
    target_os = "android",
    any(
        feature = "network",
        feature = "permissions",
        feature = "clipboard",
        feature = "files"
    ),
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

#[cfg(all(feature = "ios-io-probe", not(target_os = "ios")))]
compile_error!("ios-io-probe requires the UIKit application host");
#[cfg(all(
    feature = "ios-io-probe",
    any(
        feature = "files-probe",
        feature = "clipboard-probe",
        feature = "network-probe",
        feature = "permissions-probe",
        feature = "permissions-host-probe",
        feature = "android-assets-probe",
        feature = "ime-probe",
        feature = "keyboard-probe",
        feature = "controls-probe",
        feature = "radio-probe",
        feature = "placeholder-probe",
        feature = "choice-probe",
        feature = "select-probe",
        feature = "menu-probe",
        feature = "tags-probe"
    )
))]
compile_error!("select one mobile acceptance view");

#[cfg(all(feature = "ios-files-ui-probe", not(target_os = "ios")))]
compile_error!("ios-files-ui-probe requires UIKit");
#[cfg(all(
    feature = "ios-files-ui-probe",
    any(
        feature = "ios-io-probe",
        feature = "files-probe",
        feature = "clipboard-probe",
        feature = "permissions-probe",
        feature = "permissions-host-probe",
        feature = "network-probe",
        feature = "ime-probe",
        feature = "keyboard-probe",
        feature = "controls-probe",
        feature = "choice-probe",
        feature = "radio-probe",
        feature = "placeholder-probe",
        feature = "select-probe",
        feature = "menu-probe",
        feature = "tags-probe",
        feature = "android-assets-probe"
    )
))]
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
    #[cfg(all(feature = "clipboard", target_os = "android"))]
    let clipboard = android_clipboard()?;
    #[cfg(all(feature = "files", target_os = "android"))]
    let file_driver = android_files()?;
    #[cfg(all(feature = "clipboard", target_os = "ios"))]
    let clipboard = ios_io::clipboard()?;
    #[cfg(all(feature = "files", target_os = "ios"))]
    let file_driver = ios_io::files()
        .map_err(|error| std::io::Error::other(format!("iOS file host: {error:?}")))?;
    #[cfg(all(feature = "files", any(target_os = "android", target_os = "ios")))]
    let files = glory::blitz::files::Files::new(file_driver.clone());
    let window = WindowApp::with_config(Config {
        #[cfg(all(feature = "clipboard", any(target_os = "android", target_os = "ios")))]
        clipboard: Some(glory::blitz::clipboard::Clipboard::new(clipboard.clone())),
        #[cfg(all(feature = "files", any(target_os = "android", target_os = "ios")))]
        files: Some(files.clone()),
        base_url: Url::parse(base)?,
        provider: Some(loader.clone()),
        ..Default::default()
    });
    window.renderer().set_css("@import url('theme.css');");
    #[cfg(feature = "select-probe")]
    window
        .renderer()
        .set_css(&format!("@import url('theme.css');{}", select_probe::CSS));
    #[cfg(feature = "menu-probe")]
    window
        .renderer()
        .set_css(&format!("@import url('theme.css');{}", menu_probe::CSS));
    #[cfg(feature = "tags-probe")]
    window
        .renderer()
        .set_css(&format!("@import url('theme.css');{}", tags_probe::CSS));
    #[cfg(not(any(
        feature = "ime-probe",
        feature = "keyboard-probe",
        feature = "permissions-probe",
        feature = "controls-probe",
        feature = "radio-probe",
        feature = "placeholder-probe",
        feature = "choice-probe",
        feature = "select-probe",
        feature = "menu-probe",
        feature = "tags-probe",
        feature = "clipboard-probe",
        feature = "files-probe",
        feature = "ios-io-probe",
        feature = "ios-files-ui-probe"
    )))]
    let content = counter::Counter;
    #[cfg(feature = "ime-probe")]
    let content = ime_probe::fixture();
    #[cfg(feature = "keyboard-probe")]
    let content = keyboard_probe::fixture();
    #[cfg(all(feature = "permissions-probe", target_os = "android"))]
    let content = permissions_probe::fixture(android_permissions()?);
    #[cfg(all(feature = "permissions-probe", target_os = "ios"))]
    let content = permissions_probe::fixture(ios_permissions::service()?);
    #[cfg(feature = "controls-probe")]
    let content = controls_probe::fixture();
    #[cfg(feature = "radio-probe")]
    let content = radio_probe::fixture();
    #[cfg(feature = "placeholder-probe")]
    let content = placeholder_probe::fixture();
    #[cfg(feature = "choice-probe")]
    let content = choice_probe::fixture();
    #[cfg(feature = "select-probe")]
    let content = select_probe::fixture();
    #[cfg(feature = "menu-probe")]
    let content = menu_probe::fixture();
    #[cfg(feature = "tags-probe")]
    let content = tags_probe::fixture();
    #[cfg(all(feature = "clipboard-probe", target_os = "android"))]
    let content = clipboard_probe::fixture(clipboard);
    #[cfg(all(feature = "files-probe", target_os = "android"))]
    let content = files_probe::fixture(file_driver, files, window.renderer().clone());
    #[cfg(all(feature = "ios-io-probe", target_os = "ios"))]
    let content = ios_io::probe::fixture(clipboard, file_driver, files, window.renderer().clone());
    #[cfg(all(feature = "ios-files-ui-probe", target_os = "ios"))]
    let content =
        ios_io::system_files_probe::fixture(file_driver, files, window.renderer().clone());
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

#[cfg(all(feature = "clipboard", target_os = "android"))]
fn android_clipboard()
-> Result<Arc<glory::blitz::clipboard::AndroidClipboard>, Box<dyn std::error::Error>> {
    use jni::{JavaVM, jni_sig, jni_str};
    JavaVM::singleton()?.attach_current_thread(|env| -> Result<_, Box<dyn std::error::Error>> {
        let class = env.load_class(jni_str!("rs.glory.counter.GloryGameActivity"))?;
        let host = env
            .call_static_method(
                class,
                jni_str!("gloryClipboardHost"),
                jni_sig!(() -> JObject),
                &[],
            )?
            .l()?;
        Ok(Arc::new(glory::blitz::clipboard::AndroidClipboard::new(
            env, host,
        )?))
    })
}

#[cfg(all(feature = "files", target_os = "android"))]
fn android_files() -> Result<Arc<glory::blitz::files::AndroidFiles>, Box<dyn std::error::Error>> {
    use jni::{JavaVM, jni_sig, jni_str};
    JavaVM::singleton()?.attach_current_thread(|env| -> Result<_, Box<dyn std::error::Error>> {
        let class = env.load_class(jni_str!("rs.glory.counter.GloryGameActivity"))?;
        let host = env
            .call_static_method(
                class,
                jni_str!("gloryFileHost"),
                jni_sig!(() -> JObject),
                &[],
            )?
            .l()?;
        let driver = glory::blitz::files::AndroidFiles::new(env, host)
            .map_err(|error| std::io::Error::other(format!("Android file host: {error:?}")))?;
        Ok(Arc::new(driver))
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
