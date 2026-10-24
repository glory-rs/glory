#![cfg(all(not(target_arch = "wasm32"), feature = "raster"))]
use glory_blitz::{Config, Raster, Renderer};
use glory_render::{style::*, *};
use glory_ui::{App, graphic, view};
fn drawing(y: f64) -> GraphicProps {
    GraphicProps {
        commands: vec![
            PathCommand::Move(GraphicPoint::new(2., y)),
            PathCommand::Line(GraphicPoint::new(22., y)),
        ],
        stroke: Some(GraphicStroke {
            color: Color(0, 255, 0, 255),
            width: 4.,
            cap: StrokeCap::Butt,
        }),
        transition: Some(GraphicTransition {
            duration_millis: 200,
        }),
        ..Default::default()
    }
}
fn pixel(raster: &Raster, x: usize, y: usize) -> &[u8] {
    let width = raster.frame().unwrap().width as usize;
    &raster.pixels()[(y * width + x) * 4..(y * width + x) * 4 + 4]
}
fn host() -> (App, Renderer) {
    let mut result = None;
    let app = App::new(|runtime| {
        let renderer = Renderer::with_config(
            runtime,
            Config {
                motion: MotionPreference::NoPreference,
                ..Default::default()
            },
        );
        renderer.set_viewport(40, 40, 1.).unwrap();
        result = Some(renderer.clone());
        renderer
    });
    (app, result.unwrap())
}
#[test]
fn real_pixels_interrupt_equal_keep_completion_reduction_and_retirement_are_current_only() {
    let (app, renderer) = host();
    let source = app.scope().cage(drawing(4.));
    app.mount(
        graphic()
            .drawing(source)
            .style(Style::new().width(Length::Px(24.)).height(Length::Px(24.))),
    )
    .unwrap();
    let id = app.tree().roots()[0];
    let engine = renderer.engine_id(id, glory_html::Part::Main).unwrap();
    let mut raster = Raster::new();
    raster.draw(&renderer).unwrap();
    assert_eq!(pixel(&raster, 12, 4), [0, 255, 0, 255]);
    assert!(!renderer.inspect(|d| d.is_animating()));
    source.revise(|g| *g = drawing(20.));
    app.flush().unwrap();
    assert!(renderer.inspect(|d| d.is_animating()));
    let writes = app.runtime_stats().source_writes;
    let revision = renderer.stats().visual_revision;
    renderer.advance_time(0.1).unwrap();
    raster.draw(&renderer).unwrap();
    assert!(renderer.stats().visual_revision > revision);
    assert_eq!(pixel(&raster, 12, 12), [0, 255, 0, 255]);
    assert_eq!(pixel(&raster, 12, 20), [0, 0, 0, 0]);
    assert_eq!(app.runtime_stats().source_writes, writes);
    source.revise(|g| *g = drawing(4.));
    app.flush().unwrap();
    raster.draw(&renderer).unwrap();
    assert_eq!(pixel(&raster, 12, 12), [0, 255, 0, 255]);
    app.scope()
        .patch(id, Patch::Graphic(Change::Set(Box::new(drawing(4.)))))
        .unwrap();
    app.scope().patch(id, Patch::Graphic(Change::Keep)).unwrap();
    app.flush().unwrap();
    renderer.advance_time(0.2).unwrap();
    raster.draw(&renderer).unwrap();
    assert_eq!(pixel(&raster, 12, 8), [0, 255, 0, 255]);
    renderer.advance_time(0.31).unwrap();
    raster.draw(&renderer).unwrap();
    assert_eq!(pixel(&raster, 12, 4), [0, 255, 0, 255]);
    assert!(!renderer.inspect(|d| d.is_animating()));
    let revision = renderer.stats().visual_revision;
    renderer.advance_time(1.).unwrap();
    assert_eq!(renderer.stats().visual_revision, revision);
    source.revise(|g| *g = drawing(20.));
    app.flush().unwrap();
    renderer.advance_time(1.1).unwrap();
    renderer.set_motion_preference(MotionPreference::Reduce);
    raster.draw(&renderer).unwrap();
    assert_eq!(pixel(&raster, 12, 20), [0, 255, 0, 255]);
    assert!(!renderer.inspect(|d| d.is_animating()));
    renderer.set_motion_preference(MotionPreference::NoPreference);
    assert!(!renderer.inspect(|d| d.is_animating()));
    assert_eq!(
        renderer.engine_id(id, glory_html::Part::Main).unwrap(),
        engine
    );
    app.clear().unwrap();
    assert!(renderer.engine_id(id, glory_html::Part::Main).is_err());
    assert!(!renderer.inspect(|d| d.is_animating()));
}
#[test]
fn actual_group_clip_opacity_and_hidden_cancel_preserve_accepted_endpoint() {
    let (app, renderer) = host();
    let source = app.scope().cage(drawing(4.));
    let hidden = app.scope().cage(false);
    app.mount(
        view()
            .style(
                Style::new()
                    .width(Length::Px(12.))
                    .height(Length::Px(24.))
                    .overflow_x(Overflow::Hidden)
                    .overflow_y(Overflow::Hidden)
                    .opacity(0.5),
            )
            .semantics(hidden.map(|hidden| {
                Some(Box::new(Semantics {
                    hidden: *hidden,
                    ..Default::default()
                }))
            }))
            .fill(
                graphic()
                    .drawing(source)
                    .style(Style::new().width(Length::Px(24.)).height(Length::Px(24.))),
            ),
    )
    .unwrap();
    let mut raster = Raster::new();
    raster.draw(&renderer).unwrap();
    source.revise(|g| *g = drawing(20.));
    app.flush().unwrap();
    renderer.advance_time(0.1).unwrap();
    raster.draw(&renderer).unwrap();
    assert_eq!(pixel(&raster, 6, 12), [0, 128, 0, 128]);
    assert_eq!(pixel(&raster, 20, 12), [0, 0, 0, 0]);
    hidden.revise(|v| *v = true);
    app.flush().unwrap();
    assert!(!renderer.inspect(|d| d.is_animating()));
    hidden.revise(|v| *v = false);
    app.flush().unwrap();
    raster.draw(&renderer).unwrap();
    assert_eq!(pixel(&raster, 6, 20), [0, 128, 0, 128]);
    assert!(!renderer.inspect(|d| d.is_animating()));
}
