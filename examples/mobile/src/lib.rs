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
    #[cfg(not(any(feature = "ime-probe", feature = "keyboard-probe")))]
    let content = counter::Counter;
    #[cfg(feature = "ime-probe")]
    let content = ime_probe::fixture();
    #[cfg(feature = "keyboard-probe")]
    let content = keyboard_probe::fixture();
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
    #[cfg(feature = "android-assets")]
    let assets = glory::blitz::assets::AndroidAssets::new(app.asset_manager(), "glory")?;
    #[cfg(feature = "android-assets-probe")]
    android_assets_probe::run(assets.clone())?;
    eprintln!("Glory mobile: creating Activity event loop");
    let mut shell = Shell::android(app)?;
    eprintln!("Glory mobile: creating semantic document");
    #[cfg(not(feature = "android-assets"))]
    let (window, loader) = counter_window()?;
    #[cfg(feature = "android-assets")]
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
    eprintln!("Glory mobile: entering native window loop");
    shell.add_window(
        window,
        WindowAttributes::default().with_title("Glory Counter"),
    );
    shell.run()?;
    eprintln!("Glory mobile assets: {:?}", loader.stats());
    Ok(())
}
