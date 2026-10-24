//! One pure Rust widget factory for the native SSR generator and wasm app.
use glory::prelude::*;
use glory::ui::list::ListStats;
use std::cell::RefCell;
use std::rc::Rc;

pub struct State {
    pub count: Cage<usize>,
    pub value: Cage<String>,
    pub multiline: Cage<String>,
    pub checked: Cage<bool>,
    pub empty: Cage<String>,
    pub show: Cage<bool>,
    pub order: Cage<Vec<usize>>,
    pub position: Cage<f64>,
    pub stats: Rc<RefCell<Option<ListStats>>>,
}
pub fn ui(scope: &Scope) -> (glory::ui::View, State) {
    let count = scope.cage(0_usize);
    let value = scope.cage("server value".to_owned());
    let multiline = scope.cage("\nline<&>\r\n末尾".to_owned());
    let checked = scope.cage(true);
    let empty = scope.cage(String::new());
    let show = scope.cage(true);
    let order = scope.cage(vec![0_usize, 1, 2]);
    let position = scope.cage(0.0);
    let stats = Rc::new(RefCell::new(None));
    let inspected = stats.clone();
    let widget = view().class("hydrated").fill((
        group().fill((
            text().kind(TextKind::Heading(2)).content("SSR & Web"),
            text()
                .kind(TextKind::Paragraph)
                .class("h-paragraph")
                .content("a\r\nb\0<&>"),
            text().class("h-empty").content(empty),
        )),
        view().class("h-form").fill((
            input()
                .class("h-editor")
                .value(value)
                .on(value_changed, move |event| {
                    if let Event::ValueChanged { value: next, .. } = event {
                        value.revise(|value| *value = next);
                    }
                }),
            input()
                .class("h-multiline")
                .multiline(true)
                .value(multiline)
                .on(value_changed, move |event| {
                    if let Event::ValueChanged { value, .. } = event {
                        multiline.revise(|old| *old = value);
                    }
                }),
            check()
                .class("h-check")
                .label("checked")
                .checked(checked)
                .on(checked_changed, move |event| {
                    if let Event::CheckedChanged(value) = event {
                        checked.revise(|old| *old = value);
                    }
                }),
            widget(|scope| {
                let mut descriptor =
                    glory::render::Descriptor::new(glory::render::Props::Input(Default::default()));
                descriptor.classes.push("h-initial-focus".into());
                let node = scope.create(descriptor)?;
                if scope.capabilities()?.focus {
                    scope.action(glory::render::Action::Focus(node))?;
                }
                Ok(())
            }),
        )),
        button()
            .class("h-add")
            .label(count.map(|count| format!("count {count}")))
            .fill(text().class("h-icon").content("icon"))
            .on(activate, move |_| count.revise(|count| *count += 1)),
        link().class("h-link").target("#fixture").label("link"),
        image().class("h-image").source("").alt("fixture"),
        view().class("h-keyed").fill(each(
            order,
            |value| *value,
            |item, _| {
                input()
                    .name(item.map(|item| format!("hydrate key {item}")))
                    .value(item.map(|item| format!("item {item}")))
            },
        )),
        when(show, || {
            group().fill((
                text().class("h-branch").content("branch"),
                text().content("tail"),
            ))
        }),
        view().class("h-list").fill(
            list(
                100_000_000_usize,
                |index| index,
                |_, key| {
                    view()
                        .style(Style::new().height(Length::Px(24.375 + (key % 3) as f64 * 8.0)))
                        .fill(text().content(format!("row {key}")))
                },
            )
            .height(240.0)
            .measure(24.0)
            .measure_limit(64)
            .position(position)
            .inspect(move |value| *inspected.borrow_mut() = Some(value)),
        ),
    ));
    (
        widget,
        State {
            count,
            value,
            multiline,
            checked,
            empty,
            show,
            order,
            position,
            stats,
        },
    )
}
