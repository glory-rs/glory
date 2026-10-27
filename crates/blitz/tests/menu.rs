#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;
#[test]
fn native_menu_layer_admission_preserves_later_rejection_prefix_and_query_watermark() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let first = ids.allocate();
    let later = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    let mut menu = Descriptor::new(Props::View);
    menu.role = Some(Role::Menu);
    menu.focus = FocusMode::Programmatic;
    menu.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("native-menu").unwrap()),
        navigation: Some(Orientation::Vertical),
        typeahead: true,
        layer: Some(Layer {
            modal: false,
            ..Default::default()
        }),
        ..Default::default()
    }));
    renderer.preflight(menu.view(), None).unwrap();
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id: first,
                node: Box::new(menu)
            },
            Mutation::Place {
                id: first,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: later,
                node: Box::new(Descriptor::new(Props::File(Default::default())))
            }
        ]),
        Err(Error::Unsupported("native file picker controls"))
    );
    assert_eq!(renderer.stats().nodes, 1);
    assert_eq!(renderer.stats().applied, 2);
    for kind in [EventKind::Menu, EventKind::Pointer, EventKind::Typeahead] {
        renderer.listen(first, kind, true).unwrap();
        renderer.listen(first, kind, false).unwrap();
    }
    renderer
        .request(Query::Bounds(first), QueryToken(9))
        .unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            token: QueryToken(9),
            applied: 2,
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
}
