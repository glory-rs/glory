use glory_render::{memory::Memory, *};

fn capture(mode: DragCapture, focus: FocusMode) -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.focus = focus;
    node.semantics = Some(Box::new(Semantics {
        drag_capture: Some(mode),
        ..Default::default()
    }));
    node
}

#[test]
fn capture_is_finite_independent_view_metadata_with_explicit_focus_policy() {
    capture(DragCapture::Press, FocusMode::Auto)
        .validate()
        .unwrap();
    assert!(
        capture(DragCapture::PressFocus, FocusMode::Auto)
            .validate()
            .is_err()
    );
    for focus in [FocusMode::Programmatic, FocusMode::TabStop] {
        capture(DragCapture::PressFocus, focus).validate().unwrap();
    }
    for props in [
        Props::Group,
        Props::Button(ButtonProps::default()),
        Props::Input(InputProps::default()),
    ] {
        let mut node = capture(DragCapture::Press, FocusMode::Auto);
        node.props = props;
        assert!(node.validate().is_err());
    }
    let mut node = capture(DragCapture::Press, FocusMode::Auto);
    node.semantics.as_mut().unwrap().sheet_grip = true;
    assert!(node.validate().is_err());
}

#[test]
fn prepared_focus_and_capture_changes_roll_back_and_disabled_ancestors_block_drags() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let parent = ids.allocate();
    let child = ids.allocate();
    let mut tree = Memory::new(runtime);
    for (id, node, ancestor) in [
        (
            parent,
            capture(DragCapture::PressFocus, FocusMode::TabStop),
            None,
        ),
        (child, Descriptor::new(Props::View), Some(parent)),
    ] {
        tree.apply(&[
            Mutation::Create {
                id,
                node: Box::new(node),
            },
            Mutation::Place {
                id,
                parent: ancestor,
                before: None,
            },
        ])
        .unwrap();
    }
    let before = tree.stats();
    let receipt = tree.receipt();
    assert!(
        tree.prepare_patch(parent, &Patch::Focus(Change::Clear))
            .is_err()
    );
    assert_eq!(
        tree.node(parent).unwrap().descriptor.view().focus,
        FocusMode::TabStop
    );
    let mut metadata = tree
        .node(parent)
        .unwrap()
        .descriptor
        .view()
        .semantics
        .clone()
        .unwrap();
    metadata.drag_capture = Some(DragCapture::Press);
    drop(
        tree.prepare_patch(
            parent,
            &Patch::Semantics(Change::Set(Some(metadata.clone()))),
        )
        .unwrap(),
    );
    assert_eq!(
        tree.node(parent)
            .unwrap()
            .descriptor
            .view()
            .semantics
            .as_ref()
            .unwrap()
            .drag_capture,
        Some(DragCapture::PressFocus)
    );
    assert_eq!(tree.stats(), before);
    assert_eq!(tree.receipt(), receipt);
    metadata.disabled = true;
    tree.apply(&[Mutation::Patch {
        id: parent,
        patch: Patch::Semantics(Change::Set(Some(metadata))),
    }])
    .unwrap();
    assert!(!tree.enabled(parent));
    assert!(!tree.enabled(child));
    assert!(
        tree.push_event(
            parent,
            Event::Drag {
                from: parent,
                phase: DragPhase::Start,
                x: 0.,
                y: 0.
            }
        )
        .is_err()
    );
}
