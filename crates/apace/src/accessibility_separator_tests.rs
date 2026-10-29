use super::*;
use glory_render::{Orientation, Semantics};
#[test]
fn passive_separator_has_native_orientation_and_no_adjustment_or_focus_actions() {
    let app = WindowApp::new();
    app.mount(
        view()
            .role(Role::Separator)
            .semantics(Some(Box::new(Semantics {
                separator_axis: Some(Orientation::Vertical),
                ..Default::default()
            }))),
    )
    .unwrap();
    let tree = Consumer::new(build(&app, true), true);
    let all = nodes(&tree);
    let node = all
        .iter()
        .find(|node| node.role() == AxRole::Splitter)
        .unwrap();
    assert_eq!(
        node.data().orientation(),
        Some(::accesskit::Orientation::Vertical)
    );
    assert!(!node.data().supports_action(AxAction::Focus));
    assert!(!node.data().supports_action(AxAction::SetValue));
    assert!(!node.data().supports_action(AxAction::Click));
}

#[test]
fn interactive_splitter_exports_numeric_range_and_queues_accesskit_adjustment_proposals() {
    use glory_render::{RangeCommand, RangeValue, SemanticId};
    let app = WindowApp::new();
    let handle = app.app.scope().cage(None);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    let metadata = Semantics {
        splitter: Some(RangeValue {
            min: 10.0,
            max: 90.0,
            value: 50.0,
        }),
        separator_axis: Some(Orientation::Vertical),
        controls: Some(SemanticId::new("primary-pane").unwrap()),
        ..Default::default()
    };
    app.mount(
        view().fill((
            view()
                .name("Primary pane")
                .semantics(Some(Box::new(Semantics {
                    id: Some(SemanticId::new("primary-pane").unwrap()),
                    ..Default::default()
                }))),
            view()
                .role(Role::Separator)
                .name("Resize panes")
                .focus(FocusMode::TabStop)
                .node_ref(handle)
                .style(
                    Style::new()
                        .width(Length::Px(8.0))
                        .height(Length::Px(120.0)),
                )
                .semantics(Some(Box::new(metadata.clone())))
                .on(glory_render::EventKind::Range, move |event| {
                    if let Event::Range { command, .. } = event {
                        output.borrow_mut().push(command);
                    }
                }),
        )),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&app, true), true);
    let target = id(&app, handle.get().unwrap());
    let full = nodes(&tree)
        .into_iter()
        .find(|node| node.role() == AxRole::Splitter)
        .unwrap()
        .id();
    let node = tree.state().node_by_id(full).unwrap();
    assert_eq!(node.role(), AxRole::Splitter);
    assert_eq!(node.data().min_numeric_value(), Some(10.0));
    assert_eq!(node.data().max_numeric_value(), Some(90.0));
    assert_eq!(node.data().numeric_value(), Some(50.0));
    assert!(node.data().supports_action(AxAction::Focus));
    assert!(node.data().supports_action(AxAction::Increment));
    assert!(node.data().supports_action(AxAction::Decrement));
    assert!(node.data().supports_action(AxAction::SetValue));
    assert!(action(&app, &request(target, AxAction::Increment, None)).unwrap());
    assert!(
        action(
            &app,
            &request(
                target,
                AxAction::SetValue,
                Some(ActionData::NumericValue(75.0))
            )
        )
        .unwrap()
    );
    assert!(
        action(
            &app,
            &request(
                target,
                AxAction::SetValue,
                Some(ActionData::NumericValue(f64::NAN))
            )
        )
        .is_err()
    );
    assert!(seen.borrow().is_empty());
    app.app.pump().unwrap();
    assert_eq!(
        *seen.borrow(),
        vec![RangeCommand::Increase, RangeCommand::Set(75.0)]
    );
    let mut disabled = metadata;
    disabled.disabled = true;
    app.app
        .scope()
        .patch(
            handle.get().unwrap(),
            glory_render::Patch::Semantics(glory_render::Change::Set(Some(Box::new(disabled)))),
        )
        .unwrap();
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    let node = tree.state().node_by_id(full).unwrap();
    assert!(node.data().is_disabled());
    assert!(!node.data().supports_action(AxAction::Increment));
    assert!(!action(&app, &request(target, AxAction::Increment, None)).unwrap());
}

#[test]
fn slider_accesskit_has_its_own_role_steps_value_text_and_readonly_adjustment_policy() {
    use glory_render::{Direction, RangeCommand, RangeValue, SliderValue};
    let app = WindowApp::new();
    let reference = app.app.scope().cage(None);
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let metadata = Semantics {
        slider: Some(SliderValue {
            range: RangeValue {
                min: -10.,
                max: 90.,
                value: 30.,
            },
            axis: Orientation::Horizontal,
            direction: Direction::Rtl,
            step: 0.5,
            page_step: 5.,
            value_text: Some("Thirty units".into()),
        }),
        required: true,
        ..Default::default()
    };
    app.mount(
        view()
            .role(Role::Slider)
            .focus(FocusMode::TabStop)
            .name("Amount")
            .node_ref(reference)
            .style(Style::new().width(Length::Px(200.)).height(Length::Px(30.)))
            .semantics(Some(Box::new(metadata.clone())))
            .fill(view().name("Passive visual"))
            .on(glory_render::EventKind::Range, move |event| {
                output.borrow_mut().push(event)
            }),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&app, true), true);
    let target = id(&app, reference.get().unwrap());
    let full = nodes(&tree)
        .into_iter()
        .find(|node| node.role() == AxRole::Slider)
        .unwrap()
        .id();
    let node = tree.state().node_by_id(full).unwrap();
    assert_eq!(node.role(), AxRole::Slider);
    assert!(node.data().children().is_empty());
    assert!(
        !nodes(&tree)
            .iter()
            .any(|node| node.data().label() == Some("Passive visual"))
    );
    assert_eq!(node.data().min_numeric_value(), Some(-10.));
    assert_eq!(node.data().max_numeric_value(), Some(90.));
    assert_eq!(node.data().numeric_value(), Some(30.));
    assert_eq!(node.data().numeric_value_step(), Some(0.5));
    assert_eq!(node.data().value(), Some("Thirty units"));
    assert!(node.data().is_required());
    assert!(node.data().supports_action(AxAction::Increment));
    assert!(
        action(
            &app,
            &request(
                target,
                AxAction::SetValue,
                Some(ActionData::NumericValue(55.))
            )
        )
        .unwrap()
    );
    assert!(events.borrow().is_empty());
    app.app.pump().unwrap();
    assert!(matches!(
        events.borrow().as_slice(),
        [Event::Range {
            command: RangeCommand::Set(55.),
            ..
        }]
    ));
    let mut readonly = metadata;
    readonly.readonly = true;
    app.app
        .scope()
        .patch(
            reference.get().unwrap(),
            glory_render::Patch::Semantics(glory_render::Change::Set(Some(Box::new(readonly)))),
        )
        .unwrap();
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    let node = tree.state().node_by_id(full).unwrap();
    assert!(node.data().is_read_only());
    assert!(node.data().supports_action(AxAction::Focus));
    assert!(!node.data().supports_action(AxAction::Increment));
    assert!(!node.data().supports_action(AxAction::SetValue));
    assert!(action(&app, &request(target, AxAction::Increment, None)).is_err());
}

#[test]
fn spinbutton_accesskit_preserves_exact_text_optional_coordinates_and_queues_numeric_requests() {
    use glory_render::{EventKind, RangeCommand, SpinValue};
    let app = WindowApp::new();
    let reference = app.app.scope().cage(None);
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let metadata = Semantics {
        spinbutton: Some(SpinValue {
            min: Some(0.),
            max: Some(10.),
            value: Some(1.),
            value_text: Some("1.00 exact".into()),
            can_decrease: false,
            ..Default::default()
        }),
        ..Default::default()
    };
    app.mount(
        glory_ui::input()
            .name("Quantity")
            .value("1.00")
            .node_ref(reference)
            .style(Style::new().width(Length::Px(200.)).height(Length::Px(30.)))
            .semantics(Some(Box::new(metadata.clone())))
            .on(EventKind::Spin, move |event| {
                output.borrow_mut().push(event)
            }),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&app, true), true);
    let target = id(&app, reference.get().unwrap());
    let full = nodes(&tree)
        .into_iter()
        .find(|n| n.role() == AxRole::SpinButton)
        .unwrap()
        .id();
    let node = tree.state().node_by_id(full).unwrap();
    assert_eq!(node.data().numeric_value(), Some(1.));
    assert_eq!(node.data().value(), Some("1.00 exact"));
    assert!(node.data().supports_action(AxAction::Increment));
    assert!(!node.data().supports_action(AxAction::Decrement));
    assert!(!action(&app, &request(target, AxAction::Decrement, None)).unwrap());
    assert!(action(&app, &request(target, AxAction::Increment, None)).unwrap());
    assert!(
        action(
            &app,
            &request(
                target,
                AxAction::SetValue,
                Some(ActionData::NumericValue(2.5))
            )
        )
        .unwrap()
    );
    assert!(events.borrow().is_empty());
    app.app.pump().unwrap();
    assert!(matches!(
        events.borrow().as_slice(),
        [
            Event::Spin {
                command: RangeCommand::Increase,
                ..
            },
            Event::Spin {
                command: RangeCommand::Set(2.5),
                ..
            }
        ]
    ));
    let mut empty = metadata;
    empty.spinbutton.as_mut().unwrap().value = None;
    empty.spinbutton.as_mut().unwrap().min = None;
    empty.spinbutton.as_mut().unwrap().max = None;
    empty.spinbutton.as_mut().unwrap().value_text = Some("-".into());
    empty.spinbutton.as_mut().unwrap().can_increase = false;
    empty.readonly = true;
    app.app
        .scope()
        .patch(
            reference.get().unwrap(),
            glory_render::Patch::Semantics(glory_render::Change::Set(Some(Box::new(empty)))),
        )
        .unwrap();
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    let node = tree.state().node_by_id(full).unwrap();
    assert_eq!(node.role(), AxRole::SpinButton);
    assert_eq!(node.data().numeric_value(), None);
    assert_eq!(node.data().min_numeric_value(), None);
    assert_eq!(node.data().value(), Some("-"));
    assert!(!node.data().supports_action(AxAction::Increment));
    assert!(!node.data().supports_action(AxAction::SetValue));
    assert!(!action(&app, &request(target, AxAction::Increment, None)).unwrap());
}
