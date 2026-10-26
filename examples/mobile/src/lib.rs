//! Mobile application host; the component remains shared with Web/desktop.
use glory::blitz::{
    Config,
    assets::{Bytes, LoadConfig, Loader, Url},
    shell::WindowApp,
};
use glory::prelude::{group, image};
use std::{collections::BTreeMap, sync::Arc};

#[cfg(feature = "ime-probe")]
mod ime_probe;

/// One package worker per document, including every recreated Android Activity.
pub fn counter_window() -> std::result::Result<(WindowApp, Arc<Loader>), Box<dyn std::error::Error>>
{
    let loader = Arc::new(Loader::new(LoadConfig {
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
    })?);
    let window = WindowApp::with_config(Config {
        base_url: Url::parse("asset://app/")?,
        provider: Some(loader.clone()),
        ..Default::default()
    });
    window.renderer().set_css("@import url('theme.css');");
    #[cfg(not(feature = "ime-probe"))]
    let content = counter::Counter;
    #[cfg(feature = "ime-probe")]
    let content = ime_probe::fixture();
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
    eprintln!("Glory mobile: creating Activity event loop");
    let mut shell = Shell::android(app)?;
    eprintln!("Glory mobile: creating semantic document");
    let (window, loader) = counter_window()?;
    eprintln!("Glory mobile: entering native window loop");
    shell.add_window(
        window,
        WindowAttributes::default().with_title("Glory Counter"),
    );
    shell.run()?;
    eprintln!("Glory mobile assets: {:?}", loader.stats());
    Ok(())
}
