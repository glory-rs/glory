//! Native (SSR / `ServerHolder`) reproduction of the glory benchmark's
//! create→clear cycle, with NO wasm-bindgen in the loop. If the per-cycle
//! "create 1,000 rows" time degrades here the way it does in the browser, the
//! cost lives in glory's Rust core (view creation / reactivity). If it stays
//! flat, the browser degradation is a wasm-bindgen / externref artifact.
//!
//! Run: cargo run --release --manifest-path benchmarks/native-repro/Cargo.toml

use std::cell::Cell;
use std::time::Instant;

use glory_core::config::GloryConfig;
use glory_core::reflow::{Bond, Cage};
use glory_core::web::holders::ServerHolder;
use glory_core::web::widgets::*;
use glory_core::widgets::Each;
use glory_core::{Holder, Scope, Widget};

const N: usize = 1_000;
const CYCLES: usize = 12;

#[derive(Clone, Debug)]
struct Row {
    id: usize,
    label: Cage<String>,
}

thread_local! {
    static NEXT_ID: Cell<usize> = const { Cell::new(1) };
}

fn make_rows(count: usize) -> Vec<Row> {
    (0..count)
        .map(|_| {
            let id = NEXT_ID.with(|c| {
                let v = c.get();
                c.set(v + 1);
                v
            });
            Row { id, label: Cage::new(format!("row {id}")) }
        })
        .collect()
}

#[derive(Debug)]
struct Bench {
    rows: Cage<Vec<Row>>,
    selected: Cage<Option<usize>>,
}

impl Widget for Bench {
    fn build(&mut self, ctx: &mut Scope) {
        let rows = self.rows;
        let selected = self.selected;
        table()
            .class("table test-data")
            .fill(tbody().fill(Each::from_vec(
                rows,
                |row: &Row| row.id,
                move |row| RowWidget { id: row.id, label: row.label, selected },
            )))
            .show_in(ctx);
    }
}

#[derive(Debug)]
struct RowWidget {
    id: usize,
    label: Cage<String>,
    selected: Cage<Option<usize>>,
}

impl Widget for RowWidget {
    fn build(&mut self, ctx: &mut Scope) {
        let id = self.id;
        let label = self.label;
        ctx.owner().own_cage(label);
        let sel = self.selected;
        let is_selected = Bond::new(move || *sel.get() == Some(id));
        tr().toggle_class("danger", is_selected)
            .fill(td().class("col-md-1").text(id.to_string()))
            .fill(td().class("col-md-4").fill(a().class("lbl").text(label)))
            .fill(td().class("col-md-1").fill(a().class("remove").fill(span().class("remove"))))
            .fill(td().class("col-md-6"))
            .show_in(ctx);
    }
}

fn main() {
    let rows = Cage::new(Vec::<Row>::new());
    let selected = Cage::new(None::<usize>);
    let holder = ServerHolder::new(GloryConfig::default(), "/").mount(Bench { rows, selected });

    println!("cycle,create_us,html_len");
    for _ in 0..CYCLES {
        let new_rows = make_rows(N);
        let started = Instant::now();
        rows.revise(|mut v| *v = new_rows.clone());
        let create_us = started.elapsed().as_micros();

        let html_len = holder.host_node.node().inner_html().len();

        rows.revise(|mut v| v.clear());

        println!("{create_us},{html_len}");
    }
}
