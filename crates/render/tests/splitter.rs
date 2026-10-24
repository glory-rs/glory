use glory_render::*;
fn descriptor() -> Descriptor {
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.role = Some(Role::Separator);
    descriptor.focus = FocusMode::TabStop;
    descriptor.semantics = Some(Box::new(Semantics {
        splitter: Some(RangeValue {
            min: 10.0,
            max: 90.0,
            value: 50.0,
        }),
        separator_axis: Some(Orientation::Vertical),
        controls: Some(SemanticId::new("pane").unwrap()),
        ..Default::default()
    }));
    descriptor
}
#[test]
fn static_separator_stays_passive_and_only_explicit_range_enables_focus_drag_and_range_events() {
    let mut passive = Descriptor::new(Props::View);
    passive.role = Some(Role::Separator);
    passive.validate().unwrap();
    let mut ids = NodeIds::new(RuntimeId::new());
    let id = ids.allocate();
    assert!(Action::Focus(id).validate_target(&passive).is_err());
    for kind in [
        EventKind::Range,
        EventKind::Drag,
        EventKind::Activate,
        EventKind::Navigate,
        EventKind::Keyboard,
    ] {
        assert!(kind.validate_target(&passive).is_err());
    }
    let active = descriptor();
    active.validate().unwrap();
    Action::Focus(id).validate_target(&active).unwrap();
    EventKind::Range.validate_target(&active).unwrap();
    EventKind::Drag.validate_target(&active).unwrap();
    for kind in [
        EventKind::Activate,
        EventKind::Navigate,
        EventKind::Keyboard,
    ] {
        assert!(kind.validate_target(&active).is_err());
    }
    for change in [0, 1, 2, 3, 4] {
        let mut invalid = active.clone();
        match change {
            0 => invalid.role = None,
            1 => invalid.focus = FocusMode::Auto,
            2 => invalid.semantics.as_mut().unwrap().controls = None,
            3 => invalid.semantics.as_mut().unwrap().separator_axis = None,
            _ => {
                invalid
                    .semantics
                    .as_mut()
                    .unwrap()
                    .splitter
                    .as_mut()
                    .unwrap()
                    .value = f64::NAN
            }
        };
        assert!(invalid.validate().is_err());
    }
}
#[test]
fn range_proposals_validate_exact_identity_enabled_state_and_accepted_bounds() {
    let node = descriptor();
    let mut ids = NodeIds::new(RuntimeId::new());
    let id = ids.allocate();
    let other = ids.allocate();
    for command in [
        RangeCommand::Increase,
        RangeCommand::Decrease,
        RangeCommand::Minimum,
        RangeCommand::Maximum,
        RangeCommand::Set(70.0),
    ] {
        Event::Range { from: id, command }
            .validate_payload(id, &node)
            .unwrap();
    }
    for value in [f64::NAN, f64::INFINITY, 9.0, 91.0] {
        assert!(
            Event::Range {
                from: id,
                command: RangeCommand::Set(value)
            }
            .validate_payload(id, &node)
            .is_err()
        );
    }
    assert!(
        Event::Range {
            from: other,
            command: RangeCommand::Increase
        }
        .validate_payload(id, &node)
        .is_err()
    );
    let mut disabled = node.clone();
    disabled.semantics.as_mut().unwrap().disabled = true;
    disabled.validate().unwrap();
    assert!(
        Event::Range {
            from: id,
            command: RangeCommand::Increase
        }
        .validate_payload(id, &disabled)
        .is_err()
    );
    let mut memory = memory::Memory::new(id.runtime());
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(disabled),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    assert!(!memory.enabled(id));
}
#[test]
fn splitter_keyboard_is_perpendicular_to_line_and_preserves_other_keys() {
    for (line, decrease, increase) in [
        (Orientation::Vertical, "ArrowLeft", "ArrowRight"),
        (Orientation::Horizontal, "ArrowUp", "ArrowDown"),
    ] {
        for (key, expected) in [
            (decrease, RangeCommand::Decrease),
            (increase, RangeCommand::Increase),
            ("Home", RangeCommand::Minimum),
            ("End", RangeCommand::Maximum),
        ] {
            let input = KeyboardInput {
                key,
                ..Default::default()
            };
            assert_eq!(RangeCommand::splitter_key(line, input), Some(expected));
            assert_eq!(
                RangeCommand::splitter_key(
                    line,
                    KeyboardInput {
                        repeat: true,
                        ..input
                    }
                ),
                Some(expected)
            );
            for blocked in [
                KeyboardInput {
                    editable: true,
                    ..input
                },
                KeyboardInput {
                    composing: true,
                    ..input
                },
                KeyboardInput {
                    control: true,
                    ..input
                },
                KeyboardInput {
                    meta: true,
                    ..input
                },
                KeyboardInput { alt: true, ..input },
                KeyboardInput {
                    shift: true,
                    ..input
                },
            ] {
                assert!(RangeCommand::splitter_key(line, blocked).is_none());
            }
        }
        for key in ["Tab", " ", "Enter", "PageDown", "a"] {
            assert!(
                RangeCommand::splitter_key(
                    line,
                    KeyboardInput {
                        key,
                        ..Default::default()
                    }
                )
                .is_none()
            );
        }
    }
}
