#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_render::*;
#[test]
fn sheet_and_passive_grip_reject_early_and_directly_preserving_prefix_queries() {
    for grip in [false, true] {
        let runtime = RuntimeId::new();
        let mut renderer = Renderer::new(runtime);
        let mut ids = NodeIds::new(runtime);
        let first = ids.allocate();
        let rejected = ids.allocate();
        let mut node = Descriptor::new(Props::View);
        let error = if grip {
            node.semantics = Some(Box::new(Semantics {
                sheet_grip: true,
                ..Default::default()
            }));
            Error::Unsupported("native sheet gestures")
        } else {
            node.role = Some(Role::Dialog);
            node.focus = FocusMode::Programmatic;
            node.semantics = Some(Box::new(Semantics {
                id: Some(SemanticId::new("native-sheet").unwrap()),
                layer: Some(Layer {
                    placement: LayerPlacement::Bottom,
                    sheet: Some(SheetConfig {
                        fractions: vec![0.5, 1.0],
                        current: 0,
                        max_extent: 640.0,
                        settle_millis: 180,
                        dismissible: true,
                        revision: 0,
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }));
            Error::Unsupported("native top layers")
        };
        assert_eq!(renderer.preflight(node.view(), None), Err(error.clone()));
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
                    id: rejected,
                    node: Box::new(node.clone())
                }
            ]),
            Err(error.clone())
        );
        assert_eq!(renderer.stats().applied, 2);
        assert_eq!(renderer.stats().nodes, 1);
        renderer
            .request(Query::Bounds(first), QueryToken(1))
            .unwrap();
        let mut inputs = vec![];
        renderer.drain(&mut inputs);
        assert!(matches!(
            inputs.as_slice(),
            [Input::Response {
                applied: 2,
                result: Ok(_),
                ..
            }]
        ));
        let patch = Patch::Semantics(Change::Set(node.semantics));
        assert_eq!(
            renderer.preflight(Descriptor::new(Props::View).view(), Some(&patch)),
            Err(error.clone())
        );
        assert_eq!(
            renderer.apply(&[Mutation::Patch { id: first, patch }]),
            Err(error)
        );
        assert_eq!(renderer.stats().applied, 2);
    }
}
