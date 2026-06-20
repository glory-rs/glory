//! Glory implementation of the js-framework-benchmark "row table" workload.
//!
//! Renders the canonical benchmark table and exposes the standard control
//! buttons (`#run`, `#runlots`, `#add`, `#update`, `#clear`, `#swaprows`) plus
//! the per-row select / remove anchors. `../bench-harness.js` drives it.
//!
//! Each row owns its own `Cage<String>` label so the "update every 10th row"
//! operation mutates only those rows — glory's fine-grained reactivity patches
//! the touched `<a>` text nodes without re-diffing the whole list.

use std::cell::Cell;
use std::rc::Rc;

use glory::reflow::{Cage, Revisable};
use glory::web::events;
use glory::web::holders::BrowserHolder;
use glory::web::widgets::*;
use glory::widgets::Each;
use glory::*;
use wasm_bindgen::{JsCast, UnwrapThrowExt};

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

/// Small deterministic LCG so labels are reproducible and identical across the
/// glory / leptos / dioxus apps (fair, non-`Math.random` input).
#[derive(Clone, Debug)]
struct Rng(Rc<Cell<u64>>);
impl Rng {
    fn new() -> Self {
        Self(Rc::new(Cell::new(0x2545_F491_4F6C_DD1D)))
    }

    fn state(&self) -> u64 {
        self.0.get()
    }

    fn set_state(&self, state: u64) {
        self.0.set(state);
    }

    fn below(state: &mut u64, n: usize) -> usize {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((*state >> 33) as usize) % n
    }
}

#[derive(Clone, Debug)]
struct Row {
    id: usize,
    label: Cage<String>,
}

fn main() {
    BrowserHolder::new().mount(Bench::new());
}

#[derive(Debug)]
struct Bench {
    rows: Cage<Vec<Row>>,
    selected: Cage<Option<usize>>,
    next_id: Rc<Cell<usize>>,
    rng: Rng,
}

impl Bench {
    fn new() -> Self {
        Self {
            rows: Cage::new(Vec::new()),
            selected: Cage::new(None),
            next_id: Rc::new(Cell::new(1)),
            rng: Rng::new(),
        }
    }

    fn build_rows(&self, count: usize) -> Vec<Row> {
        let mut next_id = self.next_id.get();
        let mut rng_state = self.rng.state();
        let mut rows = Vec::with_capacity(count);

        for _ in 0..count {
            let id = next_id;
            next_id += 1;
            let label = format!(
                "{} {} {}",
                ADJECTIVES[Rng::below(&mut rng_state, ADJECTIVES.len())],
                COLOURS[Rng::below(&mut rng_state, COLOURS.len())],
                NOUNS[Rng::below(&mut rng_state, NOUNS.len())],
            );
            rows.push(Row {
                id,
                label: Cage::new(label),
            });
        }

        self.next_id.set(next_id);
        self.rng.set_state(rng_state);
        rows
    }
}

impl Widget for Bench {
    fn build(&mut self, ctx: &mut Scope) {
        // ---- control buttons ----
        let mk = |this: &Bench, count: usize| {
            let rows = this.rows.clone();
            let bench = this.clone_handles();
            move |_| {
                let new_rows = bench.build_rows(count);
                rows.revise(move |mut v| *v = new_rows);
            }
        };

        button().id("run").text("Create 1,000 rows").on(events::click, mk(self, 1_000)).show_in(ctx);
        button().id("runlots").text("Create 10,000 rows").on(events::click, mk(self, 10_000)).show_in(ctx);

        let bench = self.clone_handles();
        button().id("add").text("Append 1,000 rows").on(events::click, move |_| {
            let extra = bench.build_rows(1_000);
            bench.rows.revise(move |mut v| v.extend(extra));
        }).show_in(ctx);

        let rows = self.rows.clone();
        button().id("update").text("Update every 10th row").on(events::click, move |_| {
            let snapshot = rows.get();
            let mut i = 0;
            while i < snapshot.len() {
                snapshot[i].label.revise(|mut s| s.push_str(" !!!"));
                i += 10;
            }
        }).show_in(ctx);

        let rows = self.rows.clone();
        let selected = self.selected.clone();
        button().id("clear").text("Clear").on(events::click, move |_| {
            rows.revise(|mut v| v.clear());
            selected.revise(|mut s| *s = None);
        }).show_in(ctx);

        let rows = self.rows.clone();
        button().id("swaprows").text("Swap rows").on(events::click, move |_| {
            rows.revise(|mut v| {
                if v.len() > 998 {
                    v.swap(1, 998);
                }
            });
        }).show_in(ctx);

        // ---- the data table ----
        // Each row is still one Glory view, but the static row skeleton is a
        // native DOM subtree managed through `dom_subtree` instead of five
        // nested element views. The row scope owns the label cage, so detach
        // still reclaims per-row reactive state.
        let rows = self.rows.clone();
        let selected = self.selected.clone();

        table()
            .class("table test-data")
            .fill(tbody().fill(Each::from_vec(
                rows.clone(),
                |row: &Row| row.id,
                move |row| row_widget(row.id, row.label, selected, rows),
            )))
            .show_in(ctx);
    }
}

struct RowState {
    id: usize,
    label: Cage<String>,
    selected: Cage<Option<usize>>,
    label_node: web_sys::Element,
}

fn row_widget(id: usize, label: Cage<String>, selected: Cage<Option<usize>>, rows: Cage<Vec<Row>>) -> DomSubtree<RowState> {
    dom_subtree(move |ctx| {
        ctx.owner().own_cage(label);
        label.bind_view(ctx.view_id());
        selected.bind_view(ctx.view_id());

        let row_dom = build_row_dom(id, &label.get_untracked(), *selected.get_untracked() == Some(id));
        let row = row_dom.row;

        let label_node = row_dom.label_anchor.clone();
        let select_anchor = row_dom.label_anchor;
        glory::web::add_event_listener::<web_sys::MouseEvent>(&select_anchor, "click".into(), move |_| {
            glory::reflow::batch(|| selected.revise(|mut s| *s = Some(id)));
        });

        let remove_anchor = row_dom.remove_anchor;
        glory::web::add_event_listener::<web_sys::MouseEvent>(&remove_anchor, "click".into(), move |_| {
            glory::reflow::batch(|| rows.revise(|mut v| v.retain(|r| r.id != id)));
        });

        (
            row,
            RowState {
                id,
                label,
                selected,
                label_node,
            },
        )
    })
    .on_patch(|row, state, _ctx| {
        if state.label.is_revising() {
            let label = state.label.get_untracked();
            state.label_node.set_text_content(Some(label.as_str()));
        }

        if state.selected.is_revising() {
            set_selected_class(row, *state.selected.get_untracked() == Some(state.id));
        }
    })
}

struct RowDom {
    row: web_sys::Element,
    label_anchor: web_sys::Element,
    remove_anchor: web_sys::Element,
}

thread_local! {
    static ROW_TEMPLATE: web_sys::Element = build_row_template();
}

fn build_row_dom(id: usize, label: &str, selected: bool) -> RowDom {
    let row = ROW_TEMPLATE.with(|template| {
        template
            .clone_node_with_deep(true)
            .unwrap_throw()
            .unchecked_into::<web_sys::Element>()
    });

    let id_cell = row.first_element_child().expect("row template must contain id cell");
    id_cell.set_text_content(Some(&id.to_string()));

    let label_cell = id_cell.next_element_sibling().expect("row template must contain label cell");
    let label_anchor = label_cell.first_element_child().expect("row template must contain label anchor");
    label_anchor.set_text_content(Some(label));

    let remove_cell = label_cell
        .next_element_sibling()
        .expect("row template must contain remove cell");
    let remove_anchor = remove_cell
        .first_element_child()
        .expect("row template must contain remove anchor");

    set_selected_class(&row, selected);

    RowDom {
        row,
        label_anchor,
        remove_anchor,
    }
}

fn build_row_template() -> web_sys::Element {
    let row = element("tr");

    let id_cell = element("td");
    id_cell.set_attribute("class", "col-md-1").unwrap_throw();
    row.append_child(&id_cell).unwrap_throw();

    let label_cell = element("td");
    label_cell.set_attribute("class", "col-md-4").unwrap_throw();
    let label_anchor = element("a");
    label_anchor.set_attribute("class", "lbl").unwrap_throw();
    label_cell.append_child(&label_anchor).unwrap_throw();
    row.append_child(&label_cell).unwrap_throw();

    let remove_cell = element("td");
    remove_cell.set_attribute("class", "col-md-1").unwrap_throw();
    let remove_anchor = element("a");
    remove_anchor.set_attribute("class", "remove").unwrap_throw();
    let icon = element("span");
    icon.set_attribute("class", "remove glyphicon glyphicon-remove").unwrap_throw();
    icon.set_attribute("aria-hidden", "true").unwrap_throw();
    icon.set_text_content(Some("x"));
    remove_anchor.append_child(&icon).unwrap_throw();
    remove_cell.append_child(&remove_anchor).unwrap_throw();
    row.append_child(&remove_cell).unwrap_throw();

    let trailing = element("td");
    trailing.set_attribute("class", "col-md-6").unwrap_throw();
    row.append_child(&trailing).unwrap_throw();

    row
}

fn element(name: &str) -> web_sys::Element {
    glory::web::document().create_element(name).unwrap_throw()
}

fn set_selected_class(row: &web_sys::Element, selected: bool) {
    if selected {
        row.set_attribute("class", "danger").unwrap_throw();
    } else {
        row.remove_attribute("class").unwrap_throw();
    }
}

impl Bench {
    /// Cheap clone of the shared reactive handles for use inside `move`
    /// closures (every field is a cheap handle clone).
    fn clone_handles(&self) -> Bench {
        Bench {
            rows: self.rows.clone(),
            selected: self.selected.clone(),
            next_id: self.next_id.clone(),
            rng: self.rng.clone(),
        }
    }
}
