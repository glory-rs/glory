use glory_render::memory::Memory;
use glory_render::*;
fn value() -> ProgressValue {
    ProgressValue {
        min: 0.,
        max: 100.,
        current: Some(25.),
        value_text: None,
    }
}
fn descriptor() -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.role = Some(Role::ProgressBar);
    node.name = Some("Download".into());
    node.semantics = Some(Box::new(Semantics {
        progress: Some(value()),
        ..Default::default()
    }));
    node
}
#[test]
fn finite_progress_bounds_current_indeterminate_and_text_validate_independently() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            ProgressValue {
                min: bad,
                ..value()
            }
            .validate()
            .is_err()
        );
        assert!(
            ProgressValue {
                max: bad,
                ..value()
            }
            .validate()
            .is_err()
        );
        assert!(
            ProgressValue {
                current: Some(bad),
                ..value()
            }
            .validate()
            .is_err()
        );
    }
    for current in [-1., 101.] {
        assert!(
            ProgressValue {
                current: Some(current),
                ..value()
            }
            .validate()
            .is_err()
        );
    }
    assert!(
        ProgressValue {
            min: 100.,
            ..value()
        }
        .validate()
        .is_err()
    );
    assert!(ProgressValue { max: 0., ..value() }.validate().is_err());
    assert_eq!(
        ProgressValue {
            current: None,
            ..value()
        }
        .fraction()
        .unwrap(),
        None
    );
    assert_eq!(
        ProgressValue {
            min: -f64::MAX,
            max: f64::MAX,
            current: Some(0.),
            value_text: None
        }
        .fraction()
        .unwrap(),
        Some(0.5)
    );
    for text in ["".to_string(), " ".to_string(), "x".repeat(1025)] {
        assert!(
            ProgressValue {
                value_text: Some(text),
                ..value()
            }
            .validate()
            .is_err()
        );
    }
    for current in [0., 100.] {
        ProgressValue {
            current: Some(current),
            ..value()
        }
        .validate()
        .unwrap();
    }
}
#[test]
fn progress_requires_its_real_role_rejects_interaction_and_prepared_errors_roll_back() {
    let mut node = descriptor();
    node.validate().unwrap();
    for event in [
        EventKind::Range,
        EventKind::Activate,
        EventKind::Keyboard,
        EventKind::Drag,
        EventKind::Navigate,
        EventKind::Typeahead,
    ] {
        assert!(event.validate_target(&node).is_err());
    }
    node.focus = FocusMode::TabStop;
    assert!(node.validate().is_err());
    node.focus = FocusMode::Auto;
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[Mutation::Create {
        id,
        node: Box::new(node.clone()),
    }])
    .unwrap();
    let stats = tree.stats();
    let patch = Patch::Semantics(Change::Set(Some(Box::new(Semantics {
        progress: Some(ProgressValue {
            current: Some(101.),
            ..value()
        }),
        ..Default::default()
    }))));
    assert!(tree.prepare_patch(id, &patch).is_err());
    assert_eq!(tree.node(id).unwrap().descriptor, node);
    assert_eq!(tree.stats(), stats);
    node.role = Some(Role::Group);
    assert!(node.validate().is_err());
    node.role = Some(Role::ProgressBar);
    node.semantics = None;
    assert!(node.validate().is_err());
}
#[test]
fn progress_context_rejects_live_moves_resume_and_planned_interactive_descendants() {
    assert!(
        Plan::new(vec![
            PlanNode {
                parent: None,
                descriptor: descriptor()
            },
            PlanNode {
                parent: Some(0),
                descriptor: Descriptor::new(Props::Group)
            },
            PlanNode {
                parent: Some(1),
                descriptor: Descriptor::new(Props::Button(Default::default()))
            }
        ])
        .is_err()
    );
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let root = ids.allocate();
    let group = ids.allocate();
    let button = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        Mutation::Create {
            id: root,
            node: Box::new(descriptor()),
        },
        Mutation::Place {
            id: root,
            parent: None,
            before: None,
        },
        Mutation::Create {
            id: group,
            node: Box::new(Descriptor::new(Props::Group)),
        },
        Mutation::Place {
            id: group,
            parent: None,
            before: None,
        },
        Mutation::Create {
            id: button,
            node: Box::new(Descriptor::new(Props::Button(Default::default()))),
        },
        Mutation::Place {
            id: button,
            parent: Some(group),
            before: None,
        },
    ])
    .unwrap();
    let stats = tree.stats();
    assert!(
        tree.apply(&[Mutation::Place {
            id: group,
            parent: Some(root),
            before: None
        }])
        .is_err()
    );
    assert_eq!(tree.stats(), stats);
    tree.apply(&[Mutation::Suspend { id: group }]).unwrap();
    let stats = tree.stats();
    assert!(
        tree.apply(&[Mutation::Resume {
            id: group,
            parent: Some(root),
            before: None
        }])
        .is_err()
    );
    assert_eq!(tree.stats(), stats);
}
