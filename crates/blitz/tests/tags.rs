#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;
#[test]
fn native_tags_policy_admits_plain_and_refined_inputs_and_preserves_later_rejection_prefix() {
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
    renderer.preflight(tags.view(), None).unwrap();
    renderer.preflight(plain.view(), Some(&patch)).unwrap();
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
            Mutation::Patch { id: input, patch },
            Mutation::Create {
                id: rejected,
                node: Box::new(Descriptor::new(Props::File(Default::default())))
            }
        ]),
        Err(Error::Unsupported("native file picker controls"))
    );
    renderer.listen(input, EventKind::Tags, true).unwrap();
    assert_eq!(renderer.stats().nodes, 1);
    assert_eq!(renderer.stats().applied, 3);
    assert!(
        renderer
            .engine_id(rejected, glory_html::Part::Main)
            .is_err()
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
            applied: 3,
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
}
