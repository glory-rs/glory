//! JSON wire-format baseline for the command stream (M7 P2 评估).
//!
//! Measures serialize/deserialize cost of representative command batches
//! crossing the desktop IPC boundary. Decision input for whether a
//! sledgehammer-style binary encoding is worth its complexity:
//! compare these numbers against a frame budget (16.6ms) and typical
//! batch sizes (a large initial mount ≈ 1000 rows × 4 commands).
//!
//! Run: `cargo bench -p glory-core --bench command_wire`

use criterion::{Criterion, criterion_group, criterion_main};
use glory_core::renderer::{Command, CommandInsertPosition};

/// A realistic mount batch: each row = Create + SetAttribute + SetText + Insert.
fn typical_batch(rows: u64) -> Vec<Command> {
    let mut batch = Vec::with_capacity(rows as usize * 4);
    for i in 1..=rows {
        batch.push(Command::Create {
            id: i,
            name: "li".into(),
            is_void: false,
        });
        batch.push(Command::SetAttribute {
            id: i,
            name: "data-id".into(),
            value: i.to_string(),
        });
        batch.push(Command::SetText {
            id: i,
            value: format!("row {i} with some representative text content"),
        });
        batch.push(Command::Insert {
            parent: 0,
            child: i,
            position: CommandInsertPosition::Tail,
        });
    }
    batch
}

/// A typical event-driven patch: one text update.
fn patch_batch() -> Vec<Command> {
    vec![Command::SetText {
        id: 42,
        value: "Count: 1337".into(),
    }]
}

fn bench_wire(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("command_wire_json");
    group.sample_size(40);

    for rows in [100u64, 1000] {
        let batch = typical_batch(rows);
        let json = serde_json::to_string(&batch).unwrap();
        group.bench_function(format!("serialize_{rows}_rows_{}_cmds", batch.len()), |bencher| {
            bencher.iter(|| serde_json::to_string(std::hint::black_box(&batch)).unwrap())
        });
        group.bench_function(format!("deserialize_{rows}_rows_{}_bytes", json.len()), |bencher| {
            bencher.iter(|| serde_json::from_str::<Vec<Command>>(std::hint::black_box(&json)).unwrap())
        });
    }

    let patch = patch_batch();
    let patch_json = serde_json::to_string(&patch).unwrap();
    group.bench_function("serialize_single_patch", |bencher| {
        bencher.iter(|| serde_json::to_string(std::hint::black_box(&patch)).unwrap())
    });
    group.bench_function("deserialize_single_patch", |bencher| {
        bencher.iter(|| serde_json::from_str::<Vec<Command>>(std::hint::black_box(&patch_json)).unwrap())
    });
    group.finish();
}

criterion_group!(benches, bench_wire);
criterion_main!(benches);
