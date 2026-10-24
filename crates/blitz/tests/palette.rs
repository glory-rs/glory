#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;
#[test]
fn native_palette_interest_rejects_explicitly_and_preserves_prefix_query_watermark() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut renderer = Renderer::new(runtime);
    renderer
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(Descriptor::new(Props::View)),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    assert_eq!(
        renderer.listen(id, EventKind::Palette, true),
        Err(Error::Unsupported("native palette shortcut input"))
    );
    renderer.listen(id, EventKind::Palette, false).unwrap();
    assert_eq!(renderer.stats().nodes, 1);
    assert_eq!(renderer.stats().applied, 2);
    renderer
        .request(Query::Bounds(id), QueryToken(701))
        .unwrap();
    let mut input = Vec::new();
    renderer.drain(&mut input);
    assert!(matches!(
        input.as_slice(),
        [Input::Response {
            token: QueryToken(701),
            applied: 2,
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
}
