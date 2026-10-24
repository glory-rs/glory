//! One semantic factory for native pending SSR and final-state browser adoption.
use glory::prelude::*;
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

pub type Load =
    Rc<dyn Fn(u64) -> Pin<Box<dyn Future<Output = std::result::Result<String, String>>>>>;
pub struct State {
    pub draft: Cage<Option<String>>,
    pub checked: Cage<Option<bool>>,
    pub count: Cage<u64>,
    pub first: Resource<u64, String, String>,
    pub loads: Rc<Cell<usize>>,
}
fn ready(value: &ResourceState<u64, String, String>) -> bool {
    matches!(value, ResourceState::Ready { .. })
}
fn label(value: &ResourceState<u64, String, String>) -> String {
    match value {
        ResourceState::Ready { value, .. } => value.to_string(),
        _ => "pending".into(),
    }
}
pub fn widget(
    resources: Resources,
    load: Load,
    state: Rc<RefCell<Option<State>>>,
) -> impl Widget + use<> {
    glory::ui::widget(move |scope| {
        let loads = Rc::new(Cell::new(0));
        let loader = load.clone();
        let calls = loads.clone();
        let first = resources
            .bind(
                scope.owner(),
                "first",
                "v1",
                || 1_u64,
                move |id| {
                    calls.set(calls.get() + 1);
                    loader(id)
                },
            )
            .map_err(|error| {
                glory::ui::Error::Render(glory::render::Error::Backend(error.to_string()))
            })?;
        let loader = load.clone();
        let calls = loads.clone();
        let second = resources
            .bind(
                scope.owner(),
                "second",
                "v1",
                || 2_u64,
                move |id| {
                    calls.set(calls.get() + 1);
                    loader(id)
                },
            )
            .map_err(|error| {
                glory::ui::Error::Render(glory::render::Error::Backend(error.to_string()))
            })?;
        let draft = scope.cage(None::<String>);
        let checked = scope.cage(None::<bool>);
        let count = scope.cage(0_u64);
        let value = scope.bond(move || {
            draft.get().unwrap_or_else(|| {
                if ready(&second.get()) {
                    "server new".into()
                } else {
                    "server initial".into()
                }
            })
        });
        let check_value =
            scope.bond(move || checked.get().unwrap_or_else(|| !ready(&second.get())));
        let order = first.map(|value| {
            if ready(value) {
                vec![2_usize, 0, 1]
            } else {
                vec![0, 1, 2]
            }
        });
        view()
            .class("stream-app")
            .fill((
                text().class("stream-title").content("shell now"),
                input()
                    .class("stream-editor")
                    .value(value)
                    .on(value_changed, move |event| {
                        if let Event::ValueChanged { value, .. } = event {
                            draft.revise(|draft| *draft = Some(value));
                        }
                    }),
                check()
                    .class("stream-check")
                    .label("choice")
                    .checked(check_value)
                    .on(checked_changed, move |event| {
                        if let Event::CheckedChanged(value) = event {
                            checked.revise(|old| *old = Some(value));
                        }
                    }),
                button()
                    .class("stream-count")
                    .label(count.map(|count| format!("count {count}")))
                    .on(activate, move |_| count.revise(|count| *count += 1)),
                when(first.map(ready), move || {
                    group().fill(text().class("stream-first").content(first.map(label)))
                })
                .otherwise(|| text().class("stream-wait-first").content("waiting first")),
                when(second.map(ready), move || {
                    text().class("stream-second").content(second.map(label))
                })
                .otherwise(|| text().class("stream-wait-second").content("waiting second")),
                view().class("stream-keyed").fill(each(
                    order,
                    |key| *key,
                    |item, _| {
                        glory::ui::widget(move |scope| {
                            let key = item.get();
                            let draft = scope.cage(format!("row {key}"));
                            input()
                                .class(format!("stream-row-{key}"))
                                .value(draft)
                                .on(value_changed, move |event| {
                                    if let Event::ValueChanged { value, .. } = event {
                                        draft.revise(|draft| *draft = value);
                                    }
                                })
                                .show_in(scope)
                        })
                    },
                )),
                group().fill(glory::ui::widget(move |scope| {
                    let hidden = Rc::new(
                        group()
                            .fill((
                                input().class("stream-hidden").value("hidden"),
                                text().content("hidden tail"),
                            ))
                            .mount(scope)?,
                    );
                    hidden.suspend()?;
                    let mounted = hidden.clone();
                    scope.effect(move || {
                        if ready(&second.get()) {
                            mounted.place_before(None).expect("resume hidden");
                        }
                    });
                    scope.retain(hidden)
                })),
                group().fill(glory::ui::widget(move |scope| {
                    let middle = Rc::new(
                        group()
                            .fill((
                                input().class("stream-middle").value("middle"),
                                text().content("middle tail"),
                            ))
                            .mount(scope)?,
                    );
                    let mounted = middle.clone();
                    scope.effect(move || {
                        if ready(&first.get()) && !ready(&second.get()) {
                            mounted.suspend().expect("suspend middle");
                        } else {
                            mounted.place_before(None).expect("resume middle");
                        }
                    });
                    scope.retain(middle)
                })),
                list(
                    100_000_000_usize,
                    |index| index,
                    |_, key| text().content(format!("stream row {key}")),
                )
                .height(240.0)
                .row_height(24.0)
                .overscan(2),
            ))
            .show_in(scope)?;
        *state.borrow_mut() = Some(State {
            draft,
            checked,
            count,
            first,
            loads,
        });
        Ok(())
    })
}
