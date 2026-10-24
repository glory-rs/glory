use glory_render::*;
fn spin() -> Descriptor {
    let mut node = Descriptor::new(Props::Input(InputProps::default()));
    node.semantics = Some(Box::new(Semantics {
        spinbutton: Some(SpinValue {
            min: Some(0.),
            max: Some(10.),
            value: Some(2.),
            value_text: Some("2.00 exact".into()),
            ..Default::default()
        }),
        ..Default::default()
    }));
    node
}
#[test]
fn spinbuttons_are_intrinsic_single_line_editors_and_preserve_optional_values() {
    let mut node = spin();
    node.validate().unwrap();
    assert_eq!(node.effective_role(), Some(Role::SpinButton));
    for props in [
        Props::View,
        Props::Button(ButtonProps::default()),
        Props::Input(InputProps {
            multiline: true,
            ..Default::default()
        }),
        Props::Input(InputProps {
            purpose: Purpose::Password,
            ..Default::default()
        }),
    ] {
        let mut bad = node.clone();
        bad.props = props;
        assert!(bad.validate().is_err());
    }
    node.semantics
        .as_mut()
        .unwrap()
        .spinbutton
        .as_mut()
        .unwrap()
        .value = None;
    node.validate().unwrap();
    node.semantics
        .as_mut()
        .unwrap()
        .spinbutton
        .as_mut()
        .unwrap()
        .value = Some(11.);
    assert!(node.validate().is_err());
    node.semantics.as_mut().unwrap().invalid = true;
    node.validate().unwrap();
    let mut fake = Descriptor::new(Props::View);
    fake.role = Some(Role::SpinButton);
    assert!(fake.validate().is_err());
}
#[test]
fn spin_events_are_independent_finite_exact_origin_commands_with_availability() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let other = ids.allocate();
    let mut node = spin();
    EventKind::Spin.validate_target(&node).unwrap();
    assert!(EventKind::Range.validate_target(&node).is_err());
    for command in [
        RangeCommand::Increase,
        RangeCommand::Decrease,
        RangeCommand::Minimum,
        RangeCommand::Maximum,
        RangeCommand::Set(1.),
    ] {
        Event::Spin { from: id, command }
            .validate_payload(id, &node)
            .unwrap();
    }
    assert!(
        Event::Spin {
            from: other,
            command: RangeCommand::Increase
        }
        .validate_payload(id, &node)
        .is_err()
    );
    assert!(
        Event::Spin {
            from: id,
            command: RangeCommand::Set(f64::NAN)
        }
        .validate_payload(id, &node)
        .is_err()
    );
    node.semantics
        .as_mut()
        .unwrap()
        .spinbutton
        .as_mut()
        .unwrap()
        .can_increase = false;
    assert!(
        Event::Spin {
            from: id,
            command: RangeCommand::Increase
        }
        .validate_payload(id, &node)
        .is_err()
    );
    node.semantics.as_mut().unwrap().readonly = true;
    assert!(node.validate().is_err());
    node.semantics
        .as_mut()
        .unwrap()
        .spinbutton
        .as_mut()
        .unwrap()
        .can_decrease = false;
    node.validate().unwrap();
    assert!(
        Event::Spin {
            from: id,
            command: RangeCommand::Set(3.)
        }
        .validate_payload(id, &node)
        .is_err()
    );
    let ordinary = Descriptor::new(Props::Input(InputProps::default()));
    assert!(EventKind::Spin.validate_target(&ordinary).is_err());
}
#[test]
fn exact_decimal_coordinates_may_collapse_to_the_same_accessibility_float() {
    let mut node = spin();
    let spin = node
        .semantics
        .as_mut()
        .unwrap()
        .spinbutton
        .as_mut()
        .unwrap();
    spin.min = Some(1e18);
    spin.max = Some(1e18);
    spin.value = Some(1e18);
    assert!(spin.can_increase);
    node.validate().unwrap();
}
