//! UIKit event loop; iOS launches this executable from its application bundle.
#[cfg(target_os = "ios")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use glory::blitz::shell::{Shell, WindowApp, WindowAttributes};
    let mut shell = Shell::new()?;
    let window = WindowApp::new();
    window.mount(counter::Counter)?;
    shell.add_window(
        window,
        WindowAttributes::default().with_title("Glory Counter"),
    );
    shell.run()?;
    Ok(())
}

#[cfg(not(target_os = "ios"))]
fn main() -> std::process::ExitCode {
    eprintln!("counter-ios requires an iOS target and the Apple SDK");
    std::process::ExitCode::FAILURE
}
