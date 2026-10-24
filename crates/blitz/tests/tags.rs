#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;
#[test]
fn native_tags_policy_explicitly_rejects_without_rejecting_plain_inputs_or_losing_prefix() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let input = ids.allocate();
    let rejected = ids.allocate();
    let plain = Descriptor::new(Props::Input(InputProps::default()));
    let mut tags = plain.clone();
    tags.semantics = Some(Box::new(Semantics {
        tags_input: Some(TagSeparators::default()),
        ..Default::default()
    }));
    let patch = Patch::Semantics(Change::Set(tags.semantics.clone()));
    let mut renderer = Renderer::new(runtime);
    renderer.preflight(plain.view(), None).unwrap();
    assert_eq!(
        renderer.preflight(tags.view(), None),
        Err(Error::Unsupported("native tags input policy"))
    );
    assert_eq!(
        renderer.preflight(plain.view(), Some(&patch)),
        Err(Error::Unsupported("native tags input policy"))
    );
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id: input,
                node: Box::new(plain)
            },
            Mutation::Place {
                id: input,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: rejected,
                node: Box::new(tags)
            }
        ]),
        Err(Error::Unsupported("native tags input policy"))
    );
    assert_eq!(renderer.stats().nodes, 1);
    assert_eq!(renderer.stats().applied, 2);
    assert!(
        renderer
            .engine_id(rejected, glory_html::Part::Main)
            .is_err()
    );
    assert_eq!(
        renderer.apply(&[Mutation::Patch { id: input, patch }]),
        Err(Error::Unsupported("native tags input policy"))
    );
    renderer
        .request(Query::Bounds(input), QueryToken(5051))
        .unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            token: QueryToken(5051),
            applied: 2,
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
}
