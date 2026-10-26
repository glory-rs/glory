//! Shared native control acceptance. Ordinary Counter builds exclude this view.
use glory::prelude::*;
use glory::render::{CheckMode, Event, Semantics};

pub(crate) fn fixture() -> impl Widget {
    widget(|scope| {
        let checked = scope.cage(false);
        let mode = scope.cage(CheckMode::Mixed);
        let status =
            scope.bond(move || format!("Mode: {:?}; checked: {}", mode.get(), checked.get()));
        let readonly = Some(Box::new(Semantics {
            readonly: true,
            ..Default::default()
        }));
        col()
            .style(Style {
                display: Some(Display::Flex),
                flex_direction: Some(FlexDirection::Column),
                align: Some(Align::Start),
                font_family: Some(vec![Family::Sans]),
                font_size: Some(18.),
                ..Style::new().padding(Edges::all(24.)).gap(20.)
            })
            .fill((
                text().content("Native checkbox states"),
                check()
                    .label("Unchecked reference")
                    .semantics(readonly.clone()),
                check()
                    .label("Checked reference")
                    .checked(true)
                    .semantics(readonly),
                check()
                    .label("Interactive mixed checkbox")
                    .checked(checked)
                    .mode(mode)
                    .on(checked_changed, move |event| {
                        if let Event::CheckedChanged(next) = event {
                            checked.revise(|v| *v = next);
                            mode.revise(|v| *v = CheckMode::Checkbox);
                            eprintln!("Glory controls checkbox checked={next}; mode=Checkbox");
                        }
                    }),
                text().content(status),
                button().label("Restore mixed").on(activate, move |_| {
                    mode.revise(|v| *v = CheckMode::Mixed);
                    eprintln!("Glory controls checkbox mode=Mixed");
                }),
            ))
            .show_in(scope)
    })
}
