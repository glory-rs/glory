//! Browser acceptance of public structure APIs; no application RowHost or cleanup.
use g::{Cage, display::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::prelude::*;
struct Row {
    id: usize,
    label: String,
}
#[g::component(g)]
fn Child(scope: &Scope, initial: usize) -> impl IntoView {
    let count = scope.cage(initial);
    g::view! {<button type="button" class="child-count" on:click={move||count.revise(|v|*v+=1)}>{count}</button>}
}
#[g::component(g)]
fn Frame(children: ChildContent) -> impl IntoView {
    g::view! {<div class="frame"><Content value={children}/></div>}
}
struct Demo {
    app: App,
    rows: KeyedList<usize, Row>,
    flag: Cage<bool>,
    factories: Rc<Cell<usize>>,
}
thread_local! {static DEMO:RefCell<Option<Demo>>=const{RefCell::new(None)};}
fn rows(ids: &[usize]) -> Vec<Row> {
    ids.iter()
        .map(|id| Row {
            id: *id,
            label: format!("row {id}"),
        })
        .collect()
}
pub fn main() {
    let app = App::at_id("structural").unwrap();
    let scope = app.scope();
    let source = scope.keyed_list(Vec::<Row>::new(), |r| r.id).unwrap();
    let flag = scope.cage(false);
    let factories = Rc::new(Cell::new(0));
    let calls = factories.clone();
    let config = g::view! {
        <div class="structure" class:enabled={flag}>
            <span class="initial" class:initial={flag}>"marker"</span>
            <div class="primary"><Each each={source} children={move |row| {
                calls.set(calls.get()+1);let id=*row.key();let label=g::field!(row.label);
                g::view!{<><div class="member"><span class="key">{id}</span><span class="label">{label}</span><Child initial={0}/></div><span class="tail">{id}</span></>}
            }}/></div>
            <div class="secondary"><Each each={source} children={move |row| {
                let id=*row.key();let label=g::field!(row.label);
                if id%2==1 {Some(g::view!{<span class="mirror">{label}</span>})}else{None}
            }}/></div>
            <Show when={move || flag.get()}><Frame><Child initial={9}/></Frame></Show>
            <div class="stable"><Child initial={99}/></div>
        </div>
    };
    app.render(config).unwrap();
    DEMO.with(|d| {
        *d.borrow_mut() = Some(Demo {
            app,
            rows: source,
            flag,
            factories,
        })
    });
}
#[wasm_bindgen]
pub fn structural_action(action: usize) {
    DEMO.with(|d| {
        let d = d.borrow();
        let d = d.as_ref().unwrap();
        match action {
            0 => d.rows.replace(rows(&[1, 2, 3])).unwrap(),
            1 => d.rows.swap(0, 2).unwrap(),
            2 => {
                d.rows.update_at(0, |r| r.label.push('!')).unwrap();
            }
            3 => d.rows.replace(rows(&[3, 2, 1])).unwrap(),
            4 => {
                d.rows.remove_key(&3).unwrap();
            }
            5 => d.rows.extend(rows(&[3])).unwrap(),
            6 => d.rows.clear().unwrap(),
            7 => d.flag.revise(|v| *v = !*v),
            8 => d.rows.replace((1..=300).map(|id|Row{id,label:format!("{}{id}","🙂汉".repeat(1000))}).collect()).unwrap(),
            _ => unreachable!(),
        }
        d.app.flush().unwrap();
    });
}
#[wasm_bindgen]
pub fn structural_stats() -> String {
    DEMO.with(|d|{let d=d.borrow();let d=d.as_ref().unwrap();let h=d.app.stats();let r=d.app.runtime().stats();
        format!("{{\"instances\":{},\"targets\":{},\"records\":{},\"graph\":{},\"owners\":{},\"fields\":{},\"entry_keys\":{},\"structures\":{},\"factories\":{}}}",h.instances,h.targets,h.records,r.nodes,r.owners,r.fields,r.entry_keys,r.structural_runs,d.factories.get())
    })
}
#[wasm_bindgen]
pub fn structural_close() {
    DEMO.with(|d| {
        if let Some(d) = d.borrow_mut().take() {
            d.app.close().unwrap()
        }
    })
}
#[wasm_bindgen]
pub fn structural_fault() -> String {
    static ROW: Template = Template::new(&[span(&[], &[field(Field::new("id"))])]);
    let root = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .create_element("div")
        .unwrap();
    let app = App::new(root.clone()).unwrap();
    let scope = app.scope();
    let source = scope.keyed_list(vec![1usize, 2, 3], |id| *id).unwrap();
    let failed=app.render(g::view!{<div><Each each={source} children={move |row|view(&ROW).bind(Field::new(if *row.key()==2 {"missing"}else{"id"}),*row.key())}/></div>}).is_err();
    let stopped = app.flush().is_err();
    app.close().unwrap();
    let h = app.stats();
    let r = app.runtime().stats();
    format!(
        "{{\"failed\":{failed},\"stopped\":{stopped},\"instances\":{},\"records\":{},\"nodes\":{},\"owners\":{},\"children\":{}}}",
        h.instances,
        h.records,
        r.nodes,
        r.owners,
        usize::from(root.has_child_nodes())
    )
}
#[wasm_bindgen]
pub fn structural_flag_fault() -> String {
    let root = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .create_element("div")
        .unwrap();
    let app = App::new(root.clone()).unwrap();
    let scope = app.scope();
    let active = scope.cage(false);
    app.render(g::view!{<><span class="first" class:active={active}/><span class="second" class:active={active}/></>}).unwrap();
    let before = app.stats().applied;
    active.revise(|v| *v = true);
    let failed = app.flush().is_err();
    let applied = app.stats().applied - before;
    let first = root
        .query_selector(".first")
        .unwrap()
        .unwrap()
        .get_attribute("class")
        .unwrap()
        .split_ascii_whitespace()
        .any(|s| s == "active");
    let second = root
        .query_selector(".second")
        .unwrap()
        .unwrap()
        .get_attribute("class")
        .unwrap()
        .split_ascii_whitespace()
        .any(|s| s == "active");
    app.close().unwrap();
    let r = app.runtime().stats();
    format!(
        "{{\"failed\":{failed},\"applied\":{applied},\"first\":{first},\"second\":{second},\"nodes\":{},\"owners\":{}}}",
        r.nodes, r.owners
    )
}
