//! Shared semantic component: no DOM, window, HTML or renderer types.
use glory::prelude::*;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
mod web;

pub struct Counter;

impl Widget for Counter {
    fn build(&mut self, scope: &Scope) -> Result {
        // Preserve in-progress text such as "-" while deriving the numeric value.
        let value = scope.cage(String::from("0"));
        let input_node = scope.cage(None);
        let weak = scope.downgrade();
        // Acknowledge the final value after the whole host tick, even when an
        // edit and Enter/Clear return it to the previous value in the same tick.
        scope.effect(move || {
            let value = value.get();
            if let Some(node) = input_node.get()
                && let Some(scope) = weak.upgrade()
                && let Err(error) = scope.set_input_value(node, value)
            {
                scope.report_error(error);
            }
        });
        let control = Style {
            padding: Some(Edges::all(8.0)),
            font_size: Some(16.0),
            ..Style::default()
        };

        col()
            .class("counter")
            .style(Style {
                padding: Some(Edges::all(24.0)),
                gap: Some(12.0),
                align: Some(Align::Start),
                font_family: Some(vec![Family::Sans]),
                font_size: Some(16.0),
                display: Some(Display::Flex),
                flex_direction: Some(FlexDirection::Column),
                ..Style::default()
            })
            .fill((
                text().kind(TextKind::Heading(1)).content("Counter"),
                text().content(value.map(|value| format!("Value: {}", number(value)))),
                input()
                    .node_ref(input_node)
                    .name("Counter value")
                    .style(control.clone().width(Length::Px(200.0)))
                    .value("0")
                    .on(value_changed, move |event| {
                        if let Event::ValueChanged { value: next, .. } = event {
                            value.revise(|value| *value = next);
                        }
                    })
                    .on(commit, move |event| {
                        if let Event::Commit { value: next, .. } = event {
                            value.revise(|value| *value = number(&next).to_string());
                        }
                    }),
                row()
                    .style(
                        Style::new()
                            .display(Display::Flex)
                            .flex_direction(FlexDirection::Row)
                            .gap(8.0),
                    )
                    .fill((
                        button()
                            .label("Clear")
                            .style(control.clone())
                            .on(activate, move |_| value.revise(|value| *value = "0".into())),
                        button()
                            .label("-1")
                            .style(control.clone())
                            .on(activate, move |_| {
                                value.revise(|value| {
                                    *value = number(value).saturating_sub(1).to_string()
                                })
                            }),
                        button().label("+1").style(control).on(activate, move |_| {
                            value.revise(|value| {
                                *value = number(value).saturating_add(1).to_string()
                            })
                        }),
                    )),
            ))
            .show_in(scope)
    }
}

fn number(value: &str) -> i32 {
    value.parse().unwrap_or_default()
}
