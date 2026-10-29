//! UIKit event loop; iOS launches this executable from its application bundle.
#[cfg(target_os = "ios")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use glory::apace::shell::{Shell, WindowAttributes};
    let mut shell = Shell::new()?;
    #[cfg(not(feature = "network-probe"))]
    let (window, _loader) = glory_mobile_counter::counter_window()?;
    #[cfg(feature = "network-probe")]
    let (window, _loader) = glory_mobile_counter::network_window()?;
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
