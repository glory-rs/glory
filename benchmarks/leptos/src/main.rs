//! Leptos implementation of the js-framework-benchmark "row table" workload.
//!
//! Same DOM contract and control buttons as the glory app, driven by the same
//! `../bench-harness.js`. Each row's label is its own `RwSignal<String>` so the
//! "update every 10th row" op touches only those signals — leptos's
//! fine-grained reactivity patches the matching text nodes directly.

use leptos::prelude::*;

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

#[derive(Clone)]
struct Row {
    id: usize,
    label: RwSignal<String>,
}

fn main() {
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let rows = RwSignal::new(Vec::<Row>::new());
    let selected = RwSignal::new(None::<usize>);
    // Shared, framework-agnostic state kept in `StoredValue` so the make-rows
    // closure can be `Copy` and reused across several button handlers.
    let next_id = StoredValue::new(1usize);
    let rng = StoredValue::new(0x2545_F491_4F6C_DD1D_u64);

    let make_rows = move |count: usize| -> Vec<Row> {
        let mut state = rng.get_value();
        let mut draw = |n: usize| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 33) as usize) % n
        };
        let out = (0..count)
            .map(|_| {
                let id = next_id.get_value();
                next_id.set_value(id + 1);
                let label = format!(
                    "{} {} {}",
                    ADJECTIVES[draw(ADJECTIVES.len())],
                    COLOURS[draw(COLOURS.len())],
                    NOUNS[draw(NOUNS.len())],
                );
                Row {
                    id,
                    label: RwSignal::new(label),
                }
            })
            .collect();
        rng.set_value(state);
        out
    };

    view! {
        <button id="run" on:click=move |_| rows.set(make_rows(1_000))>"Create 1,000 rows"</button>
        <button id="runlots" on:click=move |_| rows.set(make_rows(10_000))>"Create 10,000 rows"</button>
        <button id="add" on:click=move |_| {
            let extra = make_rows(1_000);
            rows.update(|v| v.extend(extra));
        }>"Append 1,000 rows"</button>
        <button id="update" on:click=move |_| rows.with(|v| {
            let mut i = 0;
            while i < v.len() {
                v[i].label.update(|s| s.push_str(" !!!"));
                i += 10;
            }
        })>"Update every 10th row"</button>
        <button id="clear" on:click=move |_| { rows.set(Vec::new()); selected.set(None); }>"Clear"</button>
        <button id="swaprows" on:click=move |_| rows.update(|v| {
            if v.len() > 998 { v.swap(1, 998); }
        })>"Swap rows"</button>

        <table class="table test-data">
            <tbody>
                <For
                    each=move || rows.get()
                    key=|row| row.id
                    children=move |row| {
                        let id = row.id;
                        let label = row.label;
                        view! {
                            <tr class:danger=move || selected.get() == Some(id)>
                                <td class="col-md-1">{id}</td>
                                <td class="col-md-4">
                                    <a class="lbl" on:click=move |_| selected.set(Some(id))>
                                        {move || label.get()}
                                    </a>
                                </td>
                                <td class="col-md-1">
                                    <a class="remove" on:click=move |_| rows.update(|v| v.retain(|r| r.id != id))>
                                        <span class="remove glyphicon glyphicon-remove" aria-hidden="true"></span>
                                    </a>
                                </td>
                                <td class="col-md-6"></td>
                            </tr>
                        }
                    }
                />
            </tbody>
        </table>
    }
}
