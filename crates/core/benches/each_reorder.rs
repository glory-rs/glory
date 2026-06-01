//! Reorder benchmarks for the `Each` keyed-list widget.
//!
//! Stresses the LIS-based patch path against representative workloads:
//! full reverse, deterministic shuffle, head-prepend, tail-append, and
//! middle-removal. Run via `cargo bench -p glory-core --features web-ssr
//! --bench each_reorder`.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use glory_core::config::GloryConfig;
use glory_core::reflow::Cage;
use glory_core::web::holders::ServerHolder;
use glory_core::web::widgets::{li, ul};
use glory_core::widgets::Each;
use glory_core::{Holder, Scope, Widget};

#[derive(Debug)]
struct ListWidget {
    items: Cage<Vec<String>>,
}

impl Widget for ListWidget {
    fn build(&mut self, ctx: &mut Scope) {
        ul().fill(Each::from_vec(self.items, |s| s.clone(), |s| li().text(s.clone())))
            .show_in(ctx);
    }
}

fn make_initial(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("k{i}")).collect()
}

fn make_holder() -> ServerHolder {
    ServerHolder::new(GloryConfig::default(), "/")
}

fn bench_reverse(c: &mut Criterion) {
    let mut group = c.benchmark_group("each_reverse");
    for &n in &[10_usize, 100, 1000] {
        group.bench_function(format!("n={n}"), |b| {
            b.iter_with_setup(
                || {
                    let initial = make_initial(n);
                    let items = Cage::new(initial.clone());
                    let holder = make_holder().mount(ListWidget { items });
                    (items, holder)
                },
                |(items, _holder)| {
                    items.revise(|mut v| v.reverse());
                    black_box(());
                },
            );
        });
    }
    group.finish();
}

fn bench_shuffle(c: &mut Criterion) {
    let mut group = c.benchmark_group("each_shuffle");
    for &n in &[10_usize, 100, 1000] {
        group.bench_function(format!("n={n}"), |b| {
            b.iter_with_setup(
                || {
                    let initial = make_initial(n);
                    // stride coprime to n for a deterministic spread
                    let stride = if n % 7 == 0 { 11 } else { 7 };
                    let shuffled: Vec<String> = (0..n).map(|i| initial[(i * stride) % n].clone()).collect();
                    let items = Cage::new(initial);
                    let holder = make_holder().mount(ListWidget { items });
                    (items, shuffled, holder)
                },
                |(items, shuffled, _holder)| {
                    items.revise(|mut v| *v = shuffled);
                    black_box(());
                },
            );
        });
    }
    group.finish();
}

fn bench_prepend_one(c: &mut Criterion) {
    let mut group = c.benchmark_group("each_prepend_one");
    for &n in &[10_usize, 100, 1000] {
        group.bench_function(format!("n={n}"), |b| {
            b.iter_with_setup(
                || {
                    let items = Cage::new(make_initial(n));
                    let holder = make_holder().mount(ListWidget { items });
                    (items, holder)
                },
                |(items, _holder)| {
                    items.revise(|mut v| v.insert(0, "new-head".to_string()));
                    black_box(());
                },
            );
        });
    }
    group.finish();
}

fn bench_append_one(c: &mut Criterion) {
    let mut group = c.benchmark_group("each_append_one");
    for &n in &[10_usize, 100, 1000] {
        group.bench_function(format!("n={n}"), |b| {
            b.iter_with_setup(
                || {
                    let items = Cage::new(make_initial(n));
                    let holder = make_holder().mount(ListWidget { items });
                    (items, holder)
                },
                |(items, _holder)| {
                    items.revise(|mut v| v.push("new-tail".to_string()));
                    black_box(());
                },
            );
        });
    }
    group.finish();
}

fn bench_remove_middle(c: &mut Criterion) {
    let mut group = c.benchmark_group("each_remove_middle");
    for &n in &[10_usize, 100, 1000] {
        group.bench_function(format!("n={n}"), |b| {
            b.iter_with_setup(
                || {
                    let items = Cage::new(make_initial(n));
                    let holder = make_holder().mount(ListWidget { items });
                    let mid = n / 2;
                    (items, mid, holder)
                },
                |(items, mid, _holder)| {
                    items.revise(|mut v| {
                        v.remove(mid);
                    });
                    black_box(());
                },
            );
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_reverse,
    bench_shuffle,
    bench_prepend_one,
    bench_append_one,
    bench_remove_middle,
);
criterion_main!(benches);
