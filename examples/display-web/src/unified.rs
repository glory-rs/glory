//! Mixed HTML/Sapid/function-widget acceptance, sharing a single runtime.
use g::{Cage, Widget, core::Scope, display::App};
use sapid::Button;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::prelude::*;

#[g::component(g)]
fn Custom(scope: &Scope, shared: Cage<usize>) -> impl Widget {
    let own = scope.cage(0usize);
    g::view! {
        <div class="custom">
            <span class="local">{own}</span>
            <Button class="custom-button" label="custom" on_press={move || {
                own.revise(|n| *n += 1); shared.revise(|n| *n += 1);
            }}/>
        </div>
    }
}
#[g::component(g)]
fn Panel(children: g::ui::Children) -> impl Widget {
    sapid::card().class("semantic-panel").fill(children)
}
struct DropProbe(Rc<Cell<usize>>);
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
struct Demo {
    app: App,
    shared: Cage<usize>,
    visible: Cage<bool>,
    dropped: Rc<Cell<usize>>,
    rows: g::core::KeyedList<usize, usize>,
}
thread_local! { static DEMO: RefCell<Option<Demo>> = const {RefCell::new(None)}; }
pub fn main() {
    let app = App::at_id("unified").unwrap();
    let scope = app.scope();
    let shared = scope.cage(0usize);
    let visible = scope.cage(true);
    let dropped = Rc::new(Cell::new(0));
    let drops = dropped.clone();
    let rows = scope.keyed_list(vec![1usize, 2], |id| *id).unwrap();
    app.render(g::view! {
        <div class="unified">
            <span class="shared">{shared}</span>
            <sapid::Button class="full" label={shared.map(|n|format!("count {n}"))} on_press={move || shared.revise(|n|*n+=1)}/>
            <Button class="imported" label="imported" on_press={move || shared.revise(|n|*n+=1)}/>
            <Content value={sapid::button().class("content-widget").label("content")}/>
            <Show when={move || visible.get()}>
                <div class="branch">
                    <Custom shared={shared}/>
                    <Button class="owned" label="owned" on_press={let probe=DropProbe(drops.clone()); move || {let _=&probe;shared.revise(|n|*n+=1);}}/>
                    <Button class="dismiss" label="dismiss" on_press={move || visible.revise(|v|*v=false)}/>
                </div>
            </Show>
            <Panel><div class="raw-child"><span>"inside semantic widget"</span><Button class="nested" label="nested" on_press={move || shared.revise(|n|*n+=1)}/></div></Panel>
            <div class="widget-list"><Each each={rows} children={move |row| {
                let id=*row.key();
                g::view! {<Button class="row-button" label={format!("row {id}")}/>}
            }}/></div>
        </div>
    }).unwrap();
    DEMO.with(|d| {
        *d.borrow_mut() = Some(Demo {
            app,
            shared,
            visible,
            dropped,
            rows,
        })
    });
}
#[wasm_bindgen]
pub fn unified_swap() {
    DEMO.with(|d| {
        let d = d.borrow();
        let d = d.as_ref().unwrap();
        d.rows.swap(0, 1).unwrap();
        d.app.flush().unwrap();
    });
}
#[wasm_bindgen]
pub fn unified_toggle() {
    DEMO.with(|d| {
        let d = d.borrow();
        let d = d.as_ref().unwrap();
        d.visible.revise(|v| *v = !*v);
        d.app.flush().unwrap();
    });
}
#[wasm_bindgen]
pub fn unified_stats() -> String {
    DEMO.with(|d| {
        let d = d.borrow();
        let d = d.as_ref().unwrap();
        let r = d.app.runtime().stats();
        format!(
            "{{\"count\":{},\"drops\":{},\"owners\":{},\"nodes\":{}}}",
            d.shared.get(),
            d.dropped.get(),
            r.owners,
            r.nodes
        )
    })
}
#[wasm_bindgen]
pub fn unified_close() -> String {
    DEMO.with(|d| {
        let d = d.borrow_mut().take().unwrap();
        d.app.close().unwrap();
        let r = d.app.runtime().stats();
        format!(
            "{{\"nodes\":{},\"owners\":{},\"drops\":{}}}",
            r.nodes,
            r.owners,
            d.dropped.get()
        )
    })
}
#[wasm_bindgen]
pub fn unified_reject_nested_button() -> bool {
    let root = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .create_element("div")
        .unwrap();
    let refused = g::web::launch(
        root.clone(),
        g::ui::button()
            .label("outer")
            .fill(g::view! {<button>"inner"</button>}),
    )
    .is_err();
    refused && !root.has_child_nodes() && !root.has_attribute("data-gly-app")
}
