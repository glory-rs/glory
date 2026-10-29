#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_render::*;
#[test]
fn native_new_context_is_rejected_before_app_creation_and_preserves_direct_prefix_queries() {
    let app = glory_ui::App::new(Renderer::new);
    assert!(
        app.mount(
            glory_ui::link()
                .target("/guide")
                .semantics(Some(Box::new(Semantics {
                    link_open: Some(LinkOpen::NewContext),
                    ..Default::default()
                })))
        )
        .is_err()
    );
    assert!(app.tree().is_empty());
    let runtime = RuntimeId::new();
    let mut renderer = Renderer::new(runtime);
    let mut ids = NodeIds::new(runtime);
    let link = ids.allocate();
    let later = ids.allocate();
    let descriptor = Descriptor::new(Props::Link(LinkProps {
        target: "/guide".into(),
        label: "Guide".into(),
    }));
    let patch = Patch::Semantics(Change::Set(Some(Box::new(Semantics {
        link_open: Some(LinkOpen::NewContext),
        ..Default::default()
    }))));
    assert_eq!(
        renderer.preflight((descriptor).view(), Some(&patch)),
        Err(Error::Unsupported("native separate link contexts"))
    );
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id: link,
                node: Box::new(descriptor)
            },
            Mutation::Place {
                id: link,
                parent: None,
                before: None
            },
            Mutation::Patch { id: link, patch },
            Mutation::Create {
                id: later,
                node: Box::new(Descriptor::new(Props::View))
            }
        ]),
        Err(Error::Unsupported("native separate link contexts"))
    );
    assert_eq!(renderer.stats().applied, 2);
    assert_eq!(renderer.stats().nodes, 1);
    renderer
        .request(Query::Bounds(link), QueryToken(1))
        .unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(matches!(
        inputs.as_slice(),
        [Input::Response {
            applied: 2,
            result: Ok(QueryValue::Bounds(_)),
            ..
        }]
    ));
}
