use glory_render::*;
fn slider() -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.role = Some(Role::Slider);
    node.focus = FocusMode::TabStop;
    node.semantics = Some(Box::new(Semantics {
        slider: Some(SliderValue {
            range: RangeValue {
                min: -20.,
                max: 80.,
                value: 30.,
            },
            axis: Orientation::Horizontal,
            direction: Direction::Ltr,
            step: 0.5,
            page_step: 5.,
            value_text: Some("Thirty units".into()),
        }),
        ..Default::default()
    }));
    node
}
#[test]
fn sliders_have_independent_finite_ranges_and_real_focus_without_separator_relations() {
    let node = slider();
    node.validate().unwrap();
    let mut no_metadata = node.clone();
    no_metadata.semantics = None;
    assert!(no_metadata.validate().is_err());
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0., -1.] {
        for page in [false, true] {
            let mut node = node.clone();
            let range = node.semantics.as_mut().unwrap().slider.as_mut().unwrap();
            if page {
                range.page_step = bad
            } else {
                range.step = bad
            };
            assert!(node.validate().is_err())
        }
    }
    for value in [f64::NAN, -21., 81.] {
        let mut node = node.clone();
        node.semantics
            .as_mut()
            .unwrap()
            .slider
            .as_mut()
            .unwrap()
            .range
            .value = value;
        assert!(node.validate().is_err())
    }
    for role in [None, Some(Role::Separator), Some(Role::ProgressBar)] {
        let mut node = node.clone();
        node.role = role;
        assert!(node.validate().is_err())
    }
    let mut field = node.clone();
    let s = field.semantics.as_mut().unwrap();
    s.required = true;
    s.readonly = true;
    s.invalid = true;
    s.disabled = true;
    s.form_control = Some(FormControl {
        name: "amount".into(),
        value: Some("30".into()),
    });
    field.focus = FocusMode::Programmatic;
    field.validate().unwrap();
    let mut bad = node.clone();
    bad.focus = FocusMode::Auto;
    assert!(bad.validate().is_err());
    let mut bad = node.clone();
    bad.semantics
        .as_mut()
        .unwrap()
        .slider
        .as_mut()
        .unwrap()
        .value_text = Some("x".repeat(1025));
    assert!(bad.validate().is_err());
}
#[test]
fn slider_keys_respect_axis_rtl_pages_editors_and_modifiers() {
    let key = |axis, direction, key| {
        RangeCommand::slider_key(
            axis,
            direction,
            KeyboardInput {
                key,
                ..Default::default()
            },
        )
    };
    assert_eq!(
        key(Orientation::Horizontal, Direction::Ltr, "ArrowRight"),
        Some(RangeCommand::Increase)
    );
    assert_eq!(
        key(Orientation::Horizontal, Direction::Rtl, "ArrowRight"),
        Some(RangeCommand::Decrease)
    );
    assert_eq!(
        key(Orientation::Vertical, Direction::Rtl, "ArrowUp"),
        Some(RangeCommand::Increase)
    );
    assert_eq!(
        key(Orientation::Horizontal, Direction::Ltr, "ArrowUp"),
        None
    );
    for (axis, key, command) in [
        (Orientation::Vertical, "Home", RangeCommand::Minimum),
        (Orientation::Horizontal, "End", RangeCommand::Maximum),
        (Orientation::Vertical, "PageUp", RangeCommand::PageIncrease),
        (
            Orientation::Horizontal,
            "PageDown",
            RangeCommand::PageDecrease,
        ),
    ] {
        assert_eq!(
            RangeCommand::slider_key(
                axis,
                Direction::Ltr,
                KeyboardInput {
                    key,
                    ..Default::default()
                }
            ),
            Some(command)
        );
    }
    for input in [
        KeyboardInput {
            key: "ArrowRight",
            editable: true,
            ..Default::default()
        },
        KeyboardInput {
            key: "ArrowRight",
            composing: true,
            ..Default::default()
        },
        KeyboardInput {
            key: "ArrowRight",
            control: true,
            ..Default::default()
        },
    ] {
        assert_eq!(
            RangeCommand::slider_key(Orientation::Horizontal, Direction::Ltr, input),
            None
        )
    }
    assert_eq!(
        RangeCommand::splitter_key(
            Orientation::Vertical,
            KeyboardInput {
                key: "PageUp",
                ..Default::default()
            }
        ),
        None
    );
}

#[test]
fn slider_events_use_exact_accepted_bounds_and_readonly_does_not_publish() {
    let node = slider();
    let mut ids = NodeIds::new(RuntimeId::new());
    let id = ids.allocate();
    let other = ids.allocate();
    for command in [
        RangeCommand::Increase,
        RangeCommand::PageDecrease,
        RangeCommand::Maximum,
        RangeCommand::Set(80.),
    ] {
        Event::Range { from: id, command }
            .validate_payload(id, &node)
            .unwrap();
    }
    for command in [
        RangeCommand::Set(f64::NAN),
        RangeCommand::Set(f64::INFINITY),
        RangeCommand::Set(-21.),
        RangeCommand::Set(81.),
    ] {
        assert!(
            Event::Range { from: id, command }
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
    let mut readonly = node.clone();
    readonly.semantics.as_mut().unwrap().readonly = true;
    let mut memory = memory::Memory::new(id.runtime());
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(readonly),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    for event in [
        Event::Range {
            from: id,
            command: RangeCommand::Increase,
        },
        Event::Drag {
            from: id,
            phase: DragPhase::Start,
            x: 1.,
            y: 2.,
        },
        Event::Drag {
            from: id,
            phase: DragPhase::Drop,
            x: 1.,
            y: 2.,
        },
    ] {
        assert!(!memory.validate_event(id, &event).unwrap());
    }
    assert!(
        memory
            .validate_event(
                id,
                &Event::Drag {
                    from: id,
                    phase: DragPhase::Cancel,
                    x: 1.,
                    y: 2.
                }
            )
            .unwrap()
    );
    let mut disabled = node;
    disabled.semantics.as_mut().unwrap().disabled = true;
    assert!(
        Event::Range {
            from: id,
            command: RangeCommand::Increase
        }
        .validate_payload(id, &disabled)
        .is_err()
    );
}
#[test]
fn sliders_reject_interactive_children_before_placement_and_keep_passive_visuals() {
    let mut ids = NodeIds::new(RuntimeId::new());
    let id = ids.allocate();
    let child = ids.allocate();
    let button = ids.allocate();
    let mut memory = memory::Memory::new(id.runtime());
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(slider()),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
            Mutation::Create {
                id: child,
                node: Box::new(Descriptor::new(Props::View)),
            },
            Mutation::Place {
                id: child,
                parent: Some(id),
                before: None,
            },
            Mutation::Create {
                id: button,
                node: Box::new(Descriptor::new(Props::Button(ButtonProps::default()))),
            },
        ])
        .unwrap();
    assert!(
        memory
            .apply(&[Mutation::Place {
                id: button,
                parent: Some(child),
                before: None
            }])
            .is_err()
    );
    assert_eq!(memory.node(button).unwrap().parent, None);
    assert_eq!(memory.node(child).unwrap().parent, Some(id));
}

#[test]
fn a_neighbour_fixed_slider_can_expose_a_readonly_closed_interval_without_relaxing_range_value() {
    let mut node = slider();
    let semantics = node.semantics.as_mut().unwrap();
    let value = semantics.slider.as_mut().unwrap();
    value.range = RangeValue {
        min: 30.,
        max: 30.,
        value: 30.,
    };
    semantics.readonly = true;
    node.validate().unwrap();
    let mut editable = node.clone();
    editable.semantics.as_mut().unwrap().readonly = false;
    assert!(editable.validate().is_err());
    assert!(
        node.semantics
            .as_ref()
            .unwrap()
            .slider
            .as_ref()
            .unwrap()
            .range
            .validate()
            .is_err()
    );
    let mut ids = NodeIds::new(RuntimeId::new());
    let id = ids.allocate();
    let mut memory = memory::Memory::new(id.runtime());
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(node),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    assert!(
        !memory
            .validate_event(
                id,
                &Event::Range {
                    from: id,
                    command: RangeCommand::Increase
                }
            )
            .unwrap()
    );
    assert!(
        Event::Range {
            from: id,
            command: RangeCommand::Set(31.)
        }
        .validate_payload(id, &memory.node(id).unwrap().descriptor)
        .is_err()
    );
}
