#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_render::*;

#[test]
fn native_collapse_admission_preserves_shared_revisions_prefixes_and_query_watermarks() {
    let runtime = RuntimeId::new();
    let renderer = Renderer::new(runtime);
    assert!(renderer.capabilities().collapse_completion);
    conformance::verify_collapse(renderer, runtime, true);

    // Suspension is a terminal presentation boundary for the current revision,
    // but preserves semantic/physical identity and does not replay on Resume.
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let root = ids.allocate();
    let child = ids.allocate();
    let mut renderer = Renderer::with_config(
        runtime,
        glory_apace::Config {
            motion: MotionPreference::NoPreference,
            ..Default::default()
        },
    );
    let metadata = |open: bool, revision: u64| {
        Some(Box::new(Semantics {
            hidden: !open,
            inert: !open,
            collapse: Some(CollapseConfig {
                open,
                duration_millis: 200,
                revision,
            }),
            ..Default::default()
        }))
    };
    let mut declaration = Descriptor::new(Props::View);
    declaration.semantics = metadata(true, 0);
    let mut body = Descriptor::new(Props::View);
    body.style.height = Some(style::Length::Px(80.));
    renderer
        .apply(&[
            Mutation::Create {
                id: root,
                node: Box::new(declaration),
            },
            Mutation::Place {
                id: root,
                parent: None,
                before: None,
            },
            Mutation::Create {
                id: child,
                node: Box::new(body),
            },
            Mutation::Place {
                id: child,
                parent: Some(root),
                before: None,
            },
        ])
        .unwrap();
    renderer
        .listen(root, EventKind::CollapseComplete, true)
        .unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    inputs.clear();
    let physical = renderer.engine_id(root, glory_html::Part::Main).unwrap();
    renderer
        .apply(&[Mutation::Patch {
            id: root,
            patch: Patch::Semantics(Change::Set(metadata(false, 1))),
        }])
        .unwrap();
    renderer.advance_time(0.1).unwrap();
    renderer.apply(&[Mutation::Suspend { id: root }]).unwrap();
    renderer.drain(&mut inputs);
    assert_eq!(
        inputs
            .iter()
            .filter(|input| matches!(
                input,
                Input::Event {
                    event: Event::CollapseComplete { revision: 1 },
                    ..
                }
            ))
            .count(),
        1
    );
    assert!(!renderer.inspect(|doc| doc.is_animating()));
    inputs.clear();
    renderer
        .apply(&[Mutation::Resume {
            id: root,
            parent: None,
            before: None,
        }])
        .unwrap();
    renderer.drain(&mut inputs);
    assert!(inputs.iter().all(|input| !matches!(
        input,
        Input::Event {
            event: Event::CollapseComplete { .. },
            ..
        }
    )));
    assert_eq!(
        renderer.engine_id(root, glory_html::Part::Main).unwrap(),
        physical
    );
    renderer
        .apply(&[Mutation::Patch {
            id: root,
            patch: Patch::Semantics(Change::Set(metadata(true, 2))),
        }])
        .unwrap();
    renderer.apply(&[Mutation::Dispose { id: root }]).unwrap();
    renderer.advance_time(1.).unwrap();
    renderer.drain(&mut inputs);
    assert_eq!(renderer.stats().nodes, 0);
    assert!(!renderer.inspect(|doc| doc.is_animating()));
}

#[cfg(feature = "raster")]
mod pixels {
    use super::*;
    use glory_apace::{Config, Raster};
    use glory_render::style::*;
    use glory_ui::{App, view};
    use std::{cell::RefCell, rc::Rc};
    fn declaration(open: bool, revision: u64) -> Option<Box<Semantics>> {
        Some(Box::new(Semantics {
            collapse: Some(CollapseConfig {
                open,
                duration_millis: 200,
                revision,
            }),
            hidden: !open,
            inert: !open,
            ..Default::default()
        }))
    }
    fn bounds(app: &App, id: NodeId) -> Rect {
        let result = Rc::new(RefCell::new(None));
        let out = result.clone();
        app.request(Query::Bounds(id), move |reply| {
            *out.borrow_mut() = Some(reply.unwrap())
        })
        .unwrap();
        app.pump().unwrap();
        let QueryValue::Bounds(rect) = result.borrow_mut().take().unwrap() else {
            panic!("bounds")
        };
        rect
    }
    fn near(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.1,
            "actual={actual},expected={expected}"
        );
    }
    fn height(renderer: &Renderer, node: NodeId) -> f64 {
        Raster::new().draw(renderer).unwrap();
        let physical = renderer.engine_id(node, glory_html::Part::Main).unwrap();
        renderer
            .inspect(|doc| f64::from(doc.get_node(physical).unwrap().final_layout().size.height))
    }
    #[test]
    fn real_height_pixels_flow_and_completion_follow_native_clock_without_reactive_frames() {
        let mut renderer = None;
        let app = App::new(|runtime| {
            let value = Renderer::with_config(
                runtime,
                Config {
                    motion: MotionPreference::NoPreference,
                    ..Default::default()
                },
            );
            renderer = Some(value.clone());
            value
        });
        let renderer = renderer.unwrap();
        renderer.set_viewport(160, 160, 1.).unwrap();
        let source = app.scope().cage(declaration(true, 0));
        let reference = app.scope().cage(None);
        let following = app.scope().cage(None);
        let completions = Rc::new(RefCell::new(Vec::new()));
        let events = completions.clone();
        app.mount(
            view().fill((
                view()
                    .semantics(source)
                    .node_ref(reference)
                    .style(Style::new().width(Length::Px(80.)))
                    .on(EventKind::CollapseComplete, move |event| {
                        if let Event::CollapseComplete { revision } = event {
                            events.borrow_mut().push(revision);
                        }
                    })
                    .fill(view().style(Style {
                        height: Some(Length::Px(80.)),
                        background: Some(Color(255, 0, 0, 255)),
                        ..Style::new()
                    })),
                view().node_ref(following).style(Style {
                    height: Some(Length::Px(10.)),
                    background: Some(Color(0, 0, 255, 255)),
                    ..Style::new()
                }),
            )),
        )
        .unwrap();
        app.settle(8).unwrap();
        let node = reference.get().unwrap();
        let next = following.get().unwrap();
        near(height(&renderer, node), 80.);
        near(bounds(&app, next).y, 80.);
        assert_eq!(&*completions.borrow(), &[0]);
        source.revise(|value| *value = declaration(false, 1));
        app.flush().unwrap();
        near(height(&renderer, node), 80.);
        assert!(renderer.inspect(|doc| doc.is_animating()));
        assert!(!app.tree().active(node));
        assert!(app.tree().inert(node));
        let writes = app.runtime_stats().source_writes;
        renderer.advance_time(0.1).unwrap();
        let mut raster = Raster::new();
        raster.draw(&renderer).unwrap();
        near(height(&renderer, node), 40.);
        near(bounds(&app, next).y, 40.);
        assert_eq!(
            &raster.pixels()[(20 * 160 + 10) * 4..(20 * 160 + 10) * 4 + 4],
            &[255, 0, 0, 255]
        );
        assert_eq!(
            &raster.pixels()[(45 * 160 + 10) * 4..(45 * 160 + 10) * 4 + 4],
            &[0, 0, 255, 255]
        );
        assert_eq!(app.runtime_stats().source_writes, writes);
        assert_eq!(&*completions.borrow(), &[0]);
        renderer.advance_time(0.2).unwrap();
        app.settle(8).unwrap();
        near(height(&renderer, node), 0.);
        near(bounds(&app, next).y, 0.);
        assert_eq!(&*completions.borrow(), &[0, 1]);
        assert!(!renderer.inspect(|doc| doc.is_animating()));
        source.revise(|value| *value = declaration(true, 2));
        app.flush().unwrap();
        renderer.advance_time(0.3).unwrap();
        near(height(&renderer, node), 40.);
        source.revise(|value| *value = declaration(false, 3));
        app.flush().unwrap();
        near(height(&renderer, node), 40.);
        renderer.advance_time(0.4).unwrap();
        near(height(&renderer, node), 20.);
        renderer.advance_time(0.5).unwrap();
        app.settle(8).unwrap();
        near(height(&renderer, node), 0.);
        assert_eq!(&*completions.borrow(), &[0, 1, 3]);
        app.clear().unwrap();
        let revision = renderer.stats().visual_revision;
        renderer.advance_time(1.).unwrap();
        assert_eq!(renderer.stats().visual_revision, revision);
    }

    #[test]
    fn intrinsic_growth_retargets_remaining_time_then_releases_constraints_and_retires_unavailable_motion()
     {
        let mut renderer = None;
        let app = App::new(|runtime| {
            let value = Renderer::with_config(
                runtime,
                Config {
                    motion: MotionPreference::NoPreference,
                    ..Default::default()
                },
            );
            renderer = Some(value.clone());
            value
        });
        let renderer = renderer.unwrap();
        renderer.set_viewport(160, 200, 1.).unwrap();
        let config = app.scope().cage(declaration(false, 0));
        let body = app.scope().cage(Style::new().height(Length::Px(80.)));
        let parent = app.scope().cage(Style::new());
        let reference = app.scope().cage(None);
        let events = Rc::new(RefCell::new(Vec::new()));
        let completed = events.clone();
        app.mount(
            view().class("collapse-parent").style(parent).fill(
                view()
                    .semantics(config)
                    .node_ref(reference)
                    .on(EventKind::CollapseComplete, move |event| {
                        if let Event::CollapseComplete { revision } = event {
                            completed.borrow_mut().push(revision);
                        }
                    })
                    .fill(view().style(body)),
            ),
        )
        .unwrap();
        app.settle(8).unwrap();
        let node = reference.get().unwrap();
        config.revise(|value| *value = declaration(true, 1));
        app.flush().unwrap();
        renderer.advance_time(0.1).unwrap();
        near(height(&renderer, node), 40.);
        body.revise(|value| *value = Style::new().height(Length::Px(120.)));
        app.flush().unwrap();
        near(height(&renderer, node), 40.);
        renderer.advance_time(0.15).unwrap();
        near(height(&renderer, node), 80.);
        renderer.advance_time(0.2).unwrap();
        app.settle(8).unwrap();
        near(height(&renderer, node), 120.);
        assert_eq!(&*events.borrow(), &[0, 1]);
        body.revise(|value| *value = Style::new().height(Length::Px(160.)));
        app.flush().unwrap();
        near(height(&renderer, node), 160.);
        assert!(!renderer.inspect(|doc| doc.is_animating()));
        config.revise(|value| *value = declaration(false, 2));
        app.flush().unwrap();
        renderer.advance_time(0.3).unwrap();
        near(height(&renderer, node), 80.);
        renderer.set_motion_preference(MotionPreference::Reduce);
        app.settle(8).unwrap();
        near(height(&renderer, node), 0.);
        assert_eq!(&*events.borrow(), &[0, 1, 2]);
        assert!(!renderer.inspect(|doc| doc.is_animating()));
        renderer.set_motion_preference(MotionPreference::NoPreference);
        config.revise(|value| *value = declaration(true, 3));
        app.flush().unwrap();
        renderer.set_viewport(0, 200, 1.).unwrap();
        app.settle(8).unwrap();
        assert_eq!(&*events.borrow(), &[0, 1, 2, 3]);
        assert!(!renderer.inspect(|doc| doc.is_animating()));
        renderer.set_viewport(160, 200, 1.).unwrap();
        app.settle(8).unwrap();
        near(height(&renderer, node), 160.);
        assert_eq!(&*events.borrow(), &[0, 1, 2, 3]);
        config.revise(|value| *value = declaration(false, 4));
        app.flush().unwrap();
        parent.revise(|value| *value = Style::new().display(Display::None));
        app.flush().unwrap();
        app.settle(8).unwrap();
        assert_eq!(&*events.borrow(), &[0, 1, 2, 3, 4]);
        assert!(!renderer.inspect(|doc| doc.is_animating()));
        parent.revise(|value| *value = Style::new());
        app.flush().unwrap();
        config.revise(|value| *value = declaration(true, 5));
        app.flush().unwrap();
        assert!(renderer.inspect(|doc| doc.is_animating()));
        renderer.set_css(".collapse-parent{display:none}");
        app.settle(8).unwrap();
        assert_eq!(&*events.borrow(), &[0, 1, 2, 3, 4, 5]);
        assert!(!renderer.inspect(|doc| doc.is_animating()));
        renderer.set_css("");
        app.settle(8).unwrap();
        near(height(&renderer, node), 160.);
        config.revise(|value| *value = declaration(false, 6));
        app.flush().unwrap();
        assert!(renderer.inspect(|doc| doc.is_animating()));
        app.clear().unwrap();
        assert!(!renderer.inspect(|doc| doc.is_animating()));
        let revision = renderer.stats().visual_revision;
        renderer.advance_time(1.).unwrap();
        assert_eq!(renderer.stats().visual_revision, revision);
    }

    #[test]
    fn nested_collapses_measure_final_intrinsic_content_and_do_not_replay_hidden_child_motion() {
        let mut renderer = None;
        let app = App::new(|runtime| {
            let value = Renderer::with_config(
                runtime,
                Config {
                    motion: MotionPreference::NoPreference,
                    ..Default::default()
                },
            );
            renderer = Some(value.clone());
            value
        });
        let renderer = renderer.unwrap();
        renderer.set_viewport(160, 160, 1.).unwrap();
        let outer = app.scope().cage(declaration(false, 0));
        let inner = app.scope().cage(declaration(false, 0));
        let reference = app.scope().cage(None);
        let child = app.scope().cage(None);
        app.mount(
            view()
                .semantics(outer)
                .node_ref(reference)
                .style(Style::new().width(Length::Px(80.)))
                .fill((
                    view().style(Style::new().height(Length::Px(20.))),
                    view()
                        .semantics(inner)
                        .node_ref(child)
                        .fill(view().style(Style::new().height(Length::Px(80.)))),
                )),
        )
        .unwrap();
        app.settle(8).unwrap();
        let node = reference.get().unwrap();
        let child = child.get().unwrap();
        app.scope()
            .batch(|| {
                outer.revise(|value| *value = declaration(true, 1));
                inner.revise(|value| *value = declaration(true, 1));
            })
            .unwrap();
        app.flush().unwrap();
        renderer.advance_time(0.1).unwrap();
        near(height(&renderer, node), 50.);
        near(height(&renderer, child), 80.);
        renderer.advance_time(0.2).unwrap();
        app.settle(8).unwrap();
        near(height(&renderer, node), 100.);
        outer.revise(|value| *value = declaration(false, 2));
        app.flush().unwrap();
        renderer.advance_time(0.3).unwrap();
        near(height(&renderer, node), 50.);
        renderer.advance_time(0.4).unwrap();
        app.settle(8).unwrap();
        near(height(&renderer, node), 0.);
        assert!(!renderer.inspect(|doc| doc.is_animating()));
        outer.revise(|value| *value = declaration(true, 3));
        app.flush().unwrap();
        renderer.advance_time(0.5).unwrap();
        near(height(&renderer, node), 50.);
        app.clear().unwrap();
        assert!(!renderer.inspect(|doc| doc.is_animating()));
    }

    #[test]
    fn actual_text_wrapping_at_two_x_scale_retargets_opening_height_without_restarting_deadline() {
        let mut renderer = None;
        let app = App::new(|runtime| {
            let value = Renderer::with_config(
                runtime,
                Config {
                    motion: MotionPreference::NoPreference,
                    ..Default::default()
                },
            );
            renderer = Some(value.clone());
            value
        });
        let renderer = renderer.unwrap();
        renderer.set_viewport(640, 480, 2.).unwrap();
        let config = app.scope().cage(declaration(true, 0));
        let reference = app.scope().cage(None);
        let width = app.scope().cage(Style {
            width: Some(Length::Px(240.)),
            font_size: Some(16.),
            line_height: Some(20.),
            ..Style::new()
        });
        app.mount(
            view()
                .semantics(config)
                .node_ref(reference)
                .style(width)
                .fill(
                    glory_ui::text()
                        .content("one two three four five six seven eight nine ten eleven twelve"),
                ),
        )
        .unwrap();
        app.settle(8).unwrap();
        let node = reference.get().unwrap();
        let wide = height(&renderer, node);
        assert!(wide > 0.);
        config.revise(|value| *value = declaration(false, 1));
        app.flush().unwrap();
        renderer.advance_time(0.2).unwrap();
        app.settle(8).unwrap();
        config.revise(|value| *value = declaration(true, 2));
        app.flush().unwrap();
        renderer.advance_time(0.3).unwrap();
        near(height(&renderer, node), wide / 2.);
        width.revise(|style| style.width = Some(Length::Px(80.)));
        app.flush().unwrap();
        near(height(&renderer, node), wide / 2.);
        renderer.advance_time(0.4).unwrap();
        app.settle(8).unwrap();
        let narrow = height(&renderer, node);
        assert!(narrow > wide);
        near(bounds(&app, node).width, 80.);
        assert!(!renderer.inspect(|doc| doc.is_animating()));
        app.clear().unwrap();
    }
}
