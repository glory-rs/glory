#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::*;
#[test]
fn native_menubar_role_and_intrinsic_root_refinement_admit_with_later_rejection_prefix_watermarks()
{
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let accepted = ids.allocate();
    let rejected = ids.allocate();
    let plain = Descriptor::new(Props::View);
    let mut bar = plain.clone();
    bar.role = Some(Role::Menubar);
    bar.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("bar").unwrap()),
        navigation: Some(Orientation::Horizontal),
        ..Default::default()
    }));
    let mut renderer = Renderer::new(runtime);
    renderer.preflight(bar.view(), None).unwrap();
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id: accepted,
                node: Box::new(bar)
            },
            Mutation::Place {
                id: accepted,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: rejected,
                node: Box::new(Descriptor::new(Props::File(Default::default())))
            },
        ]),
        Err(Error::Unsupported("native file picker controls"))
    );
    assert_eq!(renderer.stats().applied, 2);
    assert_eq!(renderer.stats().nodes, 1);
    let mut trigger = Descriptor::new(Props::Button(ButtonProps::default()));
    trigger.focus = FocusMode::TabStop;
    trigger.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("trigger").unwrap()),
        menubar_item: true,
        navigation_item: true,
        menu_item: Some(MenuItemSemantics {
            kind: MenuItemKind::Action,
            owner: SemanticId::new("bar").unwrap(),
        }),
        popup: Some(PopupKind::Menu),
        controls: Some(SemanticId::new("panel").unwrap()),
        expanded: Some(false),
        ..Default::default()
    }));
    renderer.preflight(trigger.view(), None).unwrap();
    renderer
        .request(Query::Bounds(accepted), QueryToken(6101))
        .unwrap();
    let mut input = Vec::new();
    renderer.drain(&mut input);
    assert!(matches!(
        input.as_slice(),
        [Input::Response {
            token: QueryToken(6101),
            applied: 2,
            result: Ok(QueryValue::Bounds(_))
        }]
    ));
}
