use glory_render::{memory::Memory, *};
fn add(memory: &mut Memory, ids: &mut NodeIds, props: Props, parent: Option<NodeId>) -> NodeId {
    let id = ids.allocate();
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(Descriptor::new(props)),
            },
            Mutation::Place {
                id,
                parent,
                before: None,
            },
        ])
        .unwrap();
    id
}
fn inert(value: bool) -> Patch {
    Patch::Semantics(Change::Set(Some(Box::new(Semantics {
        inert: value,
        ..Default::default()
    }))))
}
fn drain(memory: &mut Memory) -> Vec<Input> {
    let mut values = vec![];
    memory.drain(&mut values);
    values
}
#[test]
fn inert_rejects_non_views_in_raw_prepared_and_direct_paths_without_advancing_state() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let button = add(
        &mut memory,
        &mut ids,
        Props::Button(Default::default()),
        None,
    );
    let initial = memory.node(button).unwrap().descriptor.to_owned();
    let stats = memory.stats();
    let mut descriptor = Descriptor::new(Props::Button(Default::default()));
    assert!(descriptor.patch(inert(true)).is_err());
    assert!(memory.prepare_patch(button, &inert(true)).is_err());
    assert!(
        memory
            .apply(&[Mutation::Patch {
                id: button,
                patch: inert(true)
            }])
            .is_err()
    );
    assert_eq!(memory.node(button).unwrap().descriptor, initial);
    assert_eq!(memory.stats(), stats);
}
#[test]
fn inherited_inert_blocks_old_and_new_user_events_but_keeps_measurements_terminal_loss_and_identity()
 {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let root = add(&mut memory, &mut ids, Props::View, None);
    let group = add(&mut memory, &mut ids, Props::Group, Some(root));
    let button = add(
        &mut memory,
        &mut ids,
        Props::Button(Default::default()),
        Some(group),
    );
    let input = add(
        &mut memory,
        &mut ids,
        Props::Input(Default::default()),
        Some(group),
    );
    memory.push_event(button, Event::Activate).unwrap();
    memory
        .push_event(
            input,
            Event::ValueChanged {
                value: "queued".into(),
                composing: false,
                revision: 0,
            },
        )
        .unwrap();
    memory
        .apply(&[Mutation::Patch {
            id: root,
            patch: inert(true),
        }])
        .unwrap();
    assert!(memory.active(input) && memory.inert(input) && !memory.enabled(input));
    assert!(drain(&mut memory).is_empty());
    for (target, event) in [
        (button, Event::Activate),
        (button, Event::FocusChanged(true)),
        (
            input,
            Event::ValueChanged {
                value: "blocked".into(),
                composing: false,
                revision: 0,
            },
        ),
        (
            button,
            Event::Navigate {
                from: button,
                command: Navigation::Next,
            },
        ),
        (
            root,
            Event::PointerWithin {
                from: button,
                related: None,
                phase: PointerPhase::Enter,
            },
        ),
    ] {
        assert!(!memory.validate_event(target, &event).unwrap(), "{event:?}");
        memory.push_event(target, event).unwrap();
    }
    assert!(drain(&mut memory).is_empty());
    for (target, event) in [
        (input, Event::FocusChanged(false)),
        (input, Event::Composition { active: false }),
        (
            root,
            Event::PointerWithin {
                from: button,
                related: None,
                phase: PointerPhase::Leave,
            },
        ),
        (
            root,
            Event::Resize {
                size: Rect::default(),
                viewport: Rect::default(),
            },
        ),
        (
            root,
            Event::Scroll {
                x: 0.,
                y: 0.,
                viewport: Rect::default(),
                ack: None,
            },
        ),
    ] {
        assert!(memory.validate_event(target, &event).unwrap());
        memory.push_event(target, event).unwrap();
    }
    assert_eq!(drain(&mut memory).len(), 5);
    assert!(
        Query::Bounds(input)
            .validate_target(memory.node(input).unwrap().descriptor.view())
            .is_ok()
    );
    memory
        .apply(&[Mutation::Patch {
            id: root,
            patch: inert(false),
        }])
        .unwrap();
    assert!(!memory.inert(input) && memory.enabled(input));
    assert!(memory.validate_event(button, &Event::Activate).unwrap());
}
