#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;
#[test]
fn rich_native_preflight_direct_plan_and_prefix_refuse_before_engine_allocation() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let good = ids.allocate();
    let editor = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    let descriptor = Descriptor::new(Props::RichEditor(Default::default()));
    let error = Err(Error::Unsupported("native rich editing"));
    assert_eq!(renderer.preflight(descriptor.view(), None), error);
    assert_eq!(
        renderer.preflight(
            descriptor.view(),
            Some(&Patch::RichDocument {
                document: Change::Clear,
                revision: 1
            })
        ),
        error
    );
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id: good,
                node: Box::new(Descriptor::new(Props::View))
            },
            Mutation::Place {
                id: good,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: editor,
                node: Box::new(descriptor.clone())
            }
        ]),
        Err(Error::Unsupported("native rich editing"))
    );
    assert_eq!(renderer.stats().applied, 2);
    assert_eq!(renderer.stats().nodes, 1);
    assert!(renderer.engine_id(editor, glory_html::Part::Main).is_err());
    renderer
        .request(Query::Bounds(good), QueryToken(1))
        .unwrap();
    renderer
        .request(Query::RichSelection(good), QueryToken(2))
        .unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(inputs.iter().any(|input| matches!(
        input,
        Input::Response {
            token: QueryToken(1),
            applied: 2,
            result: Ok(QueryValue::Bounds(_))
        }
    )));
    assert!(inputs.iter().any(|input| matches!(
        input,
        Input::Response {
            token: QueryToken(2),
            applied: 2,
            result: Err(Error::Unsupported(_))
        }
    )));
    assert!(matches!(
        renderer.action(Action::SelectRich {
            node: good,
            revision: 0,
            selection: RichSelection::default()
        }),
        Err(Error::Unsupported(_))
    ));
    let mut renderer = Renderer::new(runtime);
    let plan = Plan::new(vec![
        PlanNode {
            parent: None,
            descriptor: Descriptor::new(Props::View),
        },
        PlanNode {
            parent: Some(0),
            descriptor,
        },
    ])
    .unwrap();
    assert_eq!(
        renderer.apply(&[Mutation::CreatePlan {
            plan,
            ids: vec![good, editor]
        }]),
        Err(Error::Unsupported("native rich editing"))
    );
    assert_eq!(renderer.stats().nodes, 0);
    assert_eq!(renderer.stats().applied, 0);
}
