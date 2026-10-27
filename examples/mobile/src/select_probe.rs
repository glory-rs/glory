//! Standalone intrinsic controls; no hidden proxy or combobox refinement.
use glory::prelude::*;
use glory::render::{Event, EventKind, SelectEntry, SelectGroup, SelectOption, SelectProps};
use glory::ui::select;
use std::rc::Rc;
pub(crate) const CSS: &str = ".select-probe{display:flex;flex-direction:column;align-items:flex-start;gap:16px;padding:24px;font-family:sans-serif;font-size:18px}.select-probe-column{display:flex;flex-direction:column;gap:8px;width:240px}@media(min-width:720px){.select-probe{flex-direction:row}}";
fn key(s: &str) -> SemanticId {
    SemanticId::new(s).unwrap()
}
fn options() -> Rc<Vec<SelectEntry>> {
    let option = |id: &str, label: &str, disabled| SelectOption {
        key: key(id),
        label: label.into(),
        value: format!("value-{id}"),
        disabled,
    };
    Rc::new(vec![
        SelectEntry::Option(option("alpha", "Alpha", false)),
        SelectEntry::Option(option("disabled", "Disabled", true)),
        SelectEntry::Group(SelectGroup {
            label: "Letters / 中文".into(),
            disabled: false,
            options: vec![
                option("beta", "Beta", false),
                option("chinese", "中文", false),
            ],
        }),
        SelectEntry::Group(SelectGroup {
            label: "Disabled group".into(),
            disabled: true,
            options: vec![option("blocked", "Unavailable", false)],
        }),
    ])
}
pub(crate) fn fixture() -> impl Widget {
    widget(|scope| {
        let entries = options();
        let single = scope.cage(vec![key("chinese")]);
        let rows = scope.cage(vec![key("alpha")]);
        let multi = scope.cage(vec![key("alpha"), key("chinese")]);
        eprintln!(
            "Glory Select probe: intrinsic single / rows / multiple; grouped and disabled options"
        );
        view()
            .class("select-probe")
            .fill((
                view().class("select-probe-column").fill((
                    text().content("Single selection"),
                    select()
                        .name("Single selection")
                        .configuration(SelectProps {
                            entries: entries.clone(),
                            placeholder: "请选择".into(),
                            ..Default::default()
                        })
                        .selected(single)
                        .style(Style::new().width(Length::Px(240.)))
                        .on(EventKind::SelectChanged, move |event| {
                            if let Event::SelectChanged { keys, .. } = event {
                                single.revise(|v| *v = keys)
                            }
                        }),
                    text().content(single.map(|keys| {
                        format!("Selected: {}", keys.first().map_or("none", |k| k.as_str()))
                    })),
                )),
                view().class("select-probe-column").fill((
                    text().content("Single listbox / rows"),
                    select()
                        .name("Single listbox")
                        .configuration(SelectProps {
                            entries: entries.clone(),
                            rows: Some(6),
                            ..Default::default()
                        })
                        .selected(rows)
                        .style(Style::new().width(Length::Px(240.)))
                        .on(EventKind::SelectChanged, move |event| {
                            if let Event::SelectChanged { keys, .. } = event {
                                rows.revise(|v| *v = keys)
                            }
                        }),
                )),
                view().class("select-probe-column").fill((
                    text().content("Multiple selection"),
                    select()
                        .name("Multiple selection")
                        .configuration(SelectProps {
                            entries,
                            multiple: true,
                            rows: Some(8),
                            ..Default::default()
                        })
                        .selected(multi)
                        .style(Style::new().width(Length::Px(240.)))
                        .on(EventKind::SelectChanged, move |event| {
                            if let Event::SelectChanged { keys, .. } = event {
                                multi.revise(|v| *v = keys)
                            }
                        }),
                    text().content(multi.map(|keys| format!("{} selected", keys.len()))),
                )),
            ))
            .show_in(scope)
    })
}
