#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;

#[test]
fn native_placeholder_admission_rejects_before_model_or_engine_with_prefix_retained() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let first = ids.allocate();
    let second = ids.allocate();
    let plain = Descriptor::new(Props::Input(Default::default()));
    let mut hinted = plain.clone();
    hinted.semantics = Some(Box::new(Semantics {
        placeholder: Some("Search".into()),
        ..Default::default()
    }));
    let mut renderer = Renderer::new(runtime);
    let error = Error::Unsupported("native input placeholders");
    assert_eq!(renderer.preflight(hinted.view(), None), Err(error.clone()));
    let patch = Patch::Semantics(Change::Set(hinted.semantics.clone()));
    assert_eq!(
        renderer.preflight(plain.view(), Some(&patch)),
        Err(error.clone())
    );
    assert_eq!(renderer.stats().nodes, 0);
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id: first,
                node: Box::new(plain)
            },
            Mutation::Place {
                id: first,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: second,
                node: Box::new(hinted)
            },
        ]),
        Err(error.clone())
    );
    assert_eq!(renderer.stats().nodes, 1);
    assert_eq!(renderer.stats().applied, 2);
    assert!(renderer.engine_id(second, glory_html::Part::Main).is_err());
    assert_eq!(
        renderer.apply(&[Mutation::Patch { id: first, patch }]),
        Err(error)
    );
    assert_eq!(renderer.stats().applied, 2);
}
