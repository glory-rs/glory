//! Opt-in native IME acceptance view; absent from the ordinary shared counter.
use glory::prelude::*;

const ORIGINAL: &str = "A中😀Z";

pub(crate) fn fixture() -> impl Widget {
    widget(|scope| {
        let value = scope.cage(String::from(ORIGINAL));
        let node = scope.cage(None);
        let composing = scope.cage(false);
        let weak = scope.downgrade();
        let echo = weak.clone();
        scope.effect(move || {
            let current = value.get();
            eprintln!(
                "Glory IME probe value={current:?}, composing={}",
                composing.get()
            );
            if let Some(node) = node.get()
                && let Some(scope) = echo.upgrade()
                && let Err(error) = scope.set_input_value(node, current)
            {
                scope.report_error(error);
            }
        });
        let controls = Style {
            font_size: Some(16.),
            ..Style::new().padding(Edges::all(8.))
        };
        col()
            .style(Style {
                font_size: Some(16.),
                align: Some(Align::Start),
                ..Style::new()
                    .padding(Edges::all(24.))
                    .gap(12.)
                    .display(Display::Flex)
                    .flex_direction(FlexDirection::Column)
            })
            .fill((
                text().content("IME Probe"),
                text().content(value.map(|value| format!("Value: {value}"))),
                text().content(composing.map(|active| {
                    if *active {
                        "Composing".to_owned()
                    } else {
                        "Idle".to_owned()
                    }
                })),
                input()
                    .name("IME probe value")
                    .node_ref(node)
                    .style(controls.clone().width(Length::Px(240.)))
                    .value(ORIGINAL)
                    .on(value_changed, move |event| {
                        if let Event::ValueChanged { value: next, .. } = event {
                            value.revise(|value| *value = next);
                        }
                    })
                    .on(composition, move |event| {
                        if let Event::Composition { active } = event {
                            composing.revise(|value| *value = active);
                        }
                    }),
                row()
                    .style(Style::new().display(Display::Flex).gap(8.))
                    .fill((
                        button()
                            .label("Select middle")
                            .style(controls.clone())
                            .enabled(value.map(|v| v == ORIGINAL))
                            .on(activate, move |_| {
                                eprintln!("Glory IME probe select middle");
                                if let (Some(scope), Some(node)) = (weak.upgrade(), node.get()) {
                                    let result = scope
                                        .action(glory::render::Action::Focus(node))
                                        .and_then(|()| {
                                            scope.set_input_selection(
                                                node,
                                                glory::render::TextSelection {
                                                    start: 1,
                                                    end: 4,
                                                    ..Default::default()
                                                },
                                            )
                                        });
                                    if let Err(error) = result {
                                        scope.report_error(error);
                                    }
                                }
                            }),
                        button()
                            .label("Restore")
                            .style(controls.clone())
                            .on(activate, move |_| {
                                eprintln!("Glory IME probe restore");
                                value.revise(|v| *v = ORIGINAL.into());
                            }),
                        button()
                            .label("Reset")
                            .style(controls)
                            .on(activate, move |_| {
                                eprintln!("Glory IME probe reset");
                                value.revise(|v| *v = "RESET".into());
                            }),
                    )),
            ))
            .show_in(scope)
    })
}
