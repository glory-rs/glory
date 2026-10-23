#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    use glory_blitz::shell::{Shell, WindowApp, WindowAttributes};
    use glory_ui::{events::activate, *};
    let app = WindowApp::new();
    app.renderer()
        .set_css("body { font: 16px sans-serif } .app { padding:16px } button { padding:8px }");
    app.mount_with(|| {
        component(|scope| {
            let count = scope.cage(0);
            view()
                .class("app")
                .fill((
                    text().content(count.map(|count| format!("Count: {count}"))),
                    button()
                        .label("Increase")
                        .on(activate, move |_| count.revise(|value| *value += 1)),
                    list(
                        100_000_000_usize,
                        |index| index,
                        |index, _| text().content(index.map(|index| format!("Entry {index}"))),
                    )
                    .height(300.0)
                    .row_height(24.0)
                    .overscan(3),
                ))
                .show_in(scope)
        })
    })?;
    let mut shell = Shell::new()?;
    shell.add_window(app, WindowAttributes::default().with_title("Glory native"));
    shell.run()?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn main() {}
