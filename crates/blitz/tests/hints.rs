#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;

#[test]
fn native_hint_layers_keep_explicit_rejection_while_pointer_within_admission_preserves_prefix_watermark()
 {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut renderer = Renderer::new(runtime);
    let first = ids.allocate();
    let later = ids.allocate();
    let mut tip = Descriptor::new(Props::View);
    tip.role = Some(Role::Tooltip);
    tip.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("native-tip").unwrap()),
        layer: Some(Layer {
            modal: false,
            focus: LayerFocus::Preserve,
            placement: LayerPlacement::Anchor(glory_render::AnchorPlacement::new(
                glory_render::AnchorSide::Bottom,
                glory_render::AnchorAlign::Start,
            )),
            anchor: Some(SemanticId::new("native-anchor").unwrap()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    assert_eq!(
        renderer.preflight(tip.view(), None),
        Err(Error::Unsupported("native top layers"))
    );
    let patch = Patch::Semantics(Change::Set(tip.semantics.clone()));
    let mut bare = Descriptor::new(Props::View);
    bare.role = Some(Role::Tooltip);
    assert_eq!(
        renderer.preflight(bare.view(), Some(&patch)),
        Err(Error::Unsupported("native top layers"))
    );
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id: first,
                node: Box::new(bare)
            },
            Mutation::Place {
                id: first,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: later,
                node: Box::new(tip)
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
    renderer
        .listen(first, EventKind::PointerWithin, true)
        .unwrap();
    renderer
        .listen(first, EventKind::PointerWithin, false)
        .unwrap();
    renderer
        .request(Query::Bounds(first), QueryToken(73))
        .unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            token: QueryToken(73),
            applied: 2,
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
}
