#[path = "../../data.rs"]
mod data;
use dioxus::prelude::*;
use std::{cell::RefCell, rc::Rc};
fn main() {
    dioxus::launch(App);
}
#[component]
fn App() -> Element {
    let mut rows = use_signal(Vec::<data::Row>::new);
    let mut selected = use_signal(|| None::<usize>);
    let generator = use_hook(|| Rc::new(RefCell::new(data::Generator::default())));
    let make = move |count| generator.borrow_mut().rows(count);
    let make_lots = make.clone();
    let make_add = make.clone();
    rsx! {
        button { class: "bench-run", onclick: move |_| rows.set(make(1000)), "Create 1,000 rows" }
        button { class: "bench-runlots", onclick: move |_| rows.set(make_lots(10000)), "Create 10,000 rows" }
        button { class: "bench-add", onclick: move |_| rows.write().extend(make_add(1000)), "Append 1,000 rows" }
        button { class: "bench-update", onclick: move |_| { for row in rows.write().iter_mut().step_by(10) { row.label.push_str(" !!!"); } }, "Update every 10th row" }
        button { class: "bench-clear", onclick: move |_| { rows.set(Vec::new()); selected.set(None); }, "Clear" }
        button { class: "bench-swaprows", onclick: move |_| { let mut rows = rows.write(); if rows.len() > 998 { rows.swap(1, 998); } }, "Swap rows" }
        div { role: "table", class: "bench-table", div { class: "bench-body",
            for row in rows.read().iter().cloned() {
                div { key: "{row.id}", role: "row", class: if selected() == Some(row.id) { "bench-row danger" } else { "bench-row" },
                    div { role: "cell", class: "bench-cell col-md-1", span { "{row.id}" } }
                    div { role: "cell", class: "bench-cell col-md-4", button { class: "lbl", onclick: move |_| selected.set(Some(row.id)), "{row.label}" } }
                    div { role: "cell", class: "bench-cell col-md-1", button { class: "remove", onclick: move |_| rows.write().retain(|current| current.id != row.id), "x" } }
                    div { role: "cell", class: "bench-cell col-md-6" }
                }
            }
        } }
    }
}
