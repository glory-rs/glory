#![cfg(not(target_arch = "wasm32"))]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use glory_apace::Renderer;
use glory_html::Part;
use glory_render::style::{Length, Style};
use glory_render::*;
use glory_ui::{App, Widget, button, each, group, input, list, text, view};

fn app() -> (App, Renderer) {
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::new(runtime);
        renderer = Some(host.clone());
        host
    });
    (app, renderer.unwrap())
}
fn text_content(renderer: &Renderer) -> String {
    renderer.inspect(|doc| {
        doc.find_element_by_tag_name(&"body".into())
            .unwrap()
            .text_content()
    })
}
fn query(app: &App, query: Query) -> Result<QueryValue, Error> {
    let result = Rc::new(RefCell::new(None));
    let reply = result.clone();
    app.request(query, move |value| *reply.borrow_mut() = Some(value))
        .unwrap();
    app.pump().unwrap();
    result.borrow_mut().take().unwrap()
}
fn bounds(app: &App, node: NodeId) -> Rect {
    let QueryValue::Bounds(rect) = query(app, Query::Bounds(node)).unwrap() else {
        panic!("bounds")
    };
    rect
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= 0.03, "{a} != {b}");
}

#[cfg(feature = "css")]
#[test]
fn portable_sheets_drive_real_cascade_and_semantic_states_without_remounting() {
    use glory_render::style::sheet::ErrorKind;
    use glory_render::style::{Sheet, Value};
    let (app, renderer) = app();
    let sheet=Sheet::parse(".pane{display:flex;flex-direction:column;width:300px;gap:5px} .pane>.box{width:80px;height:20px} .box.hot{width:120px} .box{width:90px} .toggle{display:block;width:70px;height:20px} .toggle:checked{height:40px} .toggle:disabled{width:100px}").unwrap();
    renderer.set_sheet(&sheet).unwrap();
    let checked = app.scope().cage(false);
    let enabled = app.scope().cage(true);
    app.mount(
        view().class("pane").fill((
            group().fill(view().class("box hot")),
            glory_ui::check()
                .class("toggle")
                .checked(checked)
                .enabled(enabled),
        )),
    )
    .unwrap();
    let pane = app.tree().roots()[0];
    let children = app.tree().node(pane).unwrap().children.clone();
    let boxed = app.tree().node(children[0]).unwrap().children[0];
    let toggle = children[1];
    let identity = renderer.engine_id(toggle, Part::Control).unwrap();
    close(bounds(&app, boxed).width, 120.0);
    close(bounds(&app, toggle).width, 70.0);
    close(bounds(&app, toggle).height, 20.0);
    checked.revise(|v| *v = true);
    enabled.revise(|v| *v = false);
    app.flush().unwrap();
    close(bounds(&app, toggle).width, 100.0);
    close(bounds(&app, toggle).height, 40.0);
    assert_eq!(renderer.engine_id(toggle, Part::Control).unwrap(), identity);
    let before = renderer.stats();
    let bad = Sheet::parse(".box{position:sticky}").unwrap();
    let error = renderer.set_sheet(&bad).unwrap_err();
    assert!(matches!(
        error.kind,
        ErrorKind::Unsupported(glory_render::style::Unsupported {
            value: Some(Value::Sticky),
            ..
        })
    ));
    assert_eq!(renderer.stats(), before);
    close(bounds(&app, boxed).width, 120.0);
    renderer
        .set_sheet(&Sheet::parse(".box{width:140px;height:20px}").unwrap())
        .unwrap();
    close(bounds(&app, boxed).width, 140.0);
    app.clear().unwrap();
    assert_eq!(renderer.stats().nodes, 0);
}

#[test]
fn animation_clock_is_monotonic_and_queries_do_not_reset_it_to_zero() {
    let (app, renderer) = app();
    renderer.set_css("@keyframes widen{from{width:10px}to{width:110px}} .box{height:20px;animation:widen 2s linear forwards}");
    app.mount(view().class("box")).unwrap();
    let root = app.tree().roots()[0];
    close(bounds(&app, root).width, 10.0);
    let before = renderer.stats().visual_revision;
    renderer.advance_time(1.0).unwrap();
    assert!(renderer.stats().visual_revision > before);
    close(bounds(&app, root).width, 60.0);
    renderer.invalidate();
    close(bounds(&app, root).width, 60.0);
    renderer.advance_time(2.0).unwrap();
    close(bounds(&app, root).width, 110.0);
    assert!(renderer.advance_time(1.0).is_err());
    assert!(renderer.advance_time(f64::NAN).is_err());
    app.clear().unwrap();
    let before = renderer.stats().visual_revision;
    renderer.advance_time(3.0).unwrap();
    assert_eq!(
        renderer.stats().visual_revision,
        before,
        "static time advance has no visual cost"
    );
}

#[test]
fn semantic_builders_patch_the_existing_engine_parts_without_erasing_children() {
    let (app, renderer) = app();
    let label = app.scope().cage(String::from("before"));
    app.mount(button().label(label).fill(text().content(" icon")))
        .unwrap();
    let root = app.tree().roots()[0];
    let engine = renderer.engine_id(root, Part::Main).unwrap();
    let content = renderer.engine_id(root, Part::Content).unwrap();
    assert_eq!(text_content(&renderer), "before icon");
    label.revise(|value| *value = "after".into());
    app.flush().unwrap();
    assert_eq!(text_content(&renderer), "after icon");
    assert_eq!(renderer.engine_id(root, Part::Main).unwrap(), engine);
    assert_eq!(renderer.engine_id(root, Part::Content).unwrap(), content);
    assert_eq!(app.style_profile().name, "apace");
    assert!(!app.scope().capabilities().unwrap().input);
    assert!(matches!(
        app.scope().action(Action::Focus(root)),
        Err(glory_ui::Error::Render(Error::Unsupported(_)))
    ));
    app.clear().unwrap();
    assert_eq!(renderer.stats().nodes, 0);
    assert!(
        renderer.inspect(|doc| doc.get_node(engine).is_none() && doc.get_node(content).is_none())
    );
}

#[test]
fn real_css_flex_layout_and_query_barriers_update_without_reparsing_the_tree() {
    let (app, renderer) = app();
    renderer.set_css(".row{display:flex;gap:10px;width:300px;height:100px}.a{width:50px;height:20px}.b{width:70px;height:30px}");
    app.mount(
        view()
            .class("row")
            .fill((view().class("a"), view().class("b"))),
    )
    .unwrap();
    let root = app.tree().roots()[0];
    let children = app.tree().node(root).unwrap().children.clone();
    let a = children[0];
    let b = children[1];
    let native = renderer.engine_id(b, Part::Main).unwrap();
    let ra = bounds(&app, a);
    let rb = bounds(&app, b);
    close(ra.width, 50.0);
    close(ra.height, 20.0);
    close(rb.x - ra.x, 60.0);
    close(rb.width, 70.0);
    let layouts = renderer.stats().layouts;
    bounds(&app, a);
    bounds(&app, b);
    assert_eq!(
        renderer.stats().layouts,
        layouts,
        "unchanged barriers reuse resolved layout"
    );
    app.scope()
        .patch(
            a,
            Patch::Style(Change::Set(Box::new(Style::new().width(Length::Px(80.0))))),
        )
        .unwrap();
    // Request must apply the pending style before measuring.
    close(bounds(&app, b).x, 90.0);
    assert_eq!(renderer.engine_id(b, Part::Main).unwrap(), native);
    renderer.set_css(".row{display:flex;gap:20px;width:300px;height:100px}.a{height:20px}.b{width:70px;height:30px}");
    close(bounds(&app, b).x, 100.0);
    assert!(renderer.stats().layouts > layouts);
    assert!(matches!(
        query(&app, Query::Selection(a)),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn group_ranges_move_suspend_resume_and_dispose_offline_descendants() {
    let (app, renderer) = app();
    let baseline = renderer.stats().engine_nodes;
    let first = group()
        .fill((text().content("A"), group().fill(text().content("B"))))
        .mount(app.scope())
        .unwrap();
    let second = group()
        .fill(text().content("C"))
        .mount(app.scope())
        .unwrap();
    app.flush().unwrap();
    assert_eq!(text_content(&renderer), "ABC");
    assert!(matches!(
        query(&app, Query::Bounds(first.roots()[0])),
        Err(Error::Unsupported(_))
    ));
    second.place_before(Some(&first)).unwrap();
    app.flush().unwrap();
    assert_eq!(text_content(&renderer), "CAB");
    first.suspend().unwrap();
    app.flush().unwrap();
    assert_eq!(text_content(&renderer), "C");
    first.place_before(Some(&second)).unwrap();
    app.flush().unwrap();
    assert_eq!(text_content(&renderer), "ABC");
    // Dispose a logical parent containing a separately parked descendant.
    let outer = view().mount(app.scope()).unwrap();
    let inner_scope = outer.scope().child_in(outer.roots()[0]).unwrap();
    let hidden = group()
        .fill(input().value("offline"))
        .mount(&inner_scope)
        .unwrap();
    app.flush().unwrap();
    hidden.suspend().unwrap();
    app.flush().unwrap();
    drop(outer);
    app.flush().unwrap();
    assert!(!inner_scope.is_alive());
    drop(hidden);
    drop(first);
    drop(second);
    app.flush().unwrap();
    app.pump().unwrap();
    assert_eq!(renderer.stats().nodes, 0);
    assert_eq!(renderer.stats().engine_nodes, baseline);
}
#[test]
fn keyed_reorders_keep_engine_inputs_and_do_not_replay_controlled_values() {
    let (app, renderer) = app();
    let keys = app.scope().cage(vec![1, 2, 3]);
    app.mount(each(keys).key(|key| *key).children(|entry| {
        let key = entry.value;
        input().value(key.map(|key| format!("row {key}")))
    }))
    .unwrap();
    let parent = app.tree().roots()[0];
    let original = app.tree().node(parent).unwrap().children.clone();
    let engine: Vec<_> = original
        .iter()
        .map(|id| renderer.engine_id(*id, Part::Main).unwrap())
        .collect();
    let writes = renderer.stats().property_writes;
    keys.revise(|keys| keys.reverse());
    app.flush().unwrap();
    let now = app.tree().node(parent).unwrap().children.clone();
    assert_eq!(now, original.into_iter().rev().collect::<Vec<_>>());
    assert_eq!(
        now.iter()
            .map(|id| renderer.engine_id(*id, Part::Main).unwrap())
            .collect::<Vec<_>>(),
        engine.into_iter().rev().collect::<Vec<_>>()
    );
    assert_eq!(renderer.stats().property_writes, writes);
    app.clear().unwrap();
    assert_eq!(renderer.stats().nodes, 0);
}

#[test]
fn controlled_form_state_survives_detached_construction_and_offline_patches() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let root = ids.allocate();
    let editor = ids.allocate();
    let check = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    let create = |id, props| Mutation::Create {
        id,
        node: Box::new(Descriptor::new(props)),
    };
    renderer
        .apply(&[
            create(root, Props::Group),
            create(
                editor,
                Props::Input(InputProps {
                    multiline: true,
                    value: "one\ntwo".into(),
                    ..InputProps::default()
                }),
            ),
            create(
                check,
                Props::Check(CheckProps {
                    checked: true,
                    enabled: false,
                    ..CheckProps::default()
                }),
            ),
            Mutation::Place {
                id: editor,
                parent: Some(root),
                before: None,
            },
            Mutation::Place {
                id: check,
                parent: Some(root),
                before: None,
            },
            Mutation::Place {
                id: root,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let control = renderer.engine_id(check, Part::Control).unwrap();
    let input = renderer.engine_id(editor, Part::Main).unwrap();
    renderer.drain(&mut Vec::new());
    let inspected = renderer.clone();
    let form_state = || {
        inspected.inspect(|doc| {
            let editor = doc
                .get_node(input)
                .unwrap()
                .element_data()
                .unwrap()
                .text_input_data()
                .unwrap()
                .editor
                .text()
                .to_string();
            let check = doc
                .get_node(control)
                .unwrap()
                .element_data()
                .unwrap()
                .checkbox_input_checked();
            (editor, check)
        })
    };
    assert_eq!(form_state(), ("one\ntwo".into(), Some(true)));
    renderer
        .apply(&[
            Mutation::Suspend { id: root },
            Mutation::Patch {
                id: editor,
                patch: Patch::Value {
                    value: Change::Set("new\n☃".into()),
                    revision: 1,
                },
            },
            Mutation::Patch {
                id: check,
                patch: Patch::Checked(Change::Set(false)),
            },
            Mutation::Patch {
                id: check,
                patch: Patch::Enabled(Change::Set(true)),
            },
            Mutation::Resume {
                id: root,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    renderer.drain(&mut Vec::new());
    assert_eq!(form_state(), ("new\n☃".into(), Some(false)));
    assert_eq!(renderer.engine_id(editor, Part::Main).unwrap(), input);
    renderer.apply(&[Mutation::Dispose { id: root }]).unwrap();
    assert!(
        renderer.inspect(|doc| doc.get_node(input).is_none() && doc.get_node(control).is_none())
    );
    assert_eq!(renderer.stats().nodes, 0);
}

#[test]
fn viewport_scale_and_resize_observations_use_layout_pixels() {
    let (app, renderer) = app();
    let sizes = Rc::new(RefCell::new(Vec::new()));
    let measured = sizes.clone();
    app.mount(
        view()
            .style(
                Style::new()
                    .width(Length::Percent(100.0))
                    .height(Length::Px(40.5)),
            )
            .on(glory_ui::events::resize, move |event| {
                if let Event::Resize { size, viewport } = event {
                    measured.borrow_mut().push((size, viewport));
                }
            }),
    )
    .unwrap();
    let root = app.tree().roots()[0];
    app.pump().unwrap();
    close(bounds(&app, root).width, 800.0);
    renderer.set_viewport(800, 600, 2.0).unwrap();
    app.pump().unwrap();
    close(bounds(&app, root).width, 400.0);
    close(bounds(&app, root).height, 40.5);
    assert_eq!(sizes.borrow().len(), 2);
    assert!(renderer.set_viewport(800, 600, f32::NAN).is_err());
    assert!(renderer.set_viewport(800, 600, 0.0).is_err());
    app.pump().unwrap();
    assert_eq!(sizes.borrow().len(), 2);
}

#[test]
fn system_font_text_wraps_and_reflows_in_the_real_engine() {
    let (app, renderer) = app();
    renderer.set_viewport(120, 600, 1.0).unwrap();
    app.mount(text().kind(TextKind::Paragraph).content(
        "Layout and text shaping happen in Apace. This paragraph must wrap across several lines.",
    ))
    .unwrap();
    let root = app.tree().roots()[0];
    let narrow = bounds(&app, root);
    renderer.set_viewport(600, 600, 1.0).unwrap();
    let wide = bounds(&app, root);
    assert!(narrow.height > wide.height * 2.0, "{narrow:?} vs {wide:?}");
    assert!(wide.height > 0.0);
}

#[test]
fn scrolling_moves_content_but_preserves_the_scroller_border_box() {
    let (app, renderer) = app();
    renderer.set_css(".scroll{width:200px;height:100px;overflow:auto}.long{height:1000px}");
    app.mount(view().class("scroll").fill(view().class("long")))
        .unwrap();
    let root = app.tree().roots()[0];
    let child = app.tree().node(root).unwrap().children[0];
    let before_root = bounds(&app, root);
    let before_child = bounds(&app, child);
    app.scope()
        .action(Action::ScrollTo {
            node: root,
            x: 0.0,
            y: 90.0,
        })
        .unwrap();
    app.flush().unwrap();
    let after_root = bounds(&app, root);
    let after_child = bounds(&app, child);
    close(after_root.y, before_root.y);
    close(after_child.y, before_child.y - 90.0);
    assert_eq!(
        query(&app, Query::Scroll(root)),
        Ok(QueryValue::Scroll { x: 0.0, y: 90.0 })
    );
}

#[test]
fn hundred_million_rows_stay_windowed_in_the_actual_layout_engine() {
    for adaptive in [false, true] {
        let (app, renderer) = app();
        let baseline = renderer.stats().engine_nodes;
        let position = app.scope().cage(0.0);
        let reads = Rc::new(Cell::new(0));
        let read = reads.clone();
        let latest = Rc::new(RefCell::new(None));
        let inspect = latest.clone();
        let widget = list(
            100_000_000usize,
            move |index| {
                read.set(read.get() + 1);
                index
            },
            move |_index, key| {
                view()
                    .style(Style::new().height(Length::Px(if adaptive {
                        24.0 + (key % 3) as f64 * 4.0
                    } else {
                        24.0
                    })))
                    .fill(text().content(format!("row {key}")))
            },
        )
        .height(240.0)
        .overscan(2)
        .position(position)
        .locate(|key| Some(*key))
        .inspect(move |stats| *inspect.borrow_mut() = Some(stats));
        app.mount(if adaptive {
            widget.measure(24.0).measure_limit(64)
        } else {
            widget.row_height(24.0)
        })
        .unwrap();
        let root = app.tree().roots()[0];
        for _ in 0..4 {
            app.pump().unwrap();
        }
        let mut max_rows = 0;
        let mut max_engine = 0;
        let mut max_slots = 0;
        let mut max_graph = 0;
        for i in 0..25 {
            position.revise(|value| {
                *value = if i % 2 == 0 {
                    3_000_000_000.0
                } else {
                    i as f64 * 7919.0 * 2400.0
                }
            });
            app.flush().unwrap();
            for _ in 0..4 {
                app.pump().unwrap();
            }
            let stats = latest.borrow().clone().unwrap();
            max_rows = max_rows.max(stats.rows);
            max_engine = max_engine.max(renderer.stats().engine_nodes);
            max_slots = max_slots.max(app.stats().node_slots);
            max_graph = max_graph.max(app.runtime_stats().node_slots);
            assert!(stats.rows <= 15, "{stats:?}");
            assert!(stats.height_capacity < 1000);
            assert!(renderer.stats().engine_nodes < 160);
            assert!(app.stats().node_slots < 100);
            assert!(app.runtime_stats().node_slots < 256);
            if adaptive {
                assert!(stats.measured > 0 && stats.measured <= 64);
                assert!(stats.total > 2_400_000_000.0);
            }
            assert!(bounds(&app, root).height <= 240.03);
        }
        assert!(reads.get() < 4000, "indexed reads: {}", reads.get());
        position.revise(|value| *value = 3_000_000_000.0);
        app.flush().unwrap();
        for _ in 0..4 {
            app.pump().unwrap();
        }
        assert!(text_content(&renderer).contains("row 99999999"));
        let surface = app.tree().node(root).unwrap().children[0];
        let last = *app.tree().node(surface).unwrap().children.last().unwrap();
        let outer = bounds(&app, root);
        let tail = bounds(&app, last);
        assert!(
            tail.y >= outer.y - 0.03 && tail.y + tail.height <= outer.y + outer.height + 0.03,
            "tail {tail:?} outside {outer:?}"
        );
        position.revise(|value| *value = 24_000_000.0);
        app.flush().unwrap();
        for _ in 0..4 {
            app.pump().unwrap();
        }
        let before = latest.borrow().clone().unwrap().logical;
        let QueryValue::Scroll { y, .. } = query(&app, Query::Scroll(root)).unwrap() else {
            panic!("scroll")
        };
        app.scope()
            .action(Action::ScrollTo {
                node: root,
                x: 0.0,
                y: y + 1.0,
            })
            .unwrap();
        app.flush().unwrap();
        for _ in 0..4 {
            app.pump().unwrap();
        }
        close(latest.borrow().clone().unwrap().logical - before, 1.0);
        app.clear().unwrap();
        app.pump().unwrap();
        assert_eq!(renderer.stats().nodes, 0);
        assert_eq!(renderer.stats().interests, 0);
        assert_eq!(renderer.stats().engine_nodes, baseline);
        assert_eq!(app.runtime_stats().nodes, 1);
        assert_eq!(app.runtime_stats().edges, 0);
        assert_eq!(app.runtime_stats().tasks, 0);
        eprintln!(
            "Apace adaptive={adaptive}: rows={max_rows}, engine_nodes={max_engine}, semantic_slots={max_slots}, graph_slots={max_graph}, key_reads={}",
            reads.get()
        );
    }
}
