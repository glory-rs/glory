//! Bottom-anchored editor for real software-keyboard/viewport acceptance.
use glory::prelude::*;

pub(crate) fn fixture() -> impl Widget {
    widget(|scope| {
        let value = scope.cage("底部".to_owned());
        col()
            .style(Style::new().padding(Edges::all(24.)).gap(12.))
            .fill((
                text().content("Keyboard avoidance"),
                text().content(value.map(|s| format!("Value: {s}"))),
                input()
                    .name("Bottom keyboard value")
                    .value(value)
                    .style(Style {
                        font_size: Some(18.),
                        ..Style::new()
                            .position(Position::Fixed)
                            .left(Length::Px(24.))
                            .bottom(Length::Px(24.))
                            .width(Length::Px(260.))
                            .height(Length::Px(44.))
                            .padding(Edges::all(8.))
                    })
                    .on(value_changed, move |event| {
                        if let Event::ValueChanged { value: next, .. } = event {
                            value.revise(|v| *v = next);
                        }
                    }),
            ))
            .show_in(scope)
    })
}
