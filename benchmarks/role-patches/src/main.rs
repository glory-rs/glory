#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use bench_role_patches::{CASES, Workload};
    use std::time::Instant;
    let pairs: usize = std::env::args()
        .nth(1)
        .map(|s| s.parse().unwrap())
        .unwrap_or(2048);
    let samples: usize = std::env::args()
        .nth(2)
        .map(|s| s.parse().unwrap())
        .unwrap_or(20);
    for (case, name) in CASES.iter().enumerate() {
        let mut workload = Workload::new(case);
        for _ in 0..5 {
            workload.run(pairs);
        }
        for sample in 0..samples {
            let start = Instant::now();
            workload.run(pairs);
            let ns = start.elapsed().as_nanos();
            println!(
                "{{\"case\":\"{name}\",\"sample\":{sample},\"ns\":{ns},\"result\":{}}}",
                workload.result()
            );
        }
    }
}
#[cfg(target_arch = "wasm32")]
fn main() {}
