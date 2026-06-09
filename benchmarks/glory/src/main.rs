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

use glory::reflow::{Bond, Cage};
use glory::web::events;
use glory::web::holders::BrowserHolder;
use glory::web::widgets::*;
use glory::widgets::Each;
use glory::*;

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
    fn next_u64(&self) -> u64 {
        let s = self
            .0
            .get()
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0.set(s);
        s >> 33
    }
    fn below(&self, n: usize) -> usize {
        (self.next_u64() as usize) % n
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
        (0..count)
            .map(|_| {
                let id = self.next_id.get();
                self.next_id.set(id + 1);
                let label = format!(
                    "{} {} {}",
                    ADJECTIVES[self.rng.below(ADJECTIVES.len())],
                    COLOURS[self.rng.below(COLOURS.len())],
                    NOUNS[self.rng.below(NOUNS.len())],
                );
                Row {
                    id,
                    label: Cage::new(label),
                }
            })
            .collect()
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
                rows.revise(|mut v| *v = new_rows.clone());
            }
        };

        button().id("run").text("Create 1,000 rows").on(events::click, mk(self, 1_000)).show_in(ctx);
        button().id("runlots").text("Create 10,000 rows").on(events::click, mk(self, 10_000)).show_in(ctx);

        let bench = self.clone_handles();
        button().id("add").text("Append 1,000 rows").on(events::click, move |_| {
            let extra = bench.build_rows(1_000);
            bench.rows.revise(|mut v| v.extend(extra.clone()));
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
        // Each row is its own `RowWidget` so it gets its own `Scope` (and
        // `Owner`). The widget's `build` ties the row's `label` cage to that
        // scope, so when the row is detached (clear / remove / replace) the
        // owner drops and the label cage is reclaimed instead of leaking.
        let rows = self.rows.clone();
        let selected = self.selected.clone();

        table()
            .class("table test-data")
            .fill(tbody().fill(Each::from_vec(
                rows.clone(),
                |row: &Row| row.id,
                move |row| RowWidget {
                    id: row.id,
                    label: row.label,
                    selected,
                    rows,
                },
            )))
            .show_in(ctx);
    }
}

/// One table row. Owning its own `Scope` is what lets glory reclaim the
/// per-row `label` cage on detach.
#[derive(Debug)]
struct RowWidget {
    id: usize,
    label: Cage<String>,
    selected: Cage<Option<usize>>,
    rows: Cage<Vec<Row>>,
}

impl Widget for RowWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let id = self.id;
        let label = self.label;

        // Reclaim this cage when the row's scope drops (clear / remove /
        // replace). This is the idiomatic ownership pattern now that owned
        // cages are actually freed; see crates/core/src/reflow/cage.rs.
        ctx.owner().own_cage(label);

        let sel = self.selected;
        let is_selected = Bond::new(move || *sel.get() == Some(id));
        let select = move |_| sel.revise(|mut s| *s = Some(id));

        let rows = self.rows;
        let remove = move |_| rows.revise(|mut v| v.retain(|r| r.id != id));

        tr().toggle_class("danger", is_selected)
            .fill(td().class("col-md-1").text(id.to_string()))
            .fill(td().class("col-md-4").fill(a().class("lbl").on(events::click, select).text(label)))
            .fill(td().class("col-md-1").fill(
                a().class("remove").on(events::click, remove).fill(
                    span().class("remove glyphicon glyphicon-remove").attr("aria-hidden", "true"),
                ),
            ))
            .fill(td().class("col-md-6"))
            .show_in(ctx);
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
