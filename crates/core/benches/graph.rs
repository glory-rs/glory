//! Graph creation, 100 writes/flushes and retirement, with live shape assertions.
use glory_core::Runtime;
use std::{hint::black_box, time::Instant};
fn main() {
    let census = std::env::var_os("GLORY_GRAPH_CENSUS").is_some();
    for (deps, fanout) in [
        (0, 1000),
        (1, 1),
        (1, 1000),
        (1, 10000),
        (2, 1000),
        (4, 1000),
        (32, 1000),
    ] {
        for repetition in 0..9 {
            let runtime = Runtime::new();
            let owner = runtime.owner();
            let start = Instant::now();
            let sources = (0..deps.max(1))
                .map(|_| owner.cage(0_u64))
                .collect::<Vec<_>>();
            for _ in 0..fanout {
                let inputs = sources[..deps].to_vec();
                owner.binding(move || {
                    black_box(inputs.iter().map(|c| c.get()).sum::<u64>());
                });
            }
            runtime.flush().unwrap();
            let create = start.elapsed().as_nanos();
            if census && repetition == 0 {
                let g = runtime.graph_stats();
                println!(
                    "{{\"type\":\"census\",\"deps\":{deps},\"fanout\":{fanout},\"nodes\":{:?},\"dependencies\":{:?},\"subscribers\":{:?}}}",
                    g.nodes, g.dependencies, g.subscribers
                );
            }
            let start = Instant::now();
            for _ in 0..100 {
                sources[0].revise(|v| *v += 1);
                runtime.flush().unwrap();
            }
            let update = start.elapsed().as_nanos();
            let stats = runtime.stats();
            assert_eq!(stats.queued, 0);
            assert_eq!(stats.queue_entries, 0);
            assert_eq!(stats.nodes, deps.max(1) + fanout);
            assert_eq!(stats.edges, deps * fanout);
            let start = Instant::now();
            owner.dispose();
            let dispose = start.elapsed().as_nanos();
            assert_eq!(runtime.stats().nodes, 0);
            println!(
                "{{\"type\":\"run\",\"deps\":{deps},\"fanout\":{fanout},\"repetition\":{repetition},\"create_ns\":{create},\"update_100_ns\":{update},\"dispose_ns\":{dispose}}}"
            );
        }
    }
}
