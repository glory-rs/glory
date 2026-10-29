//! The same DSL/Widget tree runs in a live server stream and final hydration.
use g::{Cage, Widget, core::KeyedList, ui::Scope};
use sapid::{Button, TextField};
use std::{cell::Cell, rc::Rc};
pub struct Model {
    pub count: Cage<usize>,
    pub visible: Cage<bool>,
    pub suspended: Cage<bool>,
    pub input: Cage<String>,
    pub rows: KeyedList<usize, usize>,
    pub vector: Cage<Vec<(usize, String)>>,
    pub drops: Rc<Cell<usize>>,
}
impl Model {
    pub fn new(scope: &Scope, stage: usize) -> Self {
        Self {
            count: scope.cage(stage),
            visible: scope.cage(stage % 2 == 1),
            suspended: scope.cage(stage == 0),
            input: scope.cage(
                if stage == 3 {
                    "server final"
                } else {
                    "server initial"
                }
                .into(),
            ),
            rows: scope
                .keyed_list(
                    if stage == 3 {
                        vec![2, 1]
                    } else {
                        vec![1, 2, 4]
                    },
                    |id| *id,
                )
                .unwrap(),
            vector: scope.cage(if stage == 3 {
                vec![(2, "two 3".into()), (1, "one 3".into())]
            } else {
                vec![(1, "one 0".into()), (2, "two 0".into())]
            }),
            drops: Rc::new(Cell::new(0)),
        }
    }
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn advance(&self, stage: usize) {
        self.vector.revise(|rows| {
            if stage == 1 {
                rows.swap(0, 1);
            }
            for (id, label) in rows {
                *label = format!("{} {stage}", if *id == 1 { "one" } else { "two" });
            }
        });
        self.count.revise(|value| *value = stage);
        self.visible.revise(|value| *value = stage % 2 == 1);
        self.suspended.revise(|value| *value = stage == 2);
        self.input.revise(|value| {
            *value = if stage == 3 {
                "server final"
            } else {
                "server update"
            }
            .into()
        });
        self.rows
            .replace(match stage {
                1 => vec![2, 1, 3],
                2 => vec![3, 2, 1],
                _ => vec![2, 1],
            })
            .unwrap();
    }
}
struct Probe(Rc<Cell<usize>>);
impl Drop for Probe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
const BODY: g::display::Outlet = g::display::Outlet::new("body");
static CONTAINER: g::display::Template = g::display::Template::new(&[g::html::view::div(
    &[
        g::html::view::class("named-container"),
        g::html::view::children(BODY),
    ],
    &[g::html::view::span(
        &[],
        &[g::html::view::text("static prefix")],
    )],
)]);
fn container(count: Cage<usize>) -> impl Widget {
    g::display::view(&CONTAINER).child(BODY,g::display::lazy(move||g::view!{<><span class="named-value">{count}</span><Button label="named widget"/></>}))
}
fn opaque(count: Cage<usize>) -> impl Widget {
    sapid::button()
        .class("factory")
        .label("factory")
        .on_press(move || count.revise(|value| *value += 1))
}
struct Suspended {
    hidden: Cage<bool>,
    count: Cage<usize>,
}
impl Widget for Suspended {
    fn build(&mut self, scope: &Scope) -> g::ui::Result {
        let count = self.count;
        let hidden = self.hidden;
        let mounted=scope.mount_with(move||g::view!{<div class="suspended"><span>{count}</span><TextField class="suspended-editor" default_value="suspended draft"/></div>})?;
        scope.structural(move || {
            if hidden.get() {
                mounted.suspend().unwrap();
            } else {
                mounted.place_before(None).unwrap();
            }
        });
        Ok(())
    }
}
pub fn view(model: &Model) -> impl Widget + use<> {
    let count = model.count;
    let visible = model.visible;
    let hidden = model.suspended;
    let input = model.input;
    let rows = model.rows;
    let vector = model.vector;
    let drops = model.drops.clone();
    g::view! {<div class="stream-mixed" class:active={visible}>
        <span class="value" title={count.map(|value|format!("value {value}"))}>{""}{count}{"tail"}</span>
        <button class="raw" on:click={move||count.revise(|value|*value+=1)}>"raw"</button>
        <{opaque(count)}/>
        <TextField class="editor" value={input} on_value_change={move|next|input.revise(|value|*value=next)}/>
        <{container(count)}/>
        <Show when={move||visible.get()}><div class="branch"><button class="branch-raw" on:click={let probe=Probe(drops.clone());move||{let _=&probe;count.revise(|value|*value+=1);}}>"branch"</button><Button label="branch widget"/></div></Show>
        <Suspended init={Suspended{hidden,count}}/>
        <div class="rows"><Each each={rows} children={move|row|{let id=*row.key();g::view!{<div class="row"><span>{id}</span><TextField class={format!("row-{id}")} default_value={format!("row {id}")}/></div>}}}/></div>
        <table><tbody><Each each={rows} children={move|row|{let id=*row.key();g::view!{<tr><td><span>{id}</span><Button label="cell widget"/></td></tr>}}}/></tbody></table>
        <div class="vector"><Each each={vector} key={|row|row.0} children={move|row|{
            let id=*row.key();let label=row.field("label",|row|&row.1,|row|&mut row.1);
            g::view!{<div class="vector-row"><span class="vector-label">{label}</span><span class="vector-index">{row.index}</span><TextField class={format!("vector-{id}")} default_value={format!("vector {id}")}/></div>}
        }}/></div>
    </div>}
}
