#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;
#[test]
fn native_menu_layer_rejection_preserves_initial_model_direct_prefix_and_query_watermark() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let first = ids.allocate();
    let later = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.role = Some(Role::Menu);
    descriptor.focus = FocusMode::Programmatic;
    descriptor.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("native-menu").unwrap()),
        navigation: Some(Orientation::Vertical),
        typeahead: true,
        layer: Some(Layer {
            modal: false,
            ..Default::default()
        }),
        ..Default::default()
    }));
    assert_eq!(
        renderer.preflight(descriptor.view(), None),
        Err(Error::Unsupported("native top layers"))
    );
    let patch = Patch::Semantics(Change::Set(descriptor.semantics.clone()));
    let mut base = Descriptor::new(Props::View);
    base.role = Some(Role::Menu);
    base.focus = FocusMode::Programmatic;
    assert_eq!(
        renderer.preflight(base.view(), Some(&patch)),
        Err(Error::Unsupported("native top layers"))
    );
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id: first,
                node: Box::new(base)
            },
            Mutation::Place {
                id: first,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: later,
                node: Box::new(descriptor)
            }
        ]),
        Err(Error::Unsupported("native top layers"))
    );
    assert_eq!(renderer.stats().nodes, 1);
    assert_eq!(renderer.stats().applied, 2);
    assert_eq!(
        renderer.apply(&[Mutation::Patch { id: first, patch }]),
        Err(Error::Unsupported("native top layers"))
    );
    assert_eq!(renderer.stats().applied, 2);
    assert_eq!(
        renderer.listen(first, EventKind::Menu, true),
        Err(Error::Unsupported("native menu input"))
    );
    renderer.listen(first, EventKind::Menu, false).unwrap();
    renderer.listen(first, EventKind::Pointer, true).unwrap();
    renderer.listen(first, EventKind::Pointer, false).unwrap();
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
