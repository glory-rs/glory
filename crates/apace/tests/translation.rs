#![cfg(all(not(target_arch = "wasm32"), feature = "raster"))]
use glory_apace::{Config, Raster, Renderer};
use glory_render::{style::*, *};
use glory_ui::{App, view};
use std::{cell::RefCell, rc::Rc};
fn motion(x: f64) -> TranslationMotion {
    TranslationMotion {
        kind: TranslationKind::To {
            target: TranslationOffset::new(x, 0.),
            duration_millis: 1000,
        },
        paused: false,
    }
}
fn semantics(value: TranslationMotion) -> Option<Box<Semantics>> {
    Some(Box::new(Semantics {
        inert: true,
        translation: Some(value),
        ..Default::default()
    }))
}
fn app() -> (App, Renderer) {
    let mut renderer = None;
    let app = App::new(|runtime| {
        let value = Renderer::with_config(
            runtime,
            Config {
                motion: MotionPreference::NoPreference,
                ..Default::default()
            },
        );
        value.set_viewport(160, 80, 1.).unwrap();
        renderer = Some(value.clone());
        value
    });
    (app, renderer.unwrap())
}
fn bounds(app: &App, node: NodeId) -> Rect {
    let result = Rc::new(RefCell::new(None));
    let reply = result.clone();
    app.request(Query::Bounds(node), move |value| {
        *reply.borrow_mut() = Some(value.unwrap())
    })
    .unwrap();
    app.pump().unwrap();
    let QueryValue::Bounds(rect) = result.borrow_mut().take().unwrap() else {
        panic!("bounds")
    };
    rect
}
fn pixel(raster: &Raster, x: usize, y: usize) -> [u8; 4] {
    raster.pixels()[(y * 160 + x) * 4..(y * 160 + x) * 4 + 4]
        .try_into()
        .unwrap()
}
fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.05,
        "actual={actual},expected={expected}"
    );
}
#[test]
fn nested_offsets_query_self_descendants_and_real_parent_clip_opacity() {
    let (app, renderer) = app();
    let reference = app.scope().cage(None);
    app.mount(
        view()
            .style(Style {
                width: Some(Length::Px(80.)),
                height: Some(Length::Px(60.)),
                overflow_x: Some(Overflow::Hidden),
                overflow_y: Some(Overflow::Hidden),
                opacity: Some(0.5),
                background: Some(Color(255, 255, 255, 255)),
                ..Style::new()
            })
            .fill(
                view().semantics(semantics(motion(12.))).fill(
                    view()
                        .semantics(semantics(motion(8.)))
                        .node_ref(reference)
                        .style(Style {
                            width: Some(Length::Px(20.)),
                            height: Some(Length::Px(20.)),
                            background: Some(Color(255, 0, 0, 255)),
                            ..Style::new()
                        }),
                ),
            ),
    )
    .unwrap();
    let mut raster = Raster::new();
    raster.draw(&renderer).unwrap();
    let target = reference.get().unwrap();
    near(bounds(&app, target).x, 20.);
    assert_eq!(pixel(&raster, 25, 5), [128, 0, 0, 128]);
    app.scope()
        .patch(
            target,
            Patch::Semantics(Change::Set(semantics(motion(70.)))),
        )
        .unwrap();
    app.flush().unwrap();
    renderer.advance_time(1.).unwrap();
    raster.draw(&renderer).unwrap();
    near(bounds(&app, target).x, 82.);
    assert_ne!(pixel(&raster, 79, 5), pixel(&raster, 84, 5));
}
#[test]
fn current_clock_pause_interrupt_and_clear_match_pixels_without_application_frames() {
    let (app, renderer) = app();
    let source = app.scope().cage(semantics(motion(0.)));
    let reference = app.scope().cage(None);
    app.mount(view().semantics(source).node_ref(reference).style(Style {
        width: Some(Length::Px(20.)),
        height: Some(Length::Px(20.)),
        background: Some(Color(255, 0, 0, 255)),
        ..Style::new()
    }))
    .unwrap();
    let node = reference.get().unwrap();
    let physical = renderer.engine_id(node, glory_html::Part::Main).unwrap();
    let mut raster = Raster::new();
    raster.draw(&renderer).unwrap();
    source.revise(|value| *value = semantics(motion(40.)));
    app.flush().unwrap();
    renderer.advance_time(0.25).unwrap();
    near(bounds(&app, node).x, 10.);
    source.revise(|value| *value = semantics(motion(40.)));
    app.flush().unwrap();
    let revision = renderer.stats().visual_revision;
    let writes = app.runtime_stats().source_writes;
    renderer.advance_time(0.5).unwrap();
    raster.draw(&renderer).unwrap();
    near(bounds(&app, node).x, 20.);
    assert_eq!(pixel(&raster, 25, 5), [255, 0, 0, 255]);
    assert_ne!(pixel(&raster, 5, 5), [255, 0, 0, 255]);
    assert!(renderer.stats().visual_revision > revision);
    assert_eq!(app.runtime_stats().source_writes, writes);
    source.revise(|value| {
        *value = semantics(TranslationMotion {
            paused: true,
            ..motion(40.)
        })
    });
    app.flush().unwrap();
    renderer.advance_time(10.).unwrap();
    raster.draw(&renderer).unwrap();
    near(bounds(&app, node).x, 20.);
    assert_eq!(pixel(&raster, 25, 5), [255, 0, 0, 255]);
    source.revise(|value| *value = semantics(motion(-20.)));
    app.flush().unwrap();
    renderer.advance_time(10.25).unwrap();
    raster.draw(&renderer).unwrap();
    near(bounds(&app, node).x, 10.);
    assert_eq!(pixel(&raster, 15, 5), [255, 0, 0, 255]);
    source.revise(|value| *value = None);
    app.flush().unwrap();
    raster.draw(&renderer).unwrap();
    near(bounds(&app, node).x, 0.);
    assert_eq!(pixel(&raster, 5, 5), [255, 0, 0, 255]);
    assert_eq!(
        renderer.engine_id(node, glory_html::Part::Main).unwrap(),
        physical
    );
    app.clear().unwrap();
    let revision = renderer.stats().visual_revision;
    renderer.advance_time(11.).unwrap();
    assert_eq!(renderer.stats().visual_revision, revision);
}
#[test]
fn loop_pause_reduce_and_zero_viewport_retire_current_engine_animation() {
    let (app, renderer) = app();
    let source = app.scope().cage(semantics(TranslationMotion {
        kind: TranslationKind::Loop {
            from: TranslationOffset::ZERO,
            to: TranslationOffset::new(100., 0.),
            period_millis: 1000,
        },
        paused: false,
    }));
    let reference = app.scope().cage(None);
    app.mount(
        view()
            .semantics(source)
            .node_ref(reference)
            .style(Style::new().width(Length::Px(20.)).height(Length::Px(20.))),
    )
    .unwrap();
    let node = reference.get().unwrap();
    renderer.advance_time(0.25).unwrap();
    near(bounds(&app, node).x, 25.);
    assert!(renderer.inspect(|d| d.is_animating()));
    renderer.set_motion_preference(MotionPreference::Reduce);
    near(bounds(&app, node).x, 0.);
    assert!(!renderer.inspect(|d| d.is_animating()));
    renderer.set_motion_preference(MotionPreference::NoPreference);
    renderer.advance_time(0.5).unwrap();
    near(bounds(&app, node).x, 25.);
    renderer.set_viewport(0, 80, 1.).unwrap();
    assert!(!renderer.inspect(|d| d.is_animating()));
    renderer.set_viewport(160, 80, 1.).unwrap();
    near(bounds(&app, node).x, 0.);
    app.clear().unwrap();
    assert_eq!(app.stats().queries, 0);
}

#[test]
fn query_translation_adds_live_ancestors_without_double_counting_any_scroll() {
    let (app, renderer) = app();
    let scroller = app.scope().cage(None);
    let child = app.scope().cage(None);
    let offset = |x, y| {
        semantics(TranslationMotion {
            kind: TranslationKind::To {
                target: TranslationOffset::new(x, y),
                duration_millis: 200,
            },
            paused: false,
        })
    };
    app.mount(
        view()
            .node_ref(scroller)
            .semantics(offset(12., 5.))
            .style(
                Style::new()
                    .width(Length::Px(80.))
                    .height(Length::Px(40.))
                    .overflow_y(Overflow::Auto),
            )
            .fill(
                view()
                    .node_ref(child)
                    .semantics(offset(8., 7.))
                    .style(Style::new().width(Length::Px(40.)).height(Length::Px(200.))),
            ),
    )
    .unwrap();
    let scroller = scroller.get().unwrap();
    let child = child.get().unwrap();
    near(bounds(&app, scroller).x, 12.);
    near(bounds(&app, scroller).y, 5.);
    near(bounds(&app, child).x, 20.);
    near(bounds(&app, child).y, 12.);
    app.scope()
        .action(Action::ScrollTo {
            node: scroller,
            x: 0.,
            y: 20.,
        })
        .unwrap();
    app.flush().unwrap();
    near(bounds(&app, scroller).y, 5.);
    near(bounds(&app, child).y, -8.);
    app.scope()
        .patch(
            child,
            Patch::Style(Change::Set(Box::new(
                Style::new().width(Length::Px(30.)).height(Length::Px(200.)),
            ))),
        )
        .unwrap();
    app.flush().unwrap();
    near(bounds(&app, child).x, 20.);
    near(bounds(&app, child).y, -8.);
    let mut raster = Raster::new();
    raster.draw(&renderer).unwrap();
    app.clear().unwrap();
}
