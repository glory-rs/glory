use glory_render::{memory::Memory, *};
#[test]
fn palette_uses_exact_non_editor_origin_and_reference_validation_respects_suspension() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let root = ids.allocate();
    let origin = ids.allocate();
    let mut memory = Memory::new(runtime);
    memory
        .apply(&[
            Mutation::Create {
                id: root,
                node: Box::new(Descriptor::new(Props::View)),
            },
            Mutation::Place {
                id: root,
                parent: None,
                before: None,
            },
            Mutation::Create {
                id: origin,
                node: Box::new(Descriptor::new(Props::Button(ButtonProps::default()))),
            },
            Mutation::Place {
                id: origin,
                parent: Some(root),
                before: None,
            },
        ])
        .unwrap();
    let event = Event::Palette { from: origin };
    assert!(memory.validate_event(origin, &event).unwrap());
    assert!(memory.validate_event(root, &event).is_err());
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    assert_eq!(
        memory.validate_event(origin, &Event::Palette { from: foreign }),
        Err(Error::ForeignRuntime)
    );
    assert!(
        EventKind::Palette
            .validate_target(&Descriptor::new(Props::Group))
            .is_err()
    );
    let mut passive = Descriptor::new(Props::View);
    passive.role = Some(Role::Tooltip);
    assert!(EventKind::Palette.validate_target(&passive).is_err());
    assert!(
        EventKind::Palette
            .validate_target(&Descriptor::new(Props::Input(InputProps::default())))
            .is_err()
    );
    memory.apply(&[Mutation::Suspend { id: origin }]).unwrap();
    assert!(!memory.validate_event(origin, &event).unwrap());
    memory
        .apply(&[
            Mutation::Resume {
                id: origin,
                parent: Some(root),
                before: None,
            },
            Mutation::Dispose { id: origin },
        ])
        .unwrap();
    assert!(memory.validate_event(origin, &event).is_err());
    for (key, control, meta, expected) in [
        ("k", true, false, true),
        ("K", false, true, true),
        ("f", true, false, false),
        ("?", false, false, false),
    ] {
        assert_eq!(
            KeyboardInput {
                key,
                control,
                meta,
                ..Default::default()
            }
            .command()
                == Some(KeyboardCommand::Palette),
            expected
        );
    }
}
