//! Event-handler install and dispatch lookup benchmarks.
//!
//! Run: `cargo bench -p glory-core --bench event_handlers`

use std::borrow::Cow;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use glory_core::renderer::{CommandRenderer, EventData, InsertPosition, Renderer};

fn build_table(rows: u64, attach_handlers: bool) -> (CommandRenderer, u64) {
    let renderer = CommandRenderer::new();
    let root = renderer.create_element(Cow::Borrowed("table"), false);
    let body = renderer.create_element(Cow::Borrowed("tbody"), false);
    renderer.insert_child(&root, &body, InsertPosition::Tail);

    let mut last_row_id = body.id();
    for row_id in 0..rows {
        let row = renderer.create_element(Cow::Borrowed("tr"), false);
        renderer.set_attribute(&row, Cow::Borrowed("data-id"), Cow::Owned(row_id.to_string()));
        renderer.set_text(&row, Cow::Owned(format!("row {row_id}")));
        if attach_handlers {
            renderer.attach_event(
                &row,
                Cow::Borrowed("click"),
                true,
                Box::new(|event| {
                    black_box(event.node_id);
                }),
            );
        }
        renderer.insert_child(&body, &row, InsertPosition::Tail);
        last_row_id = row.id();
    }

    (renderer, last_row_id)
}

fn bench_install(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_handler_install");
    group.sample_size(10);

    for rows in [1_000_u64, 10_000] {
        group.bench_function(format!("build_{rows}_rows_no_handlers"), |b| {
            b.iter(|| {
                let (renderer, _) = build_table(black_box(rows), false);
                black_box(renderer.take_batch())
            })
        });

        group.bench_function(format!("build_{rows}_rows_click_handlers"), |b| {
            b.iter(|| {
                let (renderer, _) = build_table(black_box(rows), true);
                black_box(renderer.take_batch())
            })
        });
    }

    group.finish();
}

fn bench_dispatch_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_dispatch_lookup");
    group.sample_size(40);

    for rows in [1_000_u64, 10_000] {
        let (no_handler_renderer, no_handler_last_id) = build_table(rows, false);
        no_handler_renderer.take_batch();
        let missing_event = EventData::new("click", no_handler_last_id);
        group.bench_function(format!("missing_click_{rows}_rows"), |b| {
            b.iter(|| black_box(no_handler_renderer.dispatch_event(black_box(missing_event.clone()))))
        });

        let (handler_renderer, handler_last_id) = build_table(rows, true);
        handler_renderer.take_batch();
        let click_event = EventData::new("click", handler_last_id);
        group.bench_function(format!("registered_click_{rows}_handlers_last_row"), |b| {
            b.iter(|| black_box(handler_renderer.dispatch_event(black_box(click_event.clone()))))
        });
    }

    group.finish();
}

criterion_group!(benches, bench_install, bench_dispatch_lookup);
criterion_main!(benches);
