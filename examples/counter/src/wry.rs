//! The same Counter component, with native Rust logic and a system WebView.
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use glory::wry::shell::{Config, Shell, WindowApp};
    let window = WindowApp::new();
    window.mount(counter::Counter)?;
    Shell::new().run(
        window,
        Config {
            title: "Glory Counter · Wry".into(),
            ..Default::default()
        },
    )?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn main() {}
