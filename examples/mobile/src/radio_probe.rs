//! Native radio grouping/controlled-state fixture. No permission host required.
use glory::prelude::*;
use glory::render::CheckMode;

fn column() -> Style {
    Style {
        display: Some(Display::Flex),
        flex_direction: Some(FlexDirection::Column),
        align: Some(Align::Start),
        ..Style::new().gap(16.)
    }
}
pub(crate) fn fixture() -> impl Widget {
    widget(|scope| {
        let selected = scope.cage(0usize);
        let mut rows: Vec<Box<dyn Widget>> = Vec::new();
        for (index, label, enabled) in [(0, "A", true), (1, "B (disabled)", false), (2, "C", true)]
        {
            rows.push(Box::new(
                check()
                    .label(label)
                    .mode(CheckMode::Radio)
                    .enabled(enabled)
                    .checked(selected.map(move |value| *value == index))
                    .semantics(Some(Box::new(Semantics {
                        check_group: Some(SemanticId::new("radio-probe").unwrap()),
                        navigation_item: true,
                        ..Default::default()
                    })))
                    .on(checked_changed, move |event| {
                        if event == Event::CheckedChanged(true) {
                            selected.revise(|value| *value = index);
                            eprintln!("Glory radio probe selected={index}");
                        }
                    }),
            ));
        }
        col()
            .style(Style {
                font_family: Some(vec![Family::Sans]),
                font_size: Some(18.),
                ..column().padding(Edges::all(24.))
            })
            .fill((
                text().content("Native radio group"),
                view()
                    .name("Choice")
                    .role(Role::RadioGroup)
                    .style(column())
                    .semantics(Some(Box::new(Semantics {
                        navigation: Some(Orientation::Vertical),
                        ..Default::default()
                    })))
                    .fill(rows),
                text().content(
                    selected.map(|value| format!("Selected: {}", ["A", "B", "C"][*value])),
                ),
                button()
                    .label("Restore A")
                    .on(activate, move |_| selected.revise(|value| *value = 0)),
                text().content("Touch or Space selects once; arrows skip B; Tab leaves the group."),
            ))
            .show_in(scope)
    })
}
