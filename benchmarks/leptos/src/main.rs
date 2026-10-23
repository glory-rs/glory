#[path = "../../data.rs"]
mod data;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
#[derive(Clone)]
struct Row {
    id: usize,
    label: ArcRwSignal<String>,
}
fn main() {
    let root = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .get_element_by_id("main")
        .unwrap();
    leptos::mount::mount_to(root.unchecked_into(), App).forget();
}
#[component]
fn App() -> impl IntoView {
    let rows = RwSignal::new(Vec::<Row>::new());
    let selected = RwSignal::new(None::<usize>);
    let generator = StoredValue::new(data::Generator::default());
    let make = move |count| {
        generator
            .try_update_value(|generator| generator.rows(count))
            .unwrap()
            .into_iter()
            .map(|row| Row {
                id: row.id,
                label: ArcRwSignal::new(row.label),
            })
            .collect::<Vec<_>>()
    };
    view! {
        <button class="bench-run" on:click=move |_| rows.set(make(1000))>"Create 1,000 rows"</button>
        <button class="bench-runlots" on:click=move |_| rows.set(make(10000))>"Create 10,000 rows"</button>
        <button class="bench-add" on:click=move |_| { let next = make(1000); rows.update(|rows| rows.extend(next)); }>"Append 1,000 rows"</button>
        <button class="bench-update" on:click=move |_| rows.with(|rows| { for row in rows.iter().step_by(10) { row.label.update(|label| label.push_str(" !!!")); } })>"Update every 10th row"</button>
        <button class="bench-clear" on:click=move |_| { rows.set(Vec::new()); selected.set(None); }>"Clear"</button>
        <button class="bench-swaprows" on:click=move |_| rows.update(|rows| if rows.len() > 998 { rows.swap(1, 998); })>"Swap rows"</button>
        <div role="table" class="bench-table"><div class="bench-body">
            <For each=move || rows.get() key=|row| row.id children=move |row| {
                let id = row.id;
                view! {
                    <div role="row" class="bench-row" class:danger=move || selected.get() == Some(id)>
                        <div role="cell" class="bench-cell col-md-1"><span>{id}</span></div>
                        <div role="cell" class="bench-cell col-md-4"><button class="lbl" on:click=move |_| selected.set(Some(id))>{move || row.label.get()}</button></div>
                        <div role="cell" class="bench-cell col-md-1"><button class="remove" on:click=move |_| rows.update(|rows| rows.retain(|row| row.id != id))>"x"</button></div>
                        <div role="cell" class="bench-cell col-md-6"></div>
                    </div>
                }
            }/>
        </div></div>
    }
}
