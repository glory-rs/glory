//! Real runtime/UI/list + reference interpreter. No browser/layout/FPS claim.
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use glory_render::profile::{Profile, Snapshot};
    use glory_ui::{App, list, text};
    use std::cell::Cell;
    use std::hint::black_box;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    fn phases(rows: usize, kind: &str, interval: &str, repetition: usize, snapshot: Snapshot) {
        for (phase, sample) in snapshot.iter().filter(|(_, sample)| sample.calls > 0) {
            println!(
                "{{\"type\":\"phase\",\"rows\":{rows},\"kind\":\"{kind}\",\"interval\":\"{interval}\",\"repetition\":{repetition},\"phase\":\"{}\",\"calls\":{},\"units\":{},\"inclusive_ns\":{},\"max_ns\":{},\"invalid_clock\":{}}}",
                phase.name(),
                sample.calls,
                sample.units,
                sample.elapsed.as_nanos(),
                sample.max.as_nanos(),
                sample.invalid_clock
            );
        }
    }
    fn run(rows: usize, jump: bool, enabled: bool, repetition: usize) {
        let app = App::memory();
        let profile = enabled.then(Profile::native);
        app.set_profile(profile.clone());
        let position = app.scope().cage(0.);
        let reads = Rc::new(Cell::new(0usize));
        let keys = reads.clone();
        let kind = if jump { "jump" } else { "same-window" };
        let cold = Instant::now();
        app.mount(
            list(
                rows,
                move |index| {
                    keys.set(keys.get() + 1);
                    index
                },
                |_, key| text().content(format!("row:{key}")),
            )
            .position(position)
            .height(600.)
            .row_height(24.)
            .overscan(4),
        )
        .unwrap();
        let cold_ns = cold.elapsed().as_nanos();
        if let Some(profile) = &profile {
            phases(rows, kind, "cold", repetition, profile.snapshot());
        }
        let target = |step: usize| {
            if jump {
                ((step * 7919) % (rows - 100)) as f64 * 24.
            } else {
                step as f64 * 0.001
            }
        };
        for step in 1..=100 {
            position.revise(|value| *value = target(step));
            app.flush().unwrap();
        }
        if let Some(profile) = &profile {
            assert!(profile.reset());
        }
        let initial_reads = reads.get();
        let mut frame_time = Duration::ZERO;
        let (mut max_nodes, mut max_graph) = (0, 0);
        let started = Instant::now();
        for step in 101..=1100 {
            let frame = Instant::now();
            position.revise(|value| *value = target(step));
            app.flush().unwrap();
            frame_time += frame.elapsed();
            max_nodes = max_nodes.max(app.stats().nodes);
            max_graph = max_graph.max(app.runtime_stats().node_slots);
            black_box(app.stats().pending);
        }
        let wall = started.elapsed();
        assert!(max_nodes <= 70 && max_graph < 200);
        let key_reads = reads.get() - initial_reads;
        assert!(key_reads <= 33_000);
        println!(
            "{{\"type\":\"run\",\"rows\":{rows},\"kind\":\"{kind}\",\"profile\":{enabled},\"repetition\":{repetition},\"frames\":1000,\"cold_ns\":{cold_ns},\"frame_ns\":{},\"wall_with_stats_ns\":{},\"key_reads\":{key_reads},\"max_nodes\":{max_nodes},\"max_graph\":{max_graph}}}",
            frame_time.as_nanos(),
            wall.as_nanos()
        );
        if let Some(profile) = &profile {
            phases(rows, kind, "steady", repetition, profile.snapshot());
        }
        app.clear().unwrap();
        assert_eq!(app.stats().nodes, 0);
        assert_eq!(app.runtime_stats().tasks, 0);
    }
    pub fn main() {
        for rows in [1_000_000, 100_000_000] {
            for jump in [false, true] {
                for repetition in 0..3 {
                    for enabled in if repetition % 2 == 0 {
                        [false, true]
                    } else {
                        [true, false]
                    } {
                        run(rows, jump, enabled, repetition);
                    }
                }
            }
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    native::main();
}
#[cfg(target_arch = "wasm32")]
fn main() {}
