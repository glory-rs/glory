#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;
#[test]
fn native_context_policy_rejects_before_facets_and_keeps_accepted_prefix_watermark() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let plain_id = ids.allocate();
    let rejected = ids.allocate();
    let mut plain = Descriptor::new(Props::View);
    plain.role = Some(Role::Group);
    plain.focus = FocusMode::TabStop;
    plain.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("area").unwrap()),
        ..Default::default()
    }));
    let mut refined = plain.clone();
    refined.semantics.as_mut().unwrap().context_menu = Some(ContextEditPolicy::Override);
    let patch = Patch::Semantics(Change::Set(refined.semantics.clone()));
    let mut renderer = Renderer::new(runtime);
    renderer.preflight(plain.view(), None).unwrap();
    assert_eq!(
        renderer.preflight(refined.view(), None),
        Err(Error::Unsupported("native context menu input"))
    );
    assert_eq!(
        renderer.preflight(plain.view(), Some(&patch)),
        Err(Error::Unsupported("native context menu input"))
    );
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id: plain_id,
                node: Box::new(plain)
            },
            Mutation::Place {
                id: plain_id,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: rejected,
                node: Box::new(refined)
            }
        ]),
        Err(Error::Unsupported("native context menu input"))
    );
    assert_eq!(renderer.stats().nodes, 1);
    assert_eq!(renderer.stats().applied, 2);
    assert!(
        renderer
            .engine_id(rejected, glory_html::Part::Main)
            .is_err()
    );
    assert_eq!(
        renderer.apply(&[Mutation::Patch {
            id: plain_id,
            patch
        }]),
        Err(Error::Unsupported("native context menu input"))
    );
    renderer
        .request(Query::Bounds(plain_id), QueryToken(6001))
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
