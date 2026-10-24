#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;

fn layer() -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.role = Some(Role::Dialog);
    d.focus = FocusMode::Programmatic;
    d.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("layer").unwrap()),
        layer: Some(Layer::default()),
        ..Default::default()
    }));
    d
}
#[test]
fn native_layers_reject_in_preflight_and_direct_apply_without_losing_prefix() {
    let runtime = RuntimeId::new();
    let mut renderer = Renderer::new(runtime);
    let error = Error::Unsupported("native top layers");
    let d = layer();
    assert_eq!(renderer.preflight((d).view(), None), Err(error.clone()));
    let mut ids = NodeIds::new(runtime);
    let first = ids.allocate();
    let rejected = ids.allocate();
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id: first,
                node: Box::new(Descriptor::new(Props::View))
            },
            Mutation::Place {
                id: first,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: rejected,
                node: Box::new(d.clone())
            },
        ]),
        Err(error.clone())
    );
    assert_eq!(renderer.stats().applied, 2);
    assert_eq!(renderer.stats().nodes, 1);
    renderer
        .request(Query::Bounds(first), QueryToken(1))
        .unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            applied: 2,
            result: Ok(_),
            ..
        }]
    ));
    let patch = Patch::Semantics(Change::Set(d.semantics));
    assert_eq!(
        renderer.preflight((Descriptor::new(Props::View)).view(), Some(&patch)),
        Err(error.clone())
    );
    assert_eq!(
        renderer.apply(&[Mutation::Patch { id: first, patch }]),
        Err(error)
    );
    assert_eq!(renderer.stats().applied, 2);
}
