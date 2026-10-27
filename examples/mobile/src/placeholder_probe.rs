//! Opt-in native placeholder view; ordinary packages keep the shared Counter.
use glory::prelude::*;

fn hint(text: &str) -> Option<Box<Semantics>> {
    Some(Box::new(Semantics {
        placeholder: Some(text.into()),
        ..Default::default()
    }))
}
fn field(height: f64) -> Style {
    Style {
        font_family: Some(vec![Family::Sans]),
        font_size: Some(18.),
        ..Style::new()
            .width(Length::Px(260.))
            .height(Length::Px(height))
            .padding(Edges::all(4.))
    }
}
pub(crate) fn fixture() -> impl Widget {
    widget(|scope| {
        let alternate = scope.cage(false);
        let value = scope.cage(String::new());
        let password = scope.cage(String::new());
        let multiline = scope.cage(String::new());
        let changed = |value: glory::core::Cage<String>| {
            move |event| {
                if let Event::ValueChanged { value: next, .. } = event {
                    value.revise(|current| *current = next);
                }
            }
        };
        col()
            .style(Style {
                display: Some(Display::Flex),
                flex_direction: Some(FlexDirection::Column),
                align: Some(Align::Start),
                font_size: Some(18.),
                font_family: Some(vec![Family::Sans]),
                ..Style::new().padding(Edges::all(24.)).gap(10.)
            })
            .fill((
                text().content("Native placeholders"),
                text().content("Text"),
                input()
                    .name("Search contacts")
                    .style(field(36.))
                    .value(value)
                    .semantics(alternate.map(|v| {
                        hint(if *v {
                            "Updated search hint"
                        } else {
                            "Search contacts"
                        })
                    }))
                    .on(value_changed, changed(value)),
                text().content("Password"),
                input()
                    .name("Account password")
                    .purpose(Purpose::Password)
                    .style(field(36.))
                    .value(password)
                    .semantics(hint("Enter password"))
                    .on(value_changed, changed(password)),
                text().content("Multiline"),
                input()
                    .name("Message")
                    .multiline(true)
                    .style(field(72.))
                    .value(multiline)
                    .semantics(hint("First line\nSecond line"))
                    .on(value_changed, changed(multiline)),
                input()
                    .name("Filled reference")
                    .value("Existing value")
                    .style(field(36.))
                    .semantics(hint("Hidden hint")),
                button()
                    .label("Change hint")
                    .on(activate, move |_| alternate.revise(|v| *v = !*v)),
                button().label("Clear values").on(activate, move |_| {
                    value.revise(String::clear);
                    password.revise(String::clear);
                    multiline.revise(String::clear);
                }),
            ))
            .show_in(scope)
    })
}
