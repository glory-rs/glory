use glory_render::memory::Memory;
use glory_render::*;

fn id(value: &str) -> SemanticId {
    SemanticId::new(value).unwrap()
}
fn metadata(value: Semantics) -> Patch {
    Patch::Semantics(Change::Set(Some(Box::new(value))))
}
fn descriptor(name: &str) -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.semantics = Some(Box::new(Semantics {
        id: Some(id(name)),
        ..Default::default()
    }));
    d
}
fn create(tree: &mut Memory, node: NodeId, d: Descriptor, parent: Option<NodeId>) {
    tree.apply(&[
        Mutation::Create {
            id: node,
            node: Box::new(d),
        },
        Mutation::Place {
            id: node,
            parent,
            before: None,
        },
    ])
    .unwrap();
}

#[test]
fn semantic_names_are_bounded_live_and_transactional() {
    assert!(SemanticId::new("").is_err());
    assert!(SemanticId::new("bad key").is_err());
    assert!(SemanticId::new("x".repeat(257)).is_err());
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let a = ids.allocate();
    let b = ids.allocate();
    let mut tree = Memory::new(runtime);
    create(&mut tree, a, descriptor("first"), None);
    create(&mut tree, b, descriptor("second"), Some(a));
    let stats = tree.stats();
    assert!(
        tree.prepare_patch(
            b,
            &metadata(Semantics {
                id: Some(id("first")),
                ..Default::default()
            })
        )
        .is_err()
    );
    assert_eq!(
        tree.node(b).unwrap().descriptor.semantic_id(),
        Some(&id("second"))
    );
    assert_eq!(tree.stats(), stats);
    {
        let staged = tree
            .prepare_patch(
                b,
                &metadata(Semantics {
                    id: Some(id("third")),
                    ..Default::default()
                }),
            )
            .unwrap();
        assert_eq!(staged.descriptor().semantic_id(), Some(&id("third")));
    }
    assert_eq!(tree.named(&id("second")), Some(b));
    assert_eq!(tree.named(&id("third")), None);
    tree.prepare_patch(
        b,
        &metadata(Semantics {
            id: Some(id("third")),
            ..Default::default()
        }),
    )
    .unwrap()
    .commit();
    assert_eq!(tree.named(&id("second")), None);
    assert_eq!(tree.named(&id("third")), Some(b));
    tree.apply(&[Mutation::Dispose { id: a }]).unwrap();
    assert_eq!(tree.named(&id("first")), None);
    assert_eq!(tree.named(&id("third")), None);
}

#[test]
fn invalid_composite_metadata_is_rejected_independently_and_rolls_back() {
    let cases = [
        (
            Props::View,
            Semantics {
                expanded: Some(true),
                ..Default::default()
            },
        ),
        (
            Props::View,
            Semantics {
                tab: true,
                selected: Some(true),
                ..Default::default()
            },
        ),
        (
            Props::View,
            Semantics {
                navigation_item: true,
                ..Default::default()
            },
        ),
        (
            Props::View,
            Semantics {
                disabled: true,
                ..Default::default()
            },
        ),
        (
            Props::Button(ButtonProps::default()),
            Semantics {
                hidden: true,
                ..Default::default()
            },
        ),
        (
            Props::Button(ButtonProps::default()),
            Semantics {
                navigation: Some(Orientation::Vertical),
                ..Default::default()
            },
        ),
        (
            Props::Button(ButtonProps::default()),
            Semantics {
                selected: Some(false),
                ..Default::default()
            },
        ),
        (
            Props::Button(ButtonProps::default()),
            Semantics {
                tab: true,
                ..Default::default()
            },
        ),
        (
            Props::Button(ButtonProps::default()),
            Semantics {
                tab: true,
                selected: Some(true),
                expanded: Some(true),
                ..Default::default()
            },
        ),
        (Props::Group, Semantics::default()),
        (
            Props::Text(TextProps::default()),
            Semantics {
                labelled_by: Some(id("label")),
                ..Default::default()
            },
        ),
        (
            Props::View,
            Semantics {
                id: Some(id("self")),
                controls: Some(id("self")),
                ..Default::default()
            },
        ),
        (
            Props::View,
            Semantics {
                id: Some(id("self")),
                labelled_by: Some(id("self")),
                ..Default::default()
            },
        ),
    ];
    for (props, metadata_value) in cases {
        let d = Descriptor::new(props);
        assert!(d.patched(metadata(metadata_value.clone())).is_err());
        let runtime = RuntimeId::new();
        let node = NodeIds::new(runtime).allocate();
        let mut tree = Memory::new(runtime);
        create(&mut tree, node, d.clone(), None);
        let stats = tree.stats();
        assert!(tree.prepare_patch(node, &metadata(metadata_value)).is_err());
        assert_eq!(tree.node(node).unwrap().descriptor, d);
        assert_eq!(tree.stats(), stats);
    }
}

#[test]
fn hidden_ancestors_suppress_events_without_retiring_named_nodes() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let root = ids.allocate();
    let button = ids.allocate();
    let mut tree = Memory::new(runtime);
    create(&mut tree, root, descriptor("panel"), None);
    create(
        &mut tree,
        button,
        Descriptor::new(Props::Button(ButtonProps::default())),
        Some(root),
    );
    tree.push_event(button, Event::Activate).unwrap();
    tree.apply(&[Mutation::Patch {
        id: root,
        patch: metadata(Semantics {
            id: Some(id("panel")),
            hidden: true,
            ..Default::default()
        }),
    }])
    .unwrap();
    assert!(!tree.active(button));
    assert_eq!(tree.named(&id("panel")), Some(root));
    let mut events = Vec::new();
    tree.drain(&mut events);
    assert!(events.is_empty());
    assert!(tree.push_event(button, Event::Activate).is_err());
    tree.push_event(button, Event::FocusChanged(false)).unwrap();
    tree.drain(&mut events);
    assert_eq!(events.len(), 1);
}

#[test]
fn nearest_collection_and_axis_mapping_do_not_capture_unregistered_controls() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let root = ids.allocate();
    let trigger = ids.allocate();
    let input = ids.allocate();
    let mut tree = Memory::new(runtime);
    let mut group = descriptor("collection");
    group.semantics.as_mut().unwrap().navigation = Some(Orientation::Horizontal);
    create(&mut tree, root, group, None);
    let mut button = Descriptor::new(Props::Button(ButtonProps::default()));
    button
        .patch(metadata(Semantics {
            navigation_item: true,
            ..Default::default()
        }))
        .unwrap();
    create(&mut tree, trigger, button, Some(root));
    create(
        &mut tree,
        input,
        Descriptor::new(Props::Input(InputProps::default())),
        Some(root),
    );
    assert_eq!(
        tree.navigation(trigger),
        Some((root, Orientation::Horizontal))
    );
    assert_eq!(tree.navigation(input), None);
    for axis in [Orientation::Horizontal, Orientation::Vertical] {
        let forward = if axis == Orientation::Horizontal {
            "ArrowRight"
        } else {
            "ArrowDown"
        };
        assert_eq!(
            axis.key(KeyboardInput {
                key: forward,
                repeat: true,
                ..Default::default()
            }),
            Some(Navigation::Next)
        );
        for input in [
            KeyboardInput {
                key: forward,
                editable: true,
                ..Default::default()
            },
            KeyboardInput {
                key: forward,
                composing: true,
                ..Default::default()
            },
            KeyboardInput {
                key: forward,
                shift: true,
                ..Default::default()
            },
            KeyboardInput {
                key: "Tab",
                ..Default::default()
            },
        ] {
            assert_eq!(axis.key(input), None);
        }
    }
    assert_eq!(
        Orientation::Horizontal.key(KeyboardInput {
            key: "ArrowDown",
            ..Default::default()
        }),
        None
    );
}

#[test]
fn duplicate_name_rejection_preserves_the_successful_prefix_watermark() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let a = ids.allocate();
    let b = ids.allocate();
    let mut tree = Memory::new(runtime);
    assert!(
        tree.apply(&[
            Mutation::Create {
                id: a,
                node: Box::new(descriptor("name"))
            },
            Mutation::Create {
                id: b,
                node: Box::new(descriptor("name"))
            }
        ])
        .is_err()
    );
    assert_eq!(tree.apply(&[]).unwrap().applied, 1);
    tree.request(Query::Bounds(a), QueryToken(1)).unwrap();
    let mut inputs = Vec::new();
    tree.drain(&mut inputs);
    assert!(matches!(&inputs[..], [Input::Response { applied: 1, .. }]));
    assert_eq!(tree.named(&id("name")), Some(a));
}
