# Each Bench

Small standalone timing harness for the `Each` keyed-list patch path. It runs
against the SSR backend so it can be executed without a browser.

```sh
cargo run --release --manifest-path examples/each-bench/Cargo.toml
```

The output is CSV:

```text
workload,n,iterations,total_us,avg_us
reverse,10,200,...
```

Use the numbers as local before/after baselines when changing the `Each`
reorder algorithm. The canonical Criterion suite lives in
`crates/core/benches/each_reorder.rs`.
