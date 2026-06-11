//! SSR render/string/chunk streaming benchmarks.
//!
//! Run: `cargo bench -p glory-core --features web-ssr --bench ssr_stream`

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use futures::StreamExt;
use glory_core::config::GloryConfig;
use glory_core::web::holders::{HtmlChunk, ServerHolder};
use glory_core::web::widgets::{article, div, h1, li, p, ul};
use glory_core::{Holder, Scope, Widget};

#[derive(Debug)]
struct SsrRows {
    rows: usize,
}

impl Widget for SsrRows {
    fn build(&mut self, ctx: &mut Scope) {
        let rows: Vec<_> = (0..self.rows)
            .map(|index| li().attr("data-id", index).text(format!("row {index}")))
            .collect();

        article()
            .fill(h1().text("SSR benchmark"))
            .fill(div().class("summary").text(format!("{} rows", self.rows)))
            .fill(ul().fill(rows))
            .fill(p().text("done"))
            .show_in(ctx);
    }
}

fn mount_rows(rows: usize) -> ServerHolder {
    ServerHolder::new(GloryConfig::default(), "/bench").mount(SsrRows { rows })
}

fn stream_to_vec(holder: ServerHolder) -> Vec<String> {
    futures::executor::block_on(holder.render_stream().map(HtmlChunk::into_string).collect::<Vec<_>>())
}

fn bench_ssr_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("ssr_render");
    group.sample_size(20);

    for rows in [100_usize, 1000] {
        group.bench_function(format!("render_string_{rows}_rows"), |b| {
            b.iter_batched(|| mount_rows(rows), |holder| black_box(holder.render_string()), BatchSize::SmallInput);
        });

        group.bench_function(format!("render_stream_collect_{rows}_rows"), |b| {
            b.iter_batched(|| mount_rows(rows), |holder| black_box(stream_to_vec(holder)), BatchSize::SmallInput);
        });
    }

    group.finish();
}

criterion_group!(benches, bench_ssr_render);
criterion_main!(benches);
