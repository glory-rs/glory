//! CPU/control-flow baseline, not a browser frame-rate benchmark.
use std::hint::black_box;
use std::time::Instant;

use glory_ui::list::{Cache, Heights, Layout, RowHost};

#[derive(Default)]
struct Host {
    live: usize,
    peak: usize,
    created: usize,
    disposed: usize,
}

impl RowHost for Host {
    type Key = usize;
    type Row = usize;
    fn key(&mut self, index: usize) -> usize {
        black_box(index)
    }
    fn mount(&mut self, index: usize, _: &usize) -> usize {
        self.live += 1;
        self.created += 1;
        self.peak = self.peak.max(self.live);
        index
    }
    fn update(&mut self, _: usize, _: &usize, _: &mut usize) {}
    fn place(&mut self, row: &usize, before: Option<&usize>) {
        black_box((row, before));
    }
    fn dispose(&mut self, _: usize) {
        self.live -= 1;
        self.disposed += 1;
    }
}

fn fixed(rows: usize, jumps: bool) {
    let layout = Layout::new(Heights::fixed(rows, 24.0).unwrap(), 4);
    let mut cache = Cache::default();
    let mut host = Host::default();
    let mut reads = 0;
    let steps = 20_000;
    let started = Instant::now();
    for step in 0..steps {
        let index = if jumps {
            step as u64 * 15_485_863 % (rows as u64 - 100)
        } else {
            step as u64
        };
        let window = layout
            .window(black_box(index as f64 * 24.0), 600.0)
            .unwrap();
        reads += cache
            .reconcile(window.render, 1, &mut host)
            .unwrap()
            .keys_read;
    }
    let elapsed = started.elapsed();
    cache.clear(&mut host);
    assert_eq!(host.live, 0);
    println!(
        "fixed rows={rows} jumps={jumps} steps={steps} elapsed_us={} keys_read={reads} peak_rows={} index_slots={} created={} disposed={}",
        elapsed.as_micros(),
        host.peak,
        layout.heights().storage_len(),
        host.created,
        host.disposed
    );
}

fn variable() {
    let rows = 1_000_000;
    let started = Instant::now();
    let mut layout = Layout::new(Heights::variable(rows, 24.0).unwrap(), 4);
    let build = started.elapsed();
    let started = Instant::now();
    for step in 0..100_000 {
        let index = step * 7919 % rows;
        layout
            .measure(index, 20.0 + (step % 61) as f64, None)
            .unwrap();
        let offset = layout.heights().prefix(index).unwrap();
        black_box(layout.window(offset, 600.0).unwrap());
    }
    println!(
        "variable rows={rows} steps=100000 build_us={} measure_and_lookup_us={} index_slots={}",
        build.as_micros(),
        started.elapsed().as_micros(),
        layout.heights().storage_len()
    );
}

fn main() {
    fixed(1_000_000, false);
    fixed(100_000_000, false);
    fixed(1_000_000, true);
    fixed(100_000_000, true);
    variable();
}
