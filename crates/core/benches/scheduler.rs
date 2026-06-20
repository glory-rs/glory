//! Scheduler and command-buffer hotspot benchmarks.
//!
//! Run: `cargo bench -p glory-core --features web-ssr --bench scheduler`

use std::borrow::Cow;
use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use glory_core::config::GloryConfig;
use glory_core::reflow::Cage;
use glory_core::renderer::{CommandRenderer, InsertPosition, Renderer};
use glory_core::web::holders::ServerHolder;
use glory_core::web::widgets::{div, span};
use glory_core::{Holder, Scope, Widget};

#[derive(Debug)]
struct TextRow {
    value: Cage<usize>,
}

impl Widget for TextRow {
    fn build(&mut self, ctx: &mut Scope) {
        span().text(self.value).show_in(ctx);
    }
}

#[derive(Debug)]
struct SubscriberGrid {
    rows: usize,
    value: Cage<usize>,
}

impl Widget for SubscriberGrid {
    fn build(&mut self, ctx: &mut Scope) {
        let rows: Vec<_> = (0..self.rows).map(|_| TextRow { value: self.value }).collect();
        div().fill(rows).show_in(ctx);
    }
}

fn mount_subscribers(rows: usize) -> (Cage<usize>, ServerHolder) {
    let value = Cage::new(0);
    let holder = ServerHolder::new(GloryConfig::default(), "/").mount(SubscriberGrid { rows, value });
    (value, holder)
}

fn fill_command_buffer(rows: u64, redundant_text_writes: u64, coalesce: bool) -> CommandRenderer {
    let renderer = CommandRenderer::new();
    renderer.queue().set_coalesce(coalesce);
    let root = renderer.create_element(Cow::Borrowed("ul"), false);

    for id in 0..rows {
        let row = renderer.create_element(Cow::Borrowed("li"), false);
        renderer.set_attribute(&row, Cow::Borrowed("data-id"), Cow::Owned(id.to_string()));
        for write in 0..redundant_text_writes {
            renderer.set_text(&row, Cow::Owned(format!("row {id}:{write}")));
        }
        renderer.insert_child(&root, &row, InsertPosition::Tail);
    }

    renderer
}

fn bench_scheduler_subscribers(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler_shared_signal");
    group.sample_size(20);

    for rows in [100_usize, 1000] {
        group.bench_function(format!("revise_{rows}_subscribers"), |b| {
            b.iter_batched(
                || mount_subscribers(rows),
                |(value, _holder)| {
                    value.revise(|mut value| *value += 1);
                    black_box(());
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_command_queue_flush(c: &mut Criterion) {
    let mut group = c.benchmark_group("command_queue_flush");
    group.sample_size(20);

    for rows in [100_u64, 1000] {
        group.bench_function(format!("take_raw_{rows}_rows"), |b| {
            b.iter_batched(
                || fill_command_buffer(rows, 1, false),
                |renderer| black_box(renderer.take_batch()),
                BatchSize::SmallInput,
            );
        });

        group.bench_function(format!("take_coalesced_{rows}_rows_3_text_writes"), |b| {
            b.iter_batched(
                || fill_command_buffer(rows, 3, true),
                |renderer| black_box(renderer.take_batch()),
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_scheduler_subscribers, bench_command_queue_flush);
criterion_main!(benches);
