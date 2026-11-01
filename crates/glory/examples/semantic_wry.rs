//! Native Rust state and handlers; the system WebView only interprets DOM edits.
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    use glory::prelude::*;
    use glory::wry::shell::{Config, Shell, WindowApp};
    let window = WindowApp::new();
    let count = window.app().scope().cage(0i32);
    let value = window
        .app()
        .scope()
        .cage(String::from("Native Rust + WebView"));
    window.mount(
        col()
            .style(
                Style::new()
                    .display(Display::Flex)
                    .flex_direction(FlexDirection::Column)
                    .gap(12.)
                    .padding(Edges::all(24.)),
            )
            .fill((
                text().kind(TextKind::Heading(1)).content("Glory Wry"),
                text().content(count.map(|n| format!("Count: {n}"))),
                input()
                    .name("Message")
                    .value(value)
                    .on(value_changed, move |event| {
                        if let Event::ValueChanged { value: next, .. } = event {
                            value.revise(|v| *v = next);
                        }
                    }),
                text().content(value),
                button()
                    .label("+1")
                    .on(activate, move |_| count.revise(|n| *n += 1)),
            )),
    )?;
    Shell::new().run(
        window,
        Config {
            title: "Glory · native Wry backend".into(),
            ..Default::default()
        },
    )?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn main() {}
