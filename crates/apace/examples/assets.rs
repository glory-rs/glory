#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{collections::BTreeMap, sync::Arc};

    use glory_apace::Config;
    use glory_apace::assets::{Bytes, LoadConfig, Loader, Url};
    use glory_apace::shell::{Shell, WindowApp, WindowAttributes};
    use glory_ui::{list, text, view};

    let mount = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let base = if mount.is_some() {
        "asset://media/"
    } else {
        "asset://app/"
    };
    let loader = Arc::new(Loader::new(LoadConfig {
        package: BTreeMap::from([(
            "/theme.css".into(),
            Bytes::from_static(b"body{font:16px sans-serif;background:#f4f7fa} .app{padding:16px} .row{height:28px;background:white;border-bottom:1px solid #e4e8ee}"),
        )]),
        // A directory argument explicitly grants one mount containing
        // theme.css. No argument uses only the immutable built-in package.
        mounts: mount.map(|path| BTreeMap::from([("media".into(), path)])).unwrap_or_default(),
        // Empty origins: this example cannot issue HTTP.
        ..Default::default()
    })?);
    let app = WindowApp::with_config(Config {
        base_url: Url::parse(base)?,
        provider: Some(loader),
        ..Default::default()
    });
    app.renderer().set_css("@import url('theme.css');");
    app.mount(
        view().class("app").fill((
            text().content("Owned asset package; 100,000,000 windowed rows"),
            list(
                100_000_000_usize,
                |index| index,
                |index, _| {
                    text()
                        .class("row")
                        .content(index.map(|index| format!("Entry {index}")))
                },
            )
            .height(280.)
            .row_height(28.)
            .overscan(2),
        )),
    )?;
    let mut shell = Shell::new()?;
    shell.add_window(app, WindowAttributes::default().with_title("Glory assets"));
    shell.run()?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn main() {}
