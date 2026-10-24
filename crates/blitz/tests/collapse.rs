#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;
#[test]
fn optional_height_motion_rejects_preflight_direct_patch_and_preserves_prefix_queries() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let first = ids.allocate();
    let bad = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    assert!(!renderer.capabilities().collapse_completion);
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.semantics = Some(Box::new(Semantics {
        collapse: Some(CollapseConfig {
            open: false,
            duration_millis: 240,
            revision: 0,
        }),
        hidden: true,
        inert: true,
        ..Default::default()
    }));
    let error = Error::Unsupported("native intrinsic-height collapse motion");
    assert_eq!(
        renderer.preflight(descriptor.view(), None),
        Err(error.clone())
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
                id: bad,
                node: Box::new(descriptor.clone())
            }
        ]),
        Err(error.clone())
    );
    assert_eq!(renderer.stats().nodes, 1);
    assert_eq!(renderer.stats().applied, 2);
    renderer
        .request(Query::Bounds(first), QueryToken(1))
        .unwrap();
    let patch = Patch::Semantics(Change::Set(descriptor.semantics));
    assert_eq!(
        renderer.preflight(Descriptor::new(Props::View).view(), Some(&patch)),
        Err(error.clone())
    );
    assert_eq!(
        renderer.apply(&[Mutation::Patch { id: first, patch }]),
        Err(error)
    );
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    assert!(inputs.iter().any(|input| matches!(
        input,
        Input::Response {
            token: QueryToken(1),
            applied: 2,
            ..
        }
    )));
    assert_eq!(renderer.stats().applied, 2);
}

#[test]
fn native_runs_the_explicit_collapse_rejection_conformance_matrix() {
    let runtime = RuntimeId::new();
    conformance::verify_collapse(Renderer::new(runtime), runtime, false);
}
