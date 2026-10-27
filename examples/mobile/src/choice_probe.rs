//! Opt-in shared choice-protocol view, independent of intrinsic Props::Select.
use glory::prelude::*;
use glory::render::{
    ChoiceCommand, ComboboxKind, EventKind, Layer, LayerFocus, LayerPlacement, OptionSemantics,
    Orientation, PopupKind,
};
const LABELS: [&str; 3] = ["Alpha", "Beta (disabled)", "中文"];
fn name(s: &str) -> SemanticId {
    SemanticId::new(s).unwrap()
}
fn column() -> Style {
    Style {
        display: Some(Display::Flex),
        flex_direction: Some(FlexDirection::Column),
        align: Some(Align::Start),
        ..Style::new().gap(12.)
    }
}
pub(crate) fn fixture() -> impl Widget {
    widget(|scope| {
        let open = scope.cage(true);
        let current = scope.cage(0usize);
        let selected = scope.cage(0usize);
        let draft = scope.cage(String::new());
        let mut options: Vec<Box<dyn Widget>> = Vec::new();
        for (i, label) in LABELS.into_iter().enumerate() {
            let metadata = selected.map(move |selected| {
                Some(Box::new(Semantics {
                    id: Some(name(&format!("choice-option-{i}"))),
                    option: Some(OptionSemantics {
                        owner: name("choice-options"),
                    }),
                    selected: Some(*selected == i),
                    ..Default::default()
                }))
            });
            options.push(Box::new(
                button()
                    .label(label)
                    .focus(FocusMode::Programmatic)
                    .enabled(i != 1)
                    .style(Style::new().width(Length::Px(260.)).height(Length::Px(36.)))
                    .semantics(metadata)
                    .on(activate, move |_| {
                        selected.revise(|v| *v = i);
                        open.revise(|v| *v = false);
                    })
                    .on(EventKind::Pointer, move |event| {
                        if i != 1
                            && matches!(
                                event,
                                Event::Pointer {
                                    phase: glory::render::PointerPhase::Move,
                                    ..
                                }
                            )
                        {
                            current.revise(|v| *v = i);
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
                text().content("Native combobox and typeahead"),
                button()
                    .label(selected.map(|i| format!("Choose: {}", LABELS[*i])))
                    .name("Choice")
                    .style(Style::new().width(Length::Px(280.)).height(Length::Px(40.)))
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(name("choice-trigger")),
                            combobox: Some(ComboboxKind::Select),
                            controls: Some(name("choice-options")),
                            popup: Some(PopupKind::Listbox),
                            expanded: Some(open.get()),
                            typeahead: true,
                            active_descendant: open
                                .get()
                                .then(|| name(&format!("choice-option-{}", current.get()))),
                            ..Default::default()
                        }))
                    }))
                    .on(activate, move |_| open.revise(|v| *v = !*v))
                    .on(EventKind::Choice, move |event| {
                        if let Event::Choice { command, .. } = event {
                            match command {
                                ChoiceCommand::OpenFirst | ChoiceCommand::First => {
                                    current.revise(|v| *v = 0);
                                    open.revise(|v| *v = true);
                                }
                                ChoiceCommand::OpenLast | ChoiceCommand::Last => {
                                    current.revise(|v| *v = 2);
                                    open.revise(|v| *v = true);
                                }
                                ChoiceCommand::Next | ChoiceCommand::Previous => {
                                    if open.get() {
                                        current.revise(|v| *v = if *v == 0 { 2 } else { 0 });
                                    } else {
                                        open.revise(|v| *v = true);
                                    }
                                }
                                ChoiceCommand::Commit => {
                                    if open.get() {
                                        selected.revise(|v| *v = current.get());
                                        open.revise(|v| *v = false);
                                    } else {
                                        open.revise(|v| *v = true);
                                    }
                                }
                                ChoiceCommand::Close => open.revise(|v| *v = false),
                                ChoiceCommand::RemoveLast => {}
                            }
                        }
                    })
                    .on(EventKind::Typeahead, move |event| {
                        if let Event::Typeahead { text, .. } = event
                            && let Some(i) = LABELS.iter().enumerate().find_map(|(i, label)| {
                                (i != 1 && label.to_lowercase().starts_with(&text.to_lowercase()))
                                    .then_some(i)
                            })
                        {
                            current.revise(|v| *v = i);
                            open.revise(|v| *v = true);
                        }
                    }),
                view()
                    .role(Role::Listbox)
                    .style(Style {
                        background: Some(Color(245, 245, 245, 255)),
                        ..column().width(Length::Px(280.)).padding(Edges::all(4.))
                    })
                    .semantics(open.map(move |open| {
                        Some(Box::new(Semantics {
                            id: Some(name("choice-options")),
                            labelled_by: Some(name("choice-trigger")),
                            navigation: Some(Orientation::Vertical),
                            hidden: !*open,
                            layer: Some(Layer {
                                modal: false,
                                focus: LayerFocus::Preserve,
                                placement: LayerPlacement::Anchor(
                                    glory::render::AnchorPlacement::new(
                                        glory::render::AnchorSide::Bottom,
                                        glory::render::AnchorAlign::Start,
                                    ),
                                ),
                                anchor: Some(name("choice-trigger")),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }))
                    }))
                    .on(EventKind::Dismiss, move |_| open.revise(|v| *v = false))
                    .fill(options),
                text().content(selected.map(|i| format!("Selected: {}", LABELS[*i]))),
                input()
                    .name("IME reference")
                    .value(draft)
                    .semantics(Some(Box::new(Semantics {
                        placeholder: Some("IME reference".into()),
                        ..Default::default()
                    })))
                    .on(value_changed, move |event| {
                        if let Event::ValueChanged { value, .. } = event {
                            draft.revise(|v| *v = value);
                        }
                    }),
            ))
            .show_in(scope)
    })
}
