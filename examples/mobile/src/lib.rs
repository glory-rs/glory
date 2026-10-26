//! Mobile application host; the component remains shared with Web/desktop.
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
    use glory::blitz::shell::{Shell, WindowApp, WindowAttributes};
    eprintln!("Glory mobile: creating Activity event loop");
    let mut shell = Shell::android(app)?;
    eprintln!("Glory mobile: creating semantic document");
    let window = WindowApp::new();
    window.mount(counter::Counter)?;
    eprintln!("Glory mobile: entering native window loop");
    shell.add_window(
        window,
        WindowAttributes::default().with_title("Glory Counter"),
    );
    shell.run()?;
    Ok(())
}
