//! Desktop host. The component and all event handlers live in the shared library.
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use glory::apace::shell::{Shell, WindowApp, WindowAttributes};

    let app = WindowApp::new();
    app.mount(counter::Counter)?;

    let mut shell = Shell::new()?;
    shell.add_window(app, WindowAttributes::default().with_title("Glory Counter"));
    shell.run()?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn main() -> std::process::ExitCode {
    eprintln!("counter-desktop requires Windows, Linux or macOS; Android needs a mobile host.");
    std::process::ExitCode::FAILURE
}
