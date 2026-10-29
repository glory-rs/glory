//! One mixed tree used by server, browser adoption and the native engine.
use g::{Cage, Widget, core::KeyedList, ui::Scope};
use sapid::Button;
use std::{cell::Cell, rc::Rc};

pub struct Model {
    pub count: Cage<usize>,
    pub bubbled: Cage<usize>,
    pub visible: Cage<bool>,
    pub rows: KeyedList<usize, usize>,
    pub vector: Cage<Vec<(usize, String)>>,
    pub input: Cage<String>,
    pub drops: Rc<Cell<usize>>,
}
impl Model {
    pub fn new(scope: &Scope) -> Self {
        Self {
            count: scope.cage(0),
            bubbled: scope.cage(0),
            visible: scope.cage(true),
            rows: scope.keyed_list(vec![1usize, 2], |id| *id).unwrap(),
            vector: scope.cage(vec![(1, "one".into()), (2, "two".into())]),
            input: scope.cage("server value".to_owned()),
            drops: Rc::new(Cell::new(0)),
        }
    }
}
struct Probe(Rc<Cell<usize>>);
impl Drop for Probe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

fn opaque_button(count: Cage<usize>) -> impl Widget {
    sapid::button()
        .class("opaque")
        .label("opaque factory")
        .on_press(move || count.revise(|n| *n += 1))
}

#[g::component(g)]
fn Panel(children: g::ui::Children) -> impl Widget {
    sapid::card().class("panel").fill(children)
}

pub fn view(model: &Model) -> impl Widget + use<> {
    let count = model.count;
    let bubbled = model.bubbled;
    let visible = model.visible;
    let rows = model.rows;
    let vector = model.vector;
    let input = model.input;
    let drops = model.drops.clone();
    g::view! {
        <div class="mixed" class:active={visible}>
            <span class="count" title={count.map(|n|format!("count {n}"))}>{count}</span>
            <span class="adjacent">{""}{count}{"tail"}</span>
            <button class="raw" on:click={move ||count.revise(|n|*n+=1)}>"raw button"</button>
            <Button class="semantic" label={count.map(|n|format!("widget {n}"))} on_press={move ||count.revise(|n|*n+=1)}/>
            <{opaque_button(count)}/>
            <div class="click-wrapper" on:click={move ||bubbled.revise(|n|*n+=1)}>
                <{sapid::button()} class="configured" label="configured factory" on_press={move ||count.revise(|n|*n+=1)}/>
            </div>
            <Panel><div class="nested"><span>"nested raw"</span><Button class="nested-button" label="nested widget"/></div></Panel>
            <table><tbody><tr><td><Button class="table-widget" label="table widget"/></td></tr></tbody></table>
            <sapid::TextField class="editor" value={input} on_value_change={move |value|input.revise(|current|*current=value)}/>
            <Show when={move ||visible.get()}>
                <div class="branch"><Button class="owned" label="owned" on_press={let probe=Probe(drops.clone());move ||{let _=&probe;count.revise(|n|*n+=1);}}/></div>
            </Show>
            <div class="rows"><Each each={rows} children={move |row|{
                let id=*row.key();g::view!{<div class="row"><span>{id}</span><Button class="row-button" label={format!("row {id}")}/></div>}
            }}/></div>
            <div class="vector"><Each each={vector} key={|row|row.0} children={move |row|{
                let id=*row.key();let label=row.field("label",|row|&row.1,|row|&mut row.1);
                g::view!{<div class="vector-row"><span class="vector-label">{label}</span><span class="vector-index">{row.index}</span><sapid::TextField class={format!("vector-{id}")} default_value={format!("row {id}")}/></div>}
            }}/></div>
        </div>
    }
}
