#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
#[cfg(feature = "raster")]
use glory_render::profile::{Phase, Profile};
#[cfg(feature = "raster")]
use glory_render::style::Color;
use glory_render::style::{Length, Style};
use glory_render::{Action, Event, Query, Render};
use glory_ui::{App, events, list, text, view};
use std::cell::Cell;
#[cfg(feature = "raster")]
use std::cell::RefCell;
use std::rc::Rc;

fn fixture() -> (App, Renderer) {
    let mut renderer = None;
    let app = App::new(|id| {
        let host = Renderer::new(id);
        host.set_viewport(160, 120, 1.).unwrap();
        renderer = Some(host.clone());
        host
    });
    (app, renderer.unwrap())
}

#[test]
fn observation_budget_yields_for_geometry_feedback_and_recovers() {
    let (app, renderer) = fixture();
    let active = app.scope().cage(true);
    let width = app.scope().cage(100.);
    let calls = Rc::new(Cell::new(0usize));
    let seen = calls.clone();
    app.mount(
        view()
            .style(width.map(|w| Style::new().width(Length::Px(*w)).height(Length::Px(20.))))
            .on(events::resize, move |event| {
                if let Event::Resize { size, .. } = event {
                    seen.set(seen.get() + 1);
                    if active.get() {
                        width.revise(|w| *w = if size.width < 100.5 { 101. } else { 100. });
                    }
                }
            }),
    )
    .unwrap();
    for limit in [0, App::MAX_SETTLE_PASSES + 1] {
        assert!(matches!(
            app.settle(limit),
            Err(glory_ui::Error::SettleLimit { .. })
        ));
    }
    assert_eq!(calls.get(), 0);
    let turn = app.settle(3).unwrap();
    assert_eq!(turn.passes, 3);
    assert!(turn.pending);
    assert_eq!(calls.get(), 3);
    assert!(renderer.needs_observe());
    active.revise(|v| *v = false);
    let turn = app.settle(8).unwrap();
    assert!(!turn.pending);
    assert!(turn.passes <= 2);
    let revision = renderer.stats().visual_revision;
    let before = calls.get();
    assert_eq!(app.settle(8).unwrap().passes, 1);
    assert_eq!(calls.get(), before);
    assert_eq!(renderer.stats().visual_revision, revision);
    app.clear().unwrap();
    assert!(!app.settle(8).unwrap().pending);
}

#[test]
fn query_resolved_layout_and_clamped_scroll_ack_remain_observable() {
    let (app, renderer) = fixture();
    let position = app.scope().cage(0.);
    let sizes = Rc::new(Cell::new(0));
    let seen = sizes.clone();
    app.mount(
        list(1000usize, |i| i, |_, key| text().content(format!("{key}")))
            .height(120.)
            .row_height(24.)
            .position(position)
            .inspect(move |_| seen.set(seen.get() + 1)),
    )
    .unwrap();
    assert!(!app.settle(8).unwrap().pending);
    let root = app.tree().roots()[0];
    let reply = Rc::new(Cell::new(false));
    let result = reply.clone();
    renderer.set_viewport(180, 140, 1.).unwrap();
    app.request(Query::Bounds(root), move |value| {
        value.unwrap();
        result.set(true)
    })
    .unwrap();
    // Query resolved layout already; observation still owes viewport events.
    assert!(renderer.needs_observe());
    assert!(!app.settle(8).unwrap().pending);
    assert!(reply.get());
    let before = sizes.get();
    app.scope()
        .action(Action::ScrollTo {
            node: root,
            x: 0.,
            y: 0.,
        })
        .unwrap();
    app.flush().unwrap();
    assert!(renderer.needs_observe());
    let turn = app.settle(8).unwrap();
    assert!(!turn.pending);
    assert!(turn.handled > 0);
    assert!(sizes.get() >= before);
    assert!(!renderer.needs_observe());
}

#[test]
fn pending_external_io_does_not_spin_but_completed_messages_are_observed() {
    use glory_apace::{
        Config,
        assets::{NetHandler, NetProvider, Request},
    };
    use std::sync::{Arc, Mutex};
    #[derive(Default)]
    struct Held(Mutex<Vec<(String, Box<dyn NetHandler>)>>);
    impl NetProvider for Held {
        fn fetch(&self, _: usize, request: Request, handler: Box<dyn NetHandler>) {
            self.0
                .lock()
                .unwrap()
                .push((request.url.to_string(), handler));
        }
        fn cancel(&self, _: usize, _: &str) {
            let removed = std::mem::take(&mut *self.0.lock().unwrap());
            drop(removed);
        }
        fn close(&self, _: usize) {
            let removed = std::mem::take(&mut *self.0.lock().unwrap());
            drop(removed);
        }
    }
    let provider = Arc::new(Held::default());
    let mut renderer = None;
    let app = App::new(|id| {
        let host = Renderer::with_config(
            id,
            Config {
                provider: Some(provider.clone()),
                ..Default::default()
            },
        );
        renderer = Some(host.clone());
        host
    });
    let renderer = renderer.unwrap();
    app.mount(
        glory_ui::image()
            .source("pending.png")
            .alt("pending")
            .style(Style::new().width(Length::Px(32.)).height(Length::Px(48.))),
    )
    .unwrap();
    let turn = app.settle(8).unwrap();
    assert!(!turn.pending);
    assert!(turn.passes <= 2);
    assert_eq!(renderer.resource_stats().requests, 1);
    assert!(!renderer.needs_observe());
    let (url, handler) = provider.0.lock().unwrap().pop().unwrap();
    handler.error(url, "fixture failure".into());
    assert!(renderer.needs_observe());
    assert!(!app.settle(8).unwrap().pending);
    assert_eq!(renderer.resource_stats().requests, 0);
    assert_eq!(renderer.resource_stats().failures, 1);
}

#[cfg(feature = "raster")]
#[test]
fn huge_measured_rows_converge_before_one_changed_raster_and_keep_final_geometry_pixels() {
    use glory_apace::Raster;
    use glory_ui::list::ListStats;
    for measured in [false, true] {
        let (app, renderer) = fixture();
        let profile = Profile::native();
        app.set_profile(Some(profile.clone()));
        let position = app.scope().cage(0.);
        let reads = Rc::new(Cell::new(0));
        let keys = reads.clone();
        let latest = Rc::new(RefCell::new(None::<ListStats>));
        let stats = latest.clone();
        let color = |key: usize| match key % 3 {
            0 => Color(224, 32, 16, 255),
            1 => Color(16, 192, 48, 255),
            _ => Color(32, 48, 224, 255),
        };
        let widget = list(
            100_000_000usize,
            move |i| {
                keys.set(keys.get() + 1);
                i
            },
            move |_, key| {
                view().style(Style {
                    background: Some(color(key)),
                    ..Style::new().height(Length::Px(if measured {
                        [24., 28., 32.][key % 3]
                    } else {
                        24.
                    }))
                })
            },
        )
        .height(120.)
        .overscan(2)
        .row_limit(20)
        .position(position)
        .inspect(move |s| *stats.borrow_mut() = Some(s));
        app.mount(if measured {
            widget.measure(24.).measure_limit(64)
        } else {
            widget.row_height(24.)
        })
        .unwrap();
        let mut raster = Raster::new();
        let mut max_passes = 0;
        for step in 0..60 {
            position.revise(|v| *v = (step * 19_123_000) as f64 + 7.);
            assert!(profile.reset());
            let turn = app.settle(8).unwrap();
            max_passes = max_passes.max(turn.passes);
            assert!(!turn.pending, "{turn:?}");
            assert_eq!(profile.snapshot().sample(Phase::Paint).calls, 0);
            assert!(raster.draw(&renderer).unwrap().rendered);
            assert_eq!(profile.snapshot().sample(Phase::Draw).calls, 1);
            assert_eq!(profile.snapshot().sample(Phase::Paint).calls, 1);
            let tree = app.tree();
            let root = tree.roots()[0];
            let body = tree.node(root).unwrap().children[0];
            let painted = tree
                .node(body)
                .unwrap()
                .children
                .iter()
                .find_map(|row| {
                    let node = tree.node(*row).unwrap();
                    let key = node.descriptor.view().item.unwrap().index;
                    let physical = renderer.engine_id(*row, glory_html::Part::Main).unwrap();
                    let rect =
                        renderer.inspect(|doc| doc.get_client_bounding_rect(physical).unwrap());
                    (rect.y <= 61. && rect.y + rect.height > 61.).then_some((key, rect))
                })
                .expect("viewport coverage");
            let expected = color(painted.0);
            let at = (61 * 160 + 120) * 4;
            assert_eq!(
                &raster.pixels()[at..at + 4],
                &[expected.0, expected.1, expected.2, expected.3],
                "{painted:?}"
            );
            drop(tree);
            let s = latest.borrow().clone().unwrap();
            assert!(s.rows <= 12 && s.measured <= 64 && s.height_capacity < 4096);
            assert!(app.stats().nodes < 64 && app.runtime_stats().node_slots < 256);
        }
        assert!(reads.get() < 5000);
        assert!(profile.reset());
        for _ in 0..5 {
            assert!(!app.settle(8).unwrap().pending);
            assert!(!raster.draw(&renderer).unwrap().rendered);
        }
        assert_eq!(profile.snapshot().sample(Phase::Draw).calls, 0);
        eprintln!(
            "settled list measured={measured}: max_passes={max_passes}, key_reads={}",
            reads.get()
        );
        app.clear().unwrap();
        assert!(!app.settle(8).unwrap().pending);
        assert_eq!(app.stats().nodes, 0);
    }
}
