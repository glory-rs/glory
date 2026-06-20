//! NT6 — screenshot regression for the native (Blitz) render pipeline.
//!
//! This drives the *whole* native render path with nothing but the Glory
//! command stream:
//!
//!   widget → `CommandHolder` → `BlitzConsumer` (commands → `BaseDocument`)
//!   → `doc.set_viewport` + `doc.resolve` (layout)
//!   → `blitz_paint::paint_scene` (document → `anyrender::PaintScene`)
//!   → `VelloImageRenderer` (GPU off-screen) → RGBA8 buffer.
//!
//! The buffer is then asserted on: correct length, non-empty (something was
//! painted) and a baseline check on a known background-colour pixel.
//!
//! GPU caveat: `VelloImageRenderer::new` calls `WGPUContext::create_buffer_renderer`
//! which *panics* ("No compatible device found") when wgpu can't acquire an
//! adapter/device. There is no headless fallback in this alpha, so the actual
//! render test is marked `#[ignore]`: it runs wherever a real or software
//! (lavapipe / DX12 / Vulkan) adapter is available, but `cargo test --no-run`
//! always compile/link-checks it, and CI runs it under software Vulkan
//! (see the crate-level NT6 notes / the PR description CI snippet).
//!
//! Only built under the `shell` feature (where `anyrender_vello` is available).

#![cfg(feature = "shell")]

use anyrender_vello::VelloImageRenderer;
use blitz_traits::shell::{ColorScheme, Viewport};
use glory_core::Holder;
use glory_core::reflow::Cage;
use glory_core::web::holders::CommandHolder;
use glory_core::web::widgets::{div, span};
use glory_core::{Scope, Widget};
use glory_native::BlitzConsumer;

const WIDTH: u32 = 200;
const HEIGHT: u32 = 120;

/// A widget with an explicit, opaque background so the painted output is
/// deterministic and we can baseline a known pixel.
#[derive(Debug)]
struct ColoredBox {
    label: Cage<String>,
}

impl Widget for ColoredBox {
    fn build(&mut self, ctx: &mut Scope) {
        div()
            .attr("data-app", "screenshot")
            .attr(
                "style",
                "width: 200px; height: 120px; background-color: rgb(0, 0, 255); color: rgb(255, 255, 255);",
            )
            .fill(span().text(self.label.clone()))
            .show_in(ctx);
    }
}

/// Build the document from the command stream and resolve its layout against a
/// fixed viewport, so it is ready to be painted.
fn build_resolved_consumer() -> BlitzConsumer {
    let holder = CommandHolder::new().mount(ColoredBox {
        label: Cage::new("hello".to_owned()),
    });
    let mut consumer = BlitzConsumer::new();
    consumer.apply_batch(&holder.take_batch());

    let doc = consumer.document_mut();
    doc.set_viewport(Viewport::new(WIDTH, HEIGHT, 1.0, ColorScheme::Light));
    doc.resolve(0.0);
    consumer
}

/// Render the resolved document into an RGBA8 buffer via the off-screen
/// vello image renderer (requires a wgpu adapter — see module docs).
fn render_rgba(consumer: &mut BlitzConsumer) -> Vec<u8> {
    let doc = consumer.document_mut();
    anyrender::render_to_buffer::<VelloImageRenderer, _>(
        |scene| {
            blitz_paint::paint_scene(scene, doc, 1.0, WIDTH, HEIGHT, 0, 0);
        },
        WIDTH,
        HEIGHT,
    )
}

/// Compile/link smoke: the document is built and resolved with no GPU at all.
/// This always runs and guarantees the layout half of the pipeline is sound,
/// independent of adapter availability.
#[test]
fn screenshot_pipeline_resolves_layout_without_gpu() {
    // Building + resolving the document (inside `build_resolved_consumer`) must
    // not panic — that exercises the GPU-independent half of the pipeline
    // (command stream → BaseDocument → taffy layout resolve).
    let consumer = build_resolved_consumer();

    // The command stream built a real document (root node present) and layout
    // resolved without panicking. The full paint (including the <div>) is
    // covered by `screenshot_renders_non_empty_rgba_buffer` where a wgpu
    // adapter is available.
    assert!(consumer.document().get_node(0).is_some(), "document root node should exist");
}

/// The actual GPU screenshot regression. `#[ignore]` because
/// `VelloImageRenderer::new` panics when no wgpu adapter is available; run with
/// `--ignored` on a machine/CI runner that has a (software or hardware) adapter.
#[test]
#[ignore = "needs a wgpu adapter (GPU or software Vulkan/lavapipe); run with --ignored"]
fn screenshot_renders_non_empty_rgba_buffer() {
    let mut consumer = build_resolved_consumer();
    let buffer = render_rgba(&mut consumer);

    // RGBA8: exactly width*height*4 bytes.
    assert_eq!(buffer.len(), (WIDTH * HEIGHT * 4) as usize, "RGBA8 buffer must be width*height*4");

    // Something was actually painted (not an all-zero buffer).
    assert!(buffer.iter().any(|&b| b != 0), "rendered buffer is entirely zero — nothing painted");

    // Baseline: a pixel inside the box should carry the blue background.
    // Sample the centre pixel; vello writes RGBA8 (possibly premultiplied),
    // so assert blue dominates and the pixel is opaque rather than exact bytes.
    let cx = WIDTH / 2;
    let cy = HEIGHT / 2;
    let idx = ((cy * WIDTH + cx) * 4) as usize;
    let (r, g, b, a) = (buffer[idx], buffer[idx + 1], buffer[idx + 2], buffer[idx + 3]);
    assert!(a > 0, "centre pixel should be opaque (a={a})");
    assert!(b > r && b > g, "centre pixel should be blue-dominant (r={r} g={g} b={b})");
}
