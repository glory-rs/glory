#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;

#[test]
fn native_select_is_rejected_before_facets_and_preserves_accepted_prefix_query_watermark() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let first = ids.allocate();
    let select = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    let descriptor = Descriptor::new(Props::Select(SelectProps::default()));
    assert_eq!(
        renderer.preflight(descriptor.view(), None),
        Err(Error::Unsupported("native select controls"))
    );
    assert_eq!(
        renderer.preflight(
            descriptor.view(),
            Some(&Patch::SelectSelection {
                keys: Change::Clear,
                revision: 1
            })
        ),
        Err(Error::Unsupported("native select controls"))
    );
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
                id: select,
                node: Box::new(descriptor)
            },
        ]),
        Err(Error::Unsupported("native select controls"))
    );
    assert_eq!(renderer.stats().nodes, 1);
    assert_eq!(renderer.stats().applied, 2);
    assert!(renderer.engine_id(select, glory_html::Part::Main).is_err());
    renderer
        .request(Query::Bounds(first), QueryToken(701))
        .unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            token: QueryToken(701),
            applied: 2,
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
}
