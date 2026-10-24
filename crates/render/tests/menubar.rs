use glory_render::*;
fn root() -> Descriptor {
    let mut node = Descriptor::new(Props::Button(ButtonProps {
        label: "File".into(),
        ..Default::default()
    }));
    node.focus = FocusMode::Programmatic;
    node.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("file-root").unwrap()),
        menubar_item: true,
        navigation_item: true,
        menu_item: Some(MenuItemSemantics {
            kind: MenuItemKind::Action,
            owner: SemanticId::new("application-bar").unwrap(),
        }),
        popup: Some(PopupKind::Menu),
        controls: Some(SemanticId::new("file-menu").unwrap()),
        expanded: Some(false),
        ..Default::default()
    }));
    node
}

#[test]
fn menubar_live_metadata_cannot_be_cleared_without_changing_role_and_failed_patch_rolls_back() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut bar = Descriptor::new(Props::View);
    bar.role = Some(Role::Menubar);
    bar.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("bar").unwrap()),
        navigation: Some(Orientation::Horizontal),
        ..Default::default()
    }));
    let mut memory = memory::Memory::new(runtime);
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(bar.clone()),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let before = memory.stats();
    assert!(
        memory
            .prepare_patch(id, &Patch::Semantics(Change::Clear))
            .is_err()
    );
    assert_eq!(memory.node(id).unwrap().descriptor.view(), bar.view());
    assert_eq!(memory.stats(), before);
}
#[test]
fn menubar_refinement_preserves_intrinsic_controls_and_late_focus_matches_the_current_entry() {
    let mut node = root();
    node.validate().unwrap();
    let id = NodeIds::new(RuntimeId::new()).allocate();
    assert!(matches!(
        Action::Focus(id).validate_target(&node),
        Err(Error::Unsupported(_))
    ));
    node.focus = FocusMode::TabStop;
    Action::Focus(id).validate_target(&node).unwrap();
    node.props = Props::View;
    assert!(node.validate().is_err());
    node = root();
    node.focus = FocusMode::Auto;
    assert!(node.validate().is_err());
    node = root();
    node.semantics.as_mut().unwrap().menu_item = None;
    assert!(node.validate().is_err());
    let mut bar = Descriptor::new(Props::View);
    bar.role = Some(Role::Menubar);
    bar.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("application-bar").unwrap()),
        navigation: Some(Orientation::Horizontal),
        ..Default::default()
    }));
    bar.validate().unwrap();
    let mut missing = bar.clone();
    missing.semantics = None;
    assert!(missing.validate().is_err());
    bar.semantics.as_mut().unwrap().navigation = Some(Orientation::Vertical);
    assert!(bar.validate().is_err());
}
