use glory_render::{memory::Memory, *};
fn region(name: &str, policy: ContextEditPolicy) -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.role = Some(Role::Group);
    node.focus = FocusMode::TabStop;
    node.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new(name).unwrap()),
        context_menu: Some(policy),
        ..Default::default()
    }));
    node
}
fn event(from: NodeId) -> Event {
    Event::ContextMenu {
        from,
        point: ContextPoint { x: 19.5, y: 23.0 },
        source: ContextSource::Pointer,
    }
}
fn mount(memory: &mut Memory, id: NodeId, node: Descriptor, parent: Option<NodeId>) {
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(node),
            },
            Mutation::Place {
                id,
                parent,
                before: None,
            },
        ])
        .unwrap();
}
#[test]
fn context_policy_and_viewport_geometry_are_finite_intrinsic_metadata() {
    let mut node = region("area", ContextEditPolicy::Native);
    node.validate().unwrap();
    let id = NodeIds::new(RuntimeId::new()).allocate();
    event(id).validate_payload(id, &node).unwrap();
    node.props = Props::Button(ButtonProps::default());
    assert!(node.validate().is_err());
    node = region("area", ContextEditPolicy::Override);
    node.semantics.as_mut().unwrap().popup = Some(PopupKind::Menu);
    assert!(node.validate().is_err());
    for key in ["ContextMenu", "F10"] {
        assert!(
            KeyboardInput {
                key,
                shift: key == "F10",
                editable: true,
                ..Default::default()
            }
            .context_menu()
        );
    }
    assert!(
        !KeyboardInput {
            key: "F10",
            ..Default::default()
        }
        .context_menu()
    );
    assert!(
        !KeyboardInput {
            key: "ContextMenu",
            composing: true,
            ..Default::default()
        }
        .context_menu()
    );
    assert!(
        ContextPoint {
            x: f64::NAN,
            y: 0.0
        }
        .validate()
        .is_err()
    );
    let mut menu = Descriptor::new(Props::View);
    menu.role = Some(Role::Menu);
    menu.focus = FocusMode::Programmatic;
    menu.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("menu").unwrap()),
        navigation: Some(Orientation::Vertical),
        layer: Some(Layer {
            modal: false,
            placement: LayerPlacement::ContextPoint(ContextPoint { x: -3.0, y: 12.5 }),
            anchor: Some(SemanticId::new("area").unwrap()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    menu.validate().unwrap();
    Action::ExitMenu {
        node: id,
        backwards: false,
    }
    .validate_target(&menu)
    .unwrap();
    let mut ordinary = menu.clone();
    ordinary
        .semantics
        .as_mut()
        .unwrap()
        .layer
        .as_mut()
        .unwrap()
        .placement = LayerPlacement::Anchor(glory_render::AnchorPlacement::new(
        glory_render::AnchorSide::Bottom,
        glory_render::AnchorAlign::Start,
    ));
    assert!(
        Action::ExitMenu {
            node: id,
            backwards: false
        }
        .validate_target(&ordinary)
        .is_err()
    );
    menu.semantics
        .as_mut()
        .unwrap()
        .layer
        .as_mut()
        .unwrap()
        .placement = LayerPlacement::ContextPoint(ContextPoint {
        x: f64::INFINITY,
        y: 0.0,
    });
    assert!(menu.validate().is_err());
}
#[test]
fn context_ancestry_nearest_boundary_editors_and_retired_origins_share_one_validation() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let outer = ids.allocate();
    let inner = ids.allocate();
    let child = ids.allocate();
    let editor = ids.allocate();
    let group = ids.allocate();
    let outsider = ids.allocate();
    let mut tree = Memory::new(runtime);
    mount(
        &mut tree,
        outer,
        region("outer", ContextEditPolicy::Native),
        None,
    );
    mount(
        &mut tree,
        inner,
        region("inner", ContextEditPolicy::Override),
        Some(outer),
    );
    mount(
        &mut tree,
        child,
        Descriptor::new(Props::Button(ButtonProps::default())),
        Some(inner),
    );
    mount(
        &mut tree,
        editor,
        Descriptor::new(Props::Input(InputProps::default())),
        Some(outer),
    );
    mount(&mut tree, group, Descriptor::new(Props::Group), Some(inner));
    mount(&mut tree, outsider, Descriptor::new(Props::View), None);
    assert_eq!(tree.validate_event(inner, &event(child)), Ok(true));
    assert!(tree.validate_event(outer, &event(child)).is_err());
    assert_eq!(tree.validate_event(outer, &event(editor)), Ok(false));
    assert!(tree.validate_event(inner, &event(group)).is_err());
    assert!(tree.validate_event(inner, &event(outsider)).is_err());
    tree.apply(&[Mutation::Suspend { id: child }]).unwrap();
    assert_eq!(tree.validate_event(inner, &event(child)), Ok(false));
    tree.apply(&[Mutation::Dispose { id: child }]).unwrap();
    assert_eq!(tree.validate_event(inner, &event(child)), Ok(false));
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    assert_eq!(
        tree.validate_event(inner, &event(foreign)),
        Err(Error::ForeignRuntime)
    );
    let previous = tree.stats();
    let mut invalid = region("inner", ContextEditPolicy::Native)
        .semantics
        .unwrap();
    invalid.layer = Some(Layer::default());
    assert!(
        tree.apply(&[Mutation::Patch {
            id: inner,
            patch: Patch::Semantics(Change::Set(Some(invalid)))
        }])
        .is_err()
    );
    assert_eq!(tree.stats(), previous);
    let mut disabled = region("inner", ContextEditPolicy::Override)
        .semantics
        .unwrap();
    disabled.disabled = true;
    tree.apply(&[Mutation::Patch {
        id: inner,
        patch: Patch::Semantics(Change::Set(Some(disabled))),
    }])
    .unwrap();
    assert!(!tree.enabled(inner));
    assert!(!tree.enabled(group));
    assert!(tree.push_event(inner, event(inner)).is_err());
}
