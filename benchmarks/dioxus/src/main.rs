//! Dioxus implementation of the js-framework-benchmark "row table" workload.
//!
//! Same DOM contract and control buttons as the glory / leptos apps, driven by
//! the same `../bench-harness.js`. This is written the idiomatic dioxus way: a
//! single `Signal<Vec<Row>>` whose mutations are reconciled by the VirtualDom
//! (keyed `<tr>`s), which is the architecture being compared here.

use dioxus::prelude::*;

const ADJECTIVES: [&str; 25] = [
    "pretty", "large", "big", "small", "tall", "short", "long", "handsome", "plain", "quaint",
    "clean", "elegant", "easy", "angry", "crazy", "helpful", "mushy", "odd", "unsightly",
    "adorable", "important", "inexpensive", "cheap", "expensive", "fancy",
];
const COLOURS: [&str; 11] = [
    "red", "yellow", "blue", "green", "pink", "brown", "purple", "brown", "white", "black",
    "orange",
];
const NOUNS: [&str; 13] = [
    "table", "chair", "house", "bbq", "desk", "car", "pony", "cookie", "sandwich", "burger",
    "pizza", "mouse", "keyboard",
];

#[derive(Clone, PartialEq)]
struct Row {
    id: usize,
    label: String,
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut rows = use_signal(Vec::<Row>::new);
    let mut selected = use_signal(|| None::<usize>);
    let mut next_id = use_signal(|| 1usize);
    let mut rng = use_signal(|| 0x2545_F491_4F6C_DD1D_u64);

    let mut make_rows = move |count: usize| -> Vec<Row> {
        let mut state = *rng.peek();
        let mut draw = |n: usize| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 33) as usize) % n
        };
        let mut id = *next_id.peek();
        let out = (0..count)
            .map(|_| {
                let label = format!(
                    "{} {} {}",
                    ADJECTIVES[draw(ADJECTIVES.len())],
                    COLOURS[draw(COLOURS.len())],
                    NOUNS[draw(NOUNS.len())],
                );
                let row = Row { id, label };
                id += 1;
                row
            })
            .collect();
        next_id.set(id);
        rng.set(state);
        out
    };

    rsx! {
        button { id: "run", onclick: move |_| rows.set(make_rows(1_000)), "Create 1,000 rows" }
        button { id: "runlots", onclick: move |_| rows.set(make_rows(10_000)), "Create 10,000 rows" }
        button {
            id: "add",
            onclick: move |_| {
                let extra = make_rows(1_000);
                rows.write().extend(extra);
            },
            "Append 1,000 rows"
        }
        button {
            id: "update",
            onclick: move |_| {
                let mut w = rows.write();
                let mut i = 0;
                while i < w.len() {
                    w[i].label.push_str(" !!!");
                    i += 10;
                }
            },
            "Update every 10th row"
        }
        button {
            id: "clear",
            onclick: move |_| {
                rows.set(Vec::new());
                selected.set(None);
            },
            "Clear"
        }
        button {
            id: "swaprows",
            onclick: move |_| {
                let mut w = rows.write();
                if w.len() > 998 {
                    w.swap(1, 998);
                }
            },
            "Swap rows"
        }

        table { class: "table test-data",
            tbody {
                for row in rows.read().iter() {
                    {
                        let id = row.id;
                        let label = row.label.clone();
                        rsx! {
                            tr {
                                key: "{id}",
                                class: if selected() == Some(id) { "danger" } else { "" },
                                td { class: "col-md-1", "{id}" }
                                td { class: "col-md-4",
                                    a { class: "lbl", onclick: move |_| selected.set(Some(id)), "{label}" }
                                }
                                td { class: "col-md-1",
                                    a {
                                        class: "remove",
                                        onclick: move |_| { rows.write().retain(|r| r.id != id); },
                                        span { class: "remove glyphicon glyphicon-remove", "aria-hidden": "true", "x" }
                                    }
                                }
                                td { class: "col-md-6" }
                            }
                        }
                    }
                }
            }
        }
    }
}
