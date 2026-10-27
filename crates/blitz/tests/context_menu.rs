#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;
#[test]
fn native_context_policy_admission_keeps_later_rejection_prefix_watermark() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let rejected = ids.allocate();
    let mut region = Descriptor::new(Props::View);
    region.role = Some(Role::Group);
    region.focus = FocusMode::TabStop;
    region.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("area").unwrap()),
        context_menu: Some(ContextEditPolicy::Override),
        ..Default::default()
    }));
    let mut renderer = Renderer::new(runtime);
    renderer.preflight(region.view(), None).unwrap();
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id,
                node: Box::new(region)
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
    renderer.listen(id, EventKind::ContextMenu, true).unwrap();
    assert_eq!(renderer.stats().applied, 2);
    renderer
        .request(Query::Bounds(id), QueryToken(6001))
        .unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            token: QueryToken(6001),
            applied: 2,
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
}
