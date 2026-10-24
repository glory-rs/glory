use glory_render::{memory::Memory, *};

fn capture(revision: u64, axis: Orientation) -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.semantics = Some(Box::new(Semantics {
        swipe_capture: Some(SwipeCapture { axis, revision }),
        ..Default::default()
    }));
    node
}
fn patch(revision: u64, axis: Orientation) -> Patch {
    Patch::Semantics(Change::Set(capture(revision, axis).semantics))
}
fn setup() -> (Memory, NodeId) {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        Mutation::Create {
            id,
            node: Box::new(capture(2, Orientation::Horizontal)),
        },
        Mutation::Place {
            id,
            parent: None,
            before: None,
        },
    ])
    .unwrap();
    (tree, id)
}
fn event(id: NodeId, revision: u64, phase: DragPhase) -> Event {
    Event::Swipe {
        from: id,
        phase,
        x: 1.,
        y: 2.,
        revision,
    }
}
#[test]
fn finite_independent_view_declaration_and_plan_context() {
    let node = capture(0, Orientation::Horizontal);
    node.validate().unwrap();
    EventKind::Swipe.validate_target(&node).unwrap();
    for role in [
        Role::Tooltip,
        Role::ProgressBar,
        Role::Separator,
        Role::Grid,
        Role::Tree,
    ] {
        let mut invalid = node.clone();
        invalid.role = Some(role);
        assert!(invalid.validate().is_err());
    }
    for props in [
        Props::Group,
        Props::Button(ButtonProps::default()),
        Props::Input(InputProps::default()),
    ] {
        let mut invalid = node.clone();
        invalid.props = props;
        assert!(invalid.validate().is_err());
    }
    let mut invalid = node.clone();
    invalid.semantics.as_mut().unwrap().drag_capture = Some(DragCapture::Press);
    assert!(invalid.validate().is_err());
    assert!(
        Plan::new(vec![
            PlanNode {
                parent: None,
                descriptor: Descriptor::new(Props::Button(ButtonProps::default()))
            },
            PlanNode {
                parent: Some(0),
                descriptor: node
            }
        ])
        .is_err()
    );
    assert!(
        EventKind::Swipe
            .validate_target(&Descriptor::new(Props::View))
            .is_err()
    );
}
#[test]
fn version_guard_rolls_back_and_clear_keeps_only_numeric_watermark() {
    let (mut tree, id) = setup();
    let stats = tree.stats();
    let receipt = tree.receipt();
    assert!(
        tree.prepare_patch(id, &patch(1, Orientation::Horizontal))
            .is_err()
    );
    assert!(
        tree.prepare_patch(id, &patch(2, Orientation::Vertical))
            .is_err()
    );
    drop(
        tree.prepare_patch(id, &patch(5, Orientation::Vertical))
            .unwrap(),
    );
    assert_eq!(tree.stats(), stats);
    assert_eq!(tree.receipt(), receipt);
    assert!(
        tree.validate_event(id, &event(id, 2, DragPhase::Start))
            .unwrap()
    );
    tree.apply(&[Mutation::Patch {
        id,
        patch: Patch::Semantics(Change::Clear),
    }])
    .unwrap();
    assert!(
        !tree
            .validate_event(id, &event(id, 2, DragPhase::Drop))
            .unwrap()
    );
    assert!(
        tree.prepare_patch(id, &patch(2, Orientation::Horizontal))
            .is_err()
    );
    tree.apply(&[Mutation::Patch {
        id,
        patch: patch(3, Orientation::Vertical),
    }])
    .unwrap();
    assert!(
        !tree
            .validate_event(id, &event(id, 2, DragPhase::Drop))
            .unwrap()
    );
    assert!(
        tree.validate_event(id, &event(id, 3, DragPhase::Drop))
            .unwrap()
    );
    assert!(matches!(
        tree.validate_event(id, &event(id, 4, DragPhase::Drop)),
        Err(Error::InputRevision { .. })
    ));
    // A rejected future prepare cannot advance the accepted watermark.
    tree.apply(&[Mutation::Patch {
        id,
        patch: Patch::Semantics(Change::Clear),
    }])
    .unwrap();
    tree.apply(&[Mutation::Patch {
        id,
        patch: patch(4, Orientation::Horizontal),
    }])
    .unwrap();
}
#[test]
fn exact_origin_finite_coordinates_and_terminal_cleanup() {
    let (mut tree, id) = setup();
    let mut others = NodeIds::new(id.runtime());
    others.allocate();
    let other = others.allocate();
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    for origin in [other, foreign] {
        assert!(
            tree.validate_event(
                id,
                &Event::Swipe {
                    from: origin,
                    phase: DragPhase::Start,
                    x: 0.,
                    y: 0.,
                    revision: 2
                }
            )
            .is_err()
        );
    }
    for value in [f64::NAN, f64::INFINITY] {
        assert!(
            tree.validate_event(
                id,
                &Event::Swipe {
                    from: id,
                    phase: DragPhase::Move,
                    x: value,
                    y: 0.,
                    revision: 2
                }
            )
            .is_err()
        );
    }
    let mut metadata = capture(2, Orientation::Horizontal).semantics.unwrap();
    metadata.inert = true;
    tree.apply(&[Mutation::Patch {
        id,
        patch: Patch::Semantics(Change::Set(Some(metadata))),
    }])
    .unwrap();
    assert!(
        !tree
            .validate_event(id, &event(id, 2, DragPhase::Drop))
            .unwrap()
    );
    assert!(
        tree.validate_event(id, &event(id, 2, DragPhase::Cancel))
            .unwrap()
    );
    tree.apply(&[Mutation::Suspend { id }]).unwrap();
    assert!(
        tree.validate_event(id, &event(id, 2, DragPhase::Cancel))
            .unwrap()
    );
    tree.apply(&[Mutation::Dispose { id }]).unwrap();
    assert!(
        tree.validate_event(id, &event(id, 2, DragPhase::Cancel))
            .is_err()
    );
}
#[test]
fn queued_replaced_revisions_and_retired_membership_do_not_leak() {
    let (mut tree, id) = setup();
    tree.push_event(id, event(id, 2, DragPhase::Drop)).unwrap();
    tree.apply(&[Mutation::Patch {
        id,
        patch: patch(3, Orientation::Horizontal),
    }])
    .unwrap();
    let mut inputs = vec![];
    tree.drain(&mut inputs);
    assert!(inputs.is_empty());
    tree.push_event(id, event(id, 3, DragPhase::Move)).unwrap();
    tree.apply(&[Mutation::Dispose { id }]).unwrap();
    tree.drain(&mut inputs);
    assert!(inputs.is_empty());
}
