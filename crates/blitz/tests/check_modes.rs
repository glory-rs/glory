#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_html::Part;
use glory_render::*;
use glory_ui::{App, check};

#[test]
fn native_typeahead_rejection_matches_preflight_direct_apply_listen_and_query_prefix() {
    let runtime = RuntimeId::new();
    let mut renderer = Renderer::new(runtime);
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.semantics = Some(Box::new(Semantics {
        navigation: Some(Orientation::Horizontal),
        typeahead: true,
        ..Default::default()
    }));
    assert_eq!(
        renderer.preflight((descriptor).view(), None),
        Err(Error::Unsupported("native typeahead input"))
    );
    let patch = Patch::Semantics(Change::Set(descriptor.semantics.clone()));
    assert_eq!(
        renderer.preflight((Descriptor::new(Props::View)).view(), Some(&patch)),
        Err(Error::Unsupported("native typeahead input"))
    );
    assert_eq!(
        renderer.apply(&[Mutation::Create {
            id,
            node: Box::new(descriptor)
        }]),
        Err(Error::Unsupported("native typeahead input"))
    );
    assert_eq!(renderer.stats().nodes, 0);
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id,
                node: Box::new(Descriptor::new(Props::View))
            },
            Mutation::Place {
                id,
                parent: None,
                before: None
            },
            Mutation::Patch { id, patch },
        ]),
        Err(Error::Unsupported("native typeahead input"))
    );
    assert_eq!(renderer.stats().applied, 2);
    assert_eq!(
        renderer.listen(id, EventKind::Typeahead, true),
        Err(Error::Unsupported("native typeahead input"))
    );
    renderer.listen(id, EventKind::Typeahead, false).unwrap();
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
}

#[test]
fn mixed_is_rejected_before_app_allocation_or_patch_and_switch_keeps_real_checked_identity() {
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::new(runtime);
        renderer = Some(host.clone());
        host
    });
    let renderer = renderer.unwrap();
    let before = app.stats();
    for mode in [CheckMode::Mixed, CheckMode::Radio] {
        assert!(app.mount(check().mode(mode)).is_err());
    }
    assert_eq!(app.stats().nodes, before.nodes);
    assert_eq!(renderer.stats().nodes, 0);
    let checked = app.scope().cage(false);
    app.mount(
        check()
            .mode(CheckMode::Switch)
            .checked(checked)
            .label("Notifications"),
    )
    .unwrap();
    let id = app.tree().roots()[0];
    let physical = renderer.engine_id(id, Part::Control).unwrap();
    let before = app.stats();
    assert!(
        app.scope()
            .patch(id, Patch::CheckMode(Change::Set(CheckMode::Mixed)))
            .is_err()
    );
    assert_eq!(app.stats(), before);
    app.request(Query::Bounds(id), |_| {}).unwrap();
    app.pump().unwrap();
    checked.revise(|checked| *checked = true);
    app.flush().unwrap();
    assert_eq!(renderer.engine_id(id, Part::Control).unwrap(), physical);
    renderer.inspect(|doc| {
        assert_eq!(
            doc.get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .checkbox_input_checked(),
            Some(true)
        )
    });
}
#[test]
fn direct_mixed_rejection_keeps_prefix_and_query_watermark_then_accepts_switch() {
    let runtime = RuntimeId::new();
    let mut renderer = Renderer::new(runtime);
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let later = ids.allocate();
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id,
                node: Box::new(Descriptor::new(Props::Check(CheckProps::default())))
            },
            Mutation::Place {
                id,
                parent: None,
                before: None
            },
            Mutation::Patch {
                id,
                patch: Patch::CheckMode(Change::Set(CheckMode::Mixed))
            },
            Mutation::Create {
                id: later,
                node: Box::new(Descriptor::new(Props::View))
            },
        ]),
        Err(Error::Unsupported("native indeterminate checks"))
    );
    assert_eq!(renderer.stats().applied, 2);
    assert_eq!(renderer.stats().nodes, 1);
    renderer.request(Query::Bounds(id), QueryToken(1)).unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            applied: 2,
            token: QueryToken(1),
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
    let before = renderer.stats();
    assert_eq!(
        renderer.apply(&[Mutation::Create {
            id: later,
            node: Box::new(Descriptor::new(Props::Check(CheckProps {
                mode: CheckMode::Mixed,
                ..Default::default()
            })))
        }]),
        Err(Error::Unsupported("native indeterminate checks"))
    );
    assert_eq!(renderer.stats(), before);
    assert_eq!(
        renderer
            .apply(&[Mutation::Patch {
                id,
                patch: Patch::CheckMode(Change::Set(CheckMode::Switch))
            }])
            .unwrap()
            .applied,
        3
    );
}

#[test]
fn native_radio_rejection_checks_descriptors_and_direct_prefixes() {
    let runtime = RuntimeId::new();
    let mut renderer = Renderer::new(runtime);
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let radio = Descriptor::new(Props::Check(CheckProps {
        mode: CheckMode::Radio,
        ..Default::default()
    }));
    assert_eq!(
        renderer.preflight((radio).view(), None),
        Err(Error::Unsupported("native radio groups"))
    );
    let checkbox = Descriptor::new(Props::Check(CheckProps::default()));
    assert_eq!(
        renderer.preflight(
            (checkbox).view(),
            Some(&Patch::CheckMode(Change::Set(CheckMode::Radio)))
        ),
        Err(Error::Unsupported("native radio groups"))
    );
    assert_eq!(
        renderer.apply(&[Mutation::Create {
            id,
            node: Box::new(radio)
        }]),
        Err(Error::Unsupported("native radio groups"))
    );
    assert_eq!(renderer.stats().applied, 0);
    renderer
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(checkbox),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    assert_eq!(
        renderer.apply(&[Mutation::Patch {
            id,
            patch: Patch::CheckMode(Change::Set(CheckMode::Radio))
        }]),
        Err(Error::Unsupported("native radio groups"))
    );
    renderer.request(Query::Bounds(id), QueryToken(9)).unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            applied: 2,
            token: QueryToken(9),
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
}
