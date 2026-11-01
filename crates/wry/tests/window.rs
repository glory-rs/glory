#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
#[path = "window/desktop.rs"]
mod desktop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    desktop::run()?;
    Ok(())
}
