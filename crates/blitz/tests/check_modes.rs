#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_html::Part;
use glory_render::*;
use glory_ui::{App, check};

#[cfg(feature = "raster")]
#[test]
fn semantic_mixed_properties_reach_cpu_pixels_at_both_dpi_scales() {
    use glory_blitz::Raster;
    for scale in [1., 2.] {
        let mut renderer = None;
        let app = App::new(|runtime| {
            let host = Renderer::new(runtime);
            host.set_viewport((120. * scale) as u32, (64. * scale) as u32, scale)
                .unwrap();
            renderer = Some(host.clone());
            host
        });
        let renderer = renderer.unwrap();
        let mode = app.scope().cage(CheckMode::Checkbox);
        let checked = app.scope().cage(false);
        app.mount(check().label("State").mode(mode).checked(checked))
            .unwrap();
        let mut raster = Raster::new();
        let mut frames = Vec::new();
        for (m, c) in [
            (CheckMode::Checkbox, false),
            (CheckMode::Checkbox, true),
            (CheckMode::Mixed, true),
            (CheckMode::Mixed, false),
            (CheckMode::Checkbox, false),
            (CheckMode::Checkbox, true),
        ] {
            mode.revise(|v| *v = m);
            checked.revise(|v| *v = c);
            app.flush().unwrap();
            raster.draw(&renderer).unwrap();
            frames.push(raster.pixels().to_vec());
        }
        assert!(frames[0] != frames[1], "boolean states differ");
        assert!(frames[1] != frames[2], "mixed differs from checked");
        assert!(frames[0] != frames[2], "mixed differs from unchecked");
        assert!(
            frames[2] == frames[3],
            "mixed painting is independent of checkedness"
        );
        assert!(
            frames[0] == frames[4],
            "clearing mixed restores unchecked pixels"
        );
        assert!(
            frames[1] == frames[5],
            "clearing mixed restores checked pixels"
        );
    }
}

#[test]
fn native_typeahead_admission_keeps_prefix_watermarks_and_does_not_install_an_input_driver_in_headless_mode()
 {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let rejected = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.semantics = Some(Box::new(Semantics {
        navigation: Some(Orientation::Horizontal),
        typeahead: true,
        ..Default::default()
    }));
    renderer.preflight(descriptor.view(), None).unwrap();
    renderer
        .preflight(
            Descriptor::new(Props::View).view(),
            Some(&Patch::Semantics(Change::Set(descriptor.semantics.clone()))),
        )
        .unwrap();
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id,
                node: Box::new(descriptor)
            },
            Mutation::Place {
                id,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: rejected,
                node: Box::new(Descriptor::new(Props::File(Default::default())))
            }
        ]),
        Err(Error::Unsupported("native file picker controls"))
    );
    renderer.listen(id, EventKind::Typeahead, true).unwrap();
    assert!(!renderer.capabilities().input);
    assert_eq!(renderer.stats().nodes, 1);
    renderer.request(Query::Bounds(id), QueryToken(42)).unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            token: QueryToken(42),
            applied: 2,
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
    renderer.listen(id, EventKind::Typeahead, false).unwrap();
}

#[test]
fn mixed_and_boolean_modes_share_a_control_and_preserve_the_independent_checked_value() {
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::new(runtime);
        renderer = Some(host.clone());
        host
    });
    let renderer = renderer.unwrap();
    let mode = app.scope().cage(CheckMode::Mixed);
    let checked = app.scope().cage(false);
    app.mount(check().mode(mode).checked(checked).label("Notifications"))
        .unwrap();
    let id = app.tree().roots()[0];
    let physical = renderer.engine_id(id, Part::Control).unwrap();
    let states = [
        (CheckMode::Mixed, false, true),
        (CheckMode::Mixed, true, true),
        (CheckMode::Checkbox, true, false),
        (CheckMode::Switch, false, false),
        (CheckMode::Mixed, false, true),
    ];
    for (next_mode, next_checked, mixed) in states {
        mode.revise(|value| *value = next_mode);
        checked.revise(|value| *value = next_checked);
        app.flush().unwrap();
        app.request(Query::Bounds(id), |_| {}).unwrap();
        app.pump().unwrap();
        assert_eq!(renderer.engine_id(id, Part::Control).unwrap(), physical);
        renderer.inspect(|doc| {
            let element = doc.get_node(physical).unwrap().element_data().unwrap();
            assert_eq!(element.checkbox_input_checked(), Some(next_checked));
            assert_eq!(element.checkbox_input_indeterminate().unwrap(), mixed);
        });
    }
    app.scope()
        .patch(id, Patch::CheckMode(Change::Clear))
        .unwrap();
    app.flush().unwrap();
    app.request(Query::Bounds(id), |_| {}).unwrap();
    app.pump().unwrap();
    renderer.inspect(|doc| {
        assert!(
            !doc.get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .checkbox_input_indeterminate()
                .unwrap()
        )
    });
}

#[test]
fn direct_mixed_create_patch_and_clear_keep_the_accepted_query_watermark() {
    let runtime = RuntimeId::new();
    let mut renderer = Renderer::new(runtime);
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    renderer
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(Descriptor::new(Props::Check(CheckProps {
                    mode: CheckMode::Mixed,
                    ..Default::default()
                }))),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let physical = renderer.engine_id(id, Part::Control).unwrap();
    for (index, mode) in [CheckMode::Checkbox, CheckMode::Mixed, CheckMode::Switch]
        .into_iter()
        .enumerate()
    {
        renderer
            .apply(&[Mutation::Patch {
                id,
                patch: Patch::CheckMode(Change::Set(mode)),
            }])
            .unwrap();
        renderer
            .request(Query::Bounds(id), QueryToken(index as u64))
            .unwrap();
        let mut inputs = Vec::new();
        renderer.drain(&mut inputs);
        assert!(
            matches!(inputs.as_slice(),[Input::Response { applied,result:Ok(QueryValue::Bounds(_)),..}] if *applied==3+index as u64)
        );
        assert_eq!(renderer.engine_id(id, Part::Control).unwrap(), physical);
        renderer.inspect(|doc| {
            assert_eq!(
                doc.get_node(physical)
                    .unwrap()
                    .element_data()
                    .unwrap()
                    .checkbox_input_indeterminate()
                    .unwrap(),
                mode == CheckMode::Mixed
            )
        });
    }
}

#[test]
fn native_radio_creation_mode_changes_and_query_watermarks_preserve_the_control() {
    let runtime = RuntimeId::new();
    let mut renderer = Renderer::new(runtime);
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let radio = Descriptor::new(Props::Check(CheckProps {
        mode: CheckMode::Radio,
        checked: true,
        ..Default::default()
    }));
    assert_eq!(renderer.preflight(radio.view(), None), Ok(()));
    renderer
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(radio),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let physical = renderer.engine_id(id, Part::Control).unwrap();
    for (index, mode) in [
        CheckMode::Checkbox,
        CheckMode::Radio,
        CheckMode::Mixed,
        CheckMode::Switch,
    ]
    .into_iter()
    .enumerate()
    {
        renderer
            .apply(&[Mutation::Patch {
                id,
                patch: Patch::CheckMode(Change::Set(mode)),
            }])
            .unwrap();
        renderer
            .request(Query::Bounds(id), QueryToken(index as u64))
            .unwrap();
        let mut inputs = Vec::new();
        renderer.drain(&mut inputs);
        assert!(
            matches!(inputs.as_slice(),[Input::Response{applied,result:Ok(QueryValue::Bounds(_)),..}] if *applied==3+index as u64)
        );
        assert_eq!(renderer.engine_id(id, Part::Control).unwrap(), physical);
        renderer.inspect(|doc| {
            let element = doc.get_node(physical).unwrap().element_data().unwrap();
            assert_eq!(element.checkbox_input_checked(), Some(true));
            assert_eq!(
                element.attr(blitz_dom::LocalName::from("type")),
                Some(if mode == CheckMode::Radio {
                    "radio"
                } else {
                    "checkbox"
                })
            );
        });
    }
}
