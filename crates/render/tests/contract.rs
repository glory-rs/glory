use glory_render::memory::Memory;
use glory_render::*;

fn create(id: NodeId, props: Props) -> Mutation {
    Mutation::Create {
        id,
        node: Box::new(Descriptor::new(props)),
    }
}
fn place(id: NodeId, parent: Option<NodeId>) -> Mutation {
    Mutation::Place {
        id,
        parent,
        before: None,
    }
}

#[test]
fn suspended_subtrees_resume_in_order_and_are_owned_until_disposal() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let root = ids.allocate();
    let a = ids.allocate();
    let b = ids.allocate();
    let leaf = ids.allocate();
    memory
        .apply(&[
            create(root, Props::View),
            place(root, None),
            create(a, Props::Group),
            place(a, Some(root)),
            create(b, Props::Group),
            place(b, Some(root)),
            create(leaf, Props::Text(TextProps::default())),
            place(leaf, Some(a)),
            Mutation::Suspend { id: a },
        ])
        .unwrap();
    assert!(!memory.active(leaf));
    assert_eq!(memory.node(root).unwrap().children, [a, b]);
    memory
        .apply(&[Mutation::Resume {
            id: a,
            parent: Some(root),
            before: None,
        }])
        .unwrap();
    assert_eq!(memory.node(root).unwrap().children, [b, a]);
    assert!(memory.active(leaf));
    memory
        .apply(&[Mutation::Suspend { id: a }, Mutation::Dispose { id: root }])
        .unwrap();
    assert!(memory.is_empty());
    assert_eq!(memory.stats().disposed, 4);
    assert!(memory.node(leaf).is_err());
}

#[test]
fn old_events_and_creates_cannot_target_reused_slots_or_other_apps() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let old = ids.allocate();
    memory
        .apply(&[
            create(old, Props::Button(ButtonProps::default())),
            place(old, None),
        ])
        .unwrap();
    memory.push_event(old, Event::Activate).unwrap();
    memory.apply(&[Mutation::Dispose { id: old }]).unwrap();
    assert!(ids.release(old));
    let new = ids.allocate();
    assert_eq!(new.slot(), old.slot());
    assert_ne!(old, new);
    assert!(!ids.contains(old));
    memory
        .apply(&[create(new, Props::View), place(new, None)])
        .unwrap();
    assert!(memory.push_event(old, Event::Activate).is_err());
    assert!(memory.apply(&[create(old, Props::View)]).is_err());
    let mut inputs = Vec::new();
    memory.drain(&mut inputs);
    assert!(inputs.is_empty());
    let mut foreign = NodeIds::new(RuntimeId::new());
    assert_eq!(
        memory.apply(&[create(foreign.allocate(), Props::View)]),
        Err(Error::ForeignRuntime)
    );
}

#[test]
fn scrolling_style_and_empty_false_clear_are_typed() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let check = ids.allocate();
    let input = ids.allocate();
    memory
        .apply(&[
            create(
                check,
                Props::Check(CheckProps {
                    checked: true,
                    ..CheckProps::default()
                }),
            ),
            create(
                input,
                Props::Input(InputProps {
                    value: "initial".into(),
                    ..InputProps::default()
                }),
            ),
            Mutation::Patch {
                id: check,
                patch: Patch::Checked(Change::Set(false)),
            },
            Mutation::Patch {
                id: input,
                patch: Patch::Value {
                    value: Change::Clear,
                    revision: 12,
                },
            },
        ])
        .unwrap();
    let Props::Check(p) = &memory.node(check).unwrap().descriptor.props else {
        panic!()
    };
    assert!(!p.checked);
    let Props::Input(p) = &memory.node(input).unwrap().descriptor.props else {
        panic!()
    };
    assert_eq!((&p.value, p.revision), (&String::new(), 12));
    memory
        .apply(&[Mutation::Patch {
            id: input,
            patch: Patch::Value {
                value: Change::Keep,
                revision: 99,
            },
        }])
        .unwrap();
    let Props::Input(p) = &memory.node(input).unwrap().descriptor.props else {
        panic!()
    };
    assert_eq!(p.revision, 12);
}

#[test]
fn invalid_structure_and_patches_leave_the_previous_node_intact() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let button = ids.allocate();
    let group = ids.allocate();
    let text = ids.allocate();
    let input = ids.allocate();
    memory
        .apply(&[
            create(button, Props::Button(ButtonProps::default())),
            place(button, None),
            create(group, Props::Group),
            place(group, Some(button)),
            create(text, Props::Text(TextProps::default())),
            place(text, Some(group)),
            create(input, Props::Input(InputProps::default())),
        ])
        .unwrap();
    assert!(memory.apply(&[place(input, Some(group))]).is_err());
    assert!(!memory.node(input).unwrap().placed);
    assert!(
        memory
            .apply(&[Mutation::Patch {
                id: text,
                patch: Patch::TextKind(Change::Set(TextKind::Paragraph))
            }])
            .is_err()
    );
    assert_eq!(
        memory.node(text).unwrap().descriptor.props,
        Props::Text(TextProps::default())
    );
    assert!(
        memory
            .apply(&[Mutation::Patch {
                id: group,
                patch: Patch::Classes(Change::Set(vec!["box".into()]))
            }])
            .is_err()
    );
    assert!(memory.apply(&[place(button, Some(group))]).is_err());
    assert_eq!(memory.roots(), &[button]);
}

#[test]
fn moving_a_group_preserves_its_children_and_rejects_invalid_anchors() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let a = ids.allocate();
    let b = ids.allocate();
    let c = ids.allocate();
    memory
        .apply(&[
            create(a, Props::Group),
            place(a, None),
            create(b, Props::View),
            place(b, None),
            create(c, Props::View),
            place(c, Some(a)),
        ])
        .unwrap();
    memory
        .apply(&[Mutation::Place {
            id: b,
            parent: None,
            before: Some(a),
        }])
        .unwrap();
    assert_eq!(memory.roots(), &[b, a]);
    assert_eq!(memory.node(a).unwrap().children, [c]);
    assert!(
        memory
            .apply(&[Mutation::Place {
                id: a,
                parent: Some(b),
                before: Some(c)
            }])
            .is_err()
    );
    assert_eq!(memory.roots(), &[b, a]);
}

#[test]
fn no_geometry_is_fabricated_by_the_memory_interpreter() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let id = ids.allocate();
    memory.apply(&[create(id, Props::View)]).unwrap();
    memory.request(Query::Bounds(id), QueryToken(7)).unwrap();
    let mut inputs = Vec::new();
    memory.drain(&mut inputs);
    assert!(matches!(
        &inputs[..],
        [Input::Response {
            token: QueryToken(7),
            result: Err(Error::Unsupported(_)),
            ..
        }]
    ));
    assert!(!memory.capabilities().measure);
}

#[test]
fn repeated_window_mounts_do_not_retain_an_unbounded_id_history() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    for _ in 0..20_000 {
        let id = ids.allocate();
        memory
            .apply(&[
                create(id, Props::Text(TextProps::default())),
                place(id, None),
                Mutation::Dispose { id },
            ])
            .unwrap();
        assert!(ids.release(id));
    }
    assert_eq!(ids.capacity(), 1);
    assert_eq!(memory.retained_slots(), 1);
    assert!(ids.is_empty());
    assert!(memory.is_empty());
}

#[test]
fn suspension_discards_queued_events_even_when_resumed_before_drain() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let id = ids.allocate();
    memory
        .apply(&[
            create(id, Props::Button(ButtonProps::default())),
            place(id, None),
        ])
        .unwrap();
    memory.push_event(id, Event::Activate).unwrap();
    memory
        .apply(&[
            Mutation::Suspend { id },
            Mutation::Resume {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let mut inputs = Vec::new();
    memory.drain(&mut inputs);
    assert!(inputs.is_empty());
    memory.push_event(id, Event::Activate).unwrap();
    memory.drain(&mut inputs);
    assert_eq!(
        inputs,
        [Input::Event {
            node: id,
            event: Event::Activate
        }]
    );
}

#[test]
fn queries_observe_the_preceding_commit_not_later_updates() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let id = ids.allocate();
    memory
        .apply(&[create(id, Props::Input(InputProps::default()))])
        .unwrap();
    let first = memory
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::Value {
                value: Change::Set("A".into()),
                revision: 1,
            },
        }])
        .unwrap();
    memory.request(Query::Selection(id), QueryToken(1)).unwrap();
    let second = memory
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::Value {
                value: Change::Set("B".into()),
                revision: 2,
            },
        }])
        .unwrap();
    memory.request(Query::Selection(id), QueryToken(2)).unwrap();
    let mut inputs = Vec::new();
    memory.drain(&mut inputs);
    assert!(
        matches!(&inputs[..], [Input::Response { token: QueryToken(1), applied: a, .. }, Input::Response { token: QueryToken(2), applied: b, .. }] if *a == first.applied && *b == second.applied && a < b)
    );
}

#[test]
fn small_patch_batches_do_not_carry_full_style_or_descriptor_payloads() {
    // Text/checked/value updates dominate scrolling lists. Full style and create
    // payloads are indirect so every small queued patch stays compact.
    assert!(std::mem::size_of::<Patch>() <= 64);
    assert!(std::mem::size_of::<Mutation>() <= 128);
}

#[test]
fn focus_and_collection_position_are_typed_validated_and_resettable() {
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.role = Some(Role::ListItem);
    descriptor.focus = FocusMode::Programmatic;
    descriptor.item = Some(ItemPosition {
        index: 99_999_999,
        count: Some(100_000_000),
    });
    descriptor.validate().unwrap();
    let before = descriptor.clone();
    assert!(
        descriptor
            .patch(Patch::Item(Change::Set(Some(ItemPosition {
                index: 2,
                count: Some(2)
            }))))
            .is_err()
    );
    assert!(descriptor.patch(Patch::Role(Change::Clear)).is_err());
    assert_eq!(descriptor, before);
    descriptor
        .patch(Patch::Item(Change::Set(Some(ItemPosition {
            index: 10,
            count: None,
        }))))
        .unwrap();
    descriptor.patch(Patch::Item(Change::Clear)).unwrap();
    descriptor.patch(Patch::Focus(Change::Clear)).unwrap();
    descriptor.patch(Patch::Role(Change::Clear)).unwrap();
    assert_eq!(descriptor.focus, FocusMode::Auto);
    let mut group = Descriptor::new(Props::Group);
    group.focus = FocusMode::TabStop;
    assert!(group.validate().is_err());
    let mut overflow = Descriptor::new(Props::View);
    overflow.role = Some(Role::ListItem);
    overflow.item = Some(ItemPosition {
        index: usize::MAX,
        count: None,
    });
    assert!(overflow.validate().is_err());
}
