use glory_render::memory::Memory;
use glory_render::*;

#[test]
fn typeahead_validates_scalar_clock_physical_origin_and_collection_metadata() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let other = ids.allocate();
    let button = Descriptor::new(Props::Button(ButtonProps::default()));
    for text in ["a", "É", "🦀"] {
        validate_typeahead(text, MAX_TYPEAHEAD_MILLIS).unwrap();
    }
    for text in ["", "ab", " ", "\n", "\0", "\r"] {
        assert!(validate_typeahead(text, 1).is_err());
    }
    assert!(validate_typeahead("a", MAX_TYPEAHEAD_MILLIS + 1).is_err());
    let event = Event::Typeahead {
        from: id,
        text: "a".into(),
        at_millis: 10,
    };
    assert_eq!(event.kind(), EventKind::Typeahead);
    event.validate_payload(id, &button).unwrap();
    assert!(event.validate_payload(other, &button).is_err());
    assert!(
        event
            .validate_payload(id, &Descriptor::new(Props::Group))
            .is_err()
    );
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        Mutation::Create {
            id,
            node: Box::new(button),
        },
        Mutation::Place {
            id,
            parent: None,
            before: None,
        },
    ])
    .unwrap();
    assert!(tree.validate_event(id, &event).unwrap());
    tree.apply(&[Mutation::Suspend { id }]).unwrap();
    assert!(tree.push_event(id, event).is_err());
    let mut group = Descriptor::new(Props::View);
    group.semantics = Some(Box::new(Semantics {
        typeahead: true,
        ..Default::default()
    }));
    assert!(group.validate().is_err());
    group.semantics.as_mut().unwrap().typeahead = false;
    group.semantics.as_mut().unwrap().navigation_direction = Direction::Rtl;
    assert!(group.validate().is_err());
    group.semantics.as_mut().unwrap().navigation = Some(Orientation::Horizontal);
    group.validate().unwrap();
}

#[test]
fn rtl_flips_only_unmodified_horizontal_arrows() {
    let typed = KeyboardInput {
        key: "É",
        shift: true,
        ..Default::default()
    };
    assert_eq!(typed.typeahead(1), Some("É"));
    for input in [
        KeyboardInput {
            control: true,
            ..typed
        },
        KeyboardInput { alt: true, ..typed },
        KeyboardInput {
            meta: true,
            ..typed
        },
        KeyboardInput {
            composing: true,
            ..typed
        },
        KeyboardInput {
            editable: true,
            ..typed
        },
        KeyboardInput { key: " ", ..typed },
    ] {
        assert_eq!(input.typeahead(1), None);
    }
    assert_eq!(typed.typeahead(u64::MAX), None);
    for (key, expected) in [
        ("ArrowLeft", Navigation::Next),
        ("ArrowRight", Navigation::Previous),
        ("Home", Navigation::First),
        ("End", Navigation::Last),
    ] {
        let input = KeyboardInput {
            key,
            ..Default::default()
        };
        assert_eq!(
            Orientation::Horizontal.key_with_direction(Direction::Rtl, input),
            Some(expected)
        );
        for input in [
            KeyboardInput {
                control: true,
                ..input
            },
            KeyboardInput {
                shift: true,
                ..input
            },
            KeyboardInput {
                composing: true,
                ..input
            },
            KeyboardInput {
                editable: true,
                ..input
            },
        ] {
            assert_eq!(
                Orientation::Horizontal.key_with_direction(Direction::Rtl, input),
                None
            );
        }
    }
    assert_eq!(
        Orientation::Vertical.key_with_direction(
            Direction::Rtl,
            KeyboardInput {
                key: "ArrowDown",
                ..Default::default()
            }
        ),
        Some(Navigation::Next)
    );
}
