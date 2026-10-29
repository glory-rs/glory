#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_render::*;
#[test]
fn native_graphic_direct_apply_and_preflight_keep_the_same_accepted_prefix() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let first = ids.allocate();
    let second = ids.allocate();
    let mut host = Renderer::new(runtime);
    let node = Descriptor::new(Props::Graphic(GraphicProps::default()));
    #[cfg(feature = "paint")]
    let node = {
        let mut node = node;
        node.style.padding = Some(style::Edges::all(1.0));
        node
    };
    assert!(host.preflight(node.view(), None).is_err());
    assert!(
        host.apply(&[
            Mutation::Create {
                id: first,
                node: Box::new(Descriptor::default())
            },
            Mutation::Place {
                id: first,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: second,
                node: Box::new(node)
            }
        ])
        .is_err()
    );
    assert_eq!(host.stats().nodes, 1);
    assert_eq!(host.stats().applied, 2);
    assert!(host.engine_id(first, glory_html::Part::Main).is_ok());
    assert!(host.engine_id(second, glory_html::Part::Main).is_err());
    host.request(Query::Bounds(first), QueryToken(7701))
        .unwrap();
    let mut inputs = Vec::new();
    host.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            token: QueryToken(7701),
            applied: 2,
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
}
