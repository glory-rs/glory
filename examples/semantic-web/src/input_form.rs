use glory::prelude::*;
use glory::render::{Event, Purpose};
use glory::ui::{Scope, View, events};

pub struct State {
    pub values: Vec<Cage<String>>,
    pub commits: Cage<usize>,
}
pub fn ui(scope: &Scope) -> (View, State) {
    let commits = scope.cage(0usize);
    let mut values = Vec::new();
    let mut fields = Vec::new();
    for (index, purpose) in [
        Purpose::Text,
        Purpose::Password,
        Purpose::Email,
        Purpose::Search,
        Purpose::Url,
        Purpose::Tel,
        Purpose::Number,
        Purpose::Text,
    ]
    .into_iter()
    .enumerate()
    {
        let value = scope.cage(
            if index == 7 {
                "\r\nfirst\rsecond\0🙂"
            } else {
                "  a\0🙂b  "
            }
            .to_owned(),
        );
        values.push(value);
        fields.push(
            input()
                .class(format!("input-slot-{index}"))
                .purpose(purpose)
                .multiline(index == 7)
                .value(value)
                .on(events::value_changed, move |event| {
                    if let Event::ValueChanged { value: next, .. } = event {
                        value.revise(|v| *v = next);
                    }
                })
                .on(events::commit, move |_| commits.revise(|n| *n += 1)),
        );
    }
    (
        view().class("input-form").fill(fields),
        State { values, commits },
    )
}
