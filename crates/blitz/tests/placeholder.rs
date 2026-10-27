#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_html::Part;
use glory_render::*;

fn hint(text: &str) -> Option<Box<Semantics>> {
    Some(Box::new(Semantics {
        placeholder: Some(text.into()),
        ..Default::default()
    }))
}
#[test]
fn native_hint_admission_and_mutations_preserve_identity_property_barriers_and_query_prefix() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let first = ids.allocate();
    let second = ids.allocate();
    let mut hinted = Descriptor::new(Props::Input(Default::default()));
    hinted.semantics = hint("Search");
    let mut renderer = Renderer::new(runtime);
    assert_eq!(renderer.preflight(hinted.view(), None), Ok(()));
    assert_eq!(
        renderer.preflight(
            hinted.view(),
            Some(&Patch::Semantics(Change::Set(hint("next"))))
        ),
        Ok(())
    );
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id: first,
                node: Box::new(hinted)
            },
            Mutation::Place {
                id: first,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: second,
                node: Box::new(Descriptor::new(Props::Select(Default::default())))
            },
        ]),
        Err(Error::Unsupported("native select controls"))
    );
    assert_eq!(renderer.stats().nodes, 1);
    assert_eq!(renderer.stats().applied, 2);
    renderer
        .request(Query::Bounds(first), QueryToken(42))
        .unwrap();
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
    let physical = renderer.engine_id(first, Part::Main).unwrap();
    let writes = renderer.stats().property_writes;
    for (change, expected) in [
        (Change::Set(hint("next")), Some("next")),
        (Change::Keep, Some("next")),
        (Change::Set(hint("")), Some("")),
        (Change::Clear, None),
    ] {
        renderer
            .apply(&[Mutation::Patch {
                id: first,
                patch: Patch::Semantics(change),
            }])
            .unwrap();
        renderer
            .request(Query::Bounds(first), QueryToken(43))
            .unwrap();
        inputs.clear();
        renderer.drain(&mut inputs);
        assert_eq!(renderer.engine_id(first, Part::Main).unwrap(), physical);
        assert_eq!(renderer.stats().property_writes, writes);
        renderer.inspect(|doc| {
            let node = doc.get_node(physical).unwrap();
            assert_eq!(
                node.attr(blitz_dom::LocalName::from("placeholder")),
                expected
            );
            let input = node.element_data().unwrap().text_input_data().unwrap();
            assert!(input.editor.raw_text().is_empty());
            assert_eq!(
                input.placeholder_layout().is_some(),
                expected.is_some_and(|s| !s.is_empty())
            );
        });
    }
}

#[cfg(feature = "raster")]
#[test]
fn semantic_hints_paint_for_text_password_and_multiline_then_restore_on_controlled_clear() {
    use glory_blitz::Raster;
    use glory_ui::{App, input};
    for (purpose, multiline) in [
        (Purpose::Text, false),
        (Purpose::Password, false),
        (Purpose::Text, true),
    ] {
        let mut output = None;
        let app = App::new(|runtime| {
            let host = Renderer::new(runtime);
            host.set_viewport(240, 100, 1.).unwrap();
            host.set_css("body{margin:0;background:white}input,textarea{width:140px;height:60px;padding:0;border:0;color:blue;font:18px/22px monospace}");
            output = Some(host.clone());
            host
        });
        app.mount(
            input()
                .purpose(purpose)
                .multiline(multiline)
                .semantics(hint(if multiline { "First\nsecond" } else { "Search" })),
        )
        .unwrap();
        let renderer = output.unwrap();
        let id = app.tree().roots()[0];
        let physical = renderer.engine_id(id, Part::Main).unwrap();
        let mut raster = Raster::new();
        app.settle(8).unwrap();
        raster.draw(&renderer).unwrap();
        let hinted = raster.pixels().to_vec();
        app.scope()
            .patch(id, Patch::Semantics(Change::Clear))
            .unwrap();
        app.settle(8).unwrap();
        raster.draw(&renderer).unwrap();
        let blank = raster.pixels().to_vec();
        assert_ne!(blank, hinted);
        app.scope()
            .patch(
                id,
                Patch::Semantics(Change::Set(hint(if multiline {
                    "First\nsecond"
                } else {
                    "Search"
                }))),
            )
            .unwrap();
        app.scope()
            .patch(
                id,
                Patch::Value {
                    value: Change::Set("Secret中".into()),
                    revision: 1,
                },
            )
            .unwrap();
        app.settle(8).unwrap();
        raster.draw(&renderer).unwrap();
        let filled = raster.pixels().to_vec();
        assert_ne!(filled, hinted);
        let writes = renderer.stats().property_writes;
        app.scope()
            .patch(id, Patch::Semantics(Change::Set(hint("replacement"))))
            .unwrap();
        app.settle(8).unwrap();
        raster.draw(&renderer).unwrap();
        assert_eq!(raster.pixels(), filled);
        assert_eq!(renderer.stats().property_writes, writes);
        app.scope()
            .patch(
                id,
                Patch::Value {
                    value: Change::Clear,
                    revision: 2,
                },
            )
            .unwrap();
        app.settle(8).unwrap();
        raster.draw(&renderer).unwrap();
        assert_ne!(raster.pixels(), blank);
        renderer.inspect(|doc| {
            let data = doc
                .get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .text_input_data()
                .unwrap();
            assert!(data.editor.raw_text().is_empty());
            assert!(data.display_editor().raw_text().is_empty());
            assert!(data.placeholder_layout().is_some());
        });
        assert_eq!(renderer.engine_id(id, Part::Main).unwrap(), physical);
        app.clear().unwrap();
        assert_eq!(renderer.stats().nodes, 0);
    }
}
