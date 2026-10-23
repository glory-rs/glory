use glory::prelude::*;
pub struct State {
    pub expanded: Cage<bool>,
    pub show: Cage<bool>,
    pub relation: Cage<bool>,
}
pub fn ui(scope: &Scope, initial: bool) -> (impl Widget + use<>, State) {
    let target = scope.cage(None);
    let expanded = scope.cage(initial);
    let show = scope.cage(true);
    let relation = scope.cage(true);
    let value = scope.cage("initial".to_owned());
    let disclosure = scope.bond(move || {
        if relation.get() {
            target.get().map(|controls| Disclosure {
                expanded: expanded.get(),
                controls,
            })
        } else {
            None
        }
    });
    (
        group().fill((
            when(show, move || {
                view()
                    .node_ref(target)
                    .class("disclosure-drawer")
                    .name("Drawer")
                    .fill(
                        input()
                            .class("disclosure-draft")
                            .name("Draft")
                            .value(value)
                            .on(value_changed, move |event| {
                                if let Event::ValueChanged { value: next, .. } = event {
                                    value.revise(|value| *value = next)
                                }
                            }),
                    )
            }),
            button()
                .class("disclosure-button")
                .label("Menu")
                .disclosure(disclosure)
                .on(activate, move |_| expanded.revise(|value| *value = !*value)),
            button()
                .label("Retire drawer")
                .on(activate, move |_| show.revise(|value| *value = false)),
            button()
                .label("Clear relation")
                .on(activate, move |_| relation.revise(|value| *value = false)),
            button()
                .label("Restore relation")
                .on(activate, move |_| relation.revise(|value| *value = true)),
        )),
        State {
            expanded,
            show,
            relation,
        },
    )
}
