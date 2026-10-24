use glory_render::*;
fn node() -> Descriptor {
    let mut n = Descriptor::new(Props::Input(InputProps {
        value: "12-34".into(),
        revision: 7,
        ..Default::default()
    }));
    n.semantics = Some(Box::new(Semantics {
        input_edit: Some(InputEditCapture { max_bytes: 32 }),
        ..Default::default()
    }));
    n
}
#[test]
fn edit_capture_has_explicit_target_budget_and_single_typed_path() {
    let n = node();
    n.validate().unwrap();
    EventKind::InputEdit.validate_target(&n).unwrap();
    assert!(
        EventKind::InputEdit
            .validate_target(&Descriptor::new(Props::Input(Default::default())))
            .is_err()
    );
    for max_bytes in [0, 4097, usize::MAX] {
        let mut n = n.clone();
        n.semantics.as_mut().unwrap().input_edit = Some(InputEditCapture { max_bytes });
        assert!(n.validate().is_err());
    }
    for props in [
        Props::View,
        Props::Button(Default::default()),
        Props::Input(InputProps {
            multiline: true,
            ..Default::default()
        }),
    ] {
        let mut n = n.clone();
        n.props = props;
        assert!(n.validate().is_err());
    }
    let mut n = n.clone();
    n.semantics.as_mut().unwrap().tags_input = Some(TagSeparators::default());
    assert!(n.validate().is_err());
}
#[test]
fn edit_payload_validates_current_revision_utf16_direction_and_canonical_bounded_text() {
    let n = node();
    let id = NodeIds::new(RuntimeId::new()).allocate();
    let edit = InputEdit::Value {
        value: "12-34".into(),
        revision: 7,
        selection: TextSelection {
            start: 2,
            end: 4,
            direction: SelectionDirection::Backward,
        },
        before: Some(TextSelection {
            start: 1,
            end: 3,
            direction: SelectionDirection::Forward,
        }),
        intent: InputEditIntent::Insert,
        composing: false,
    };
    Event::InputEdit(edit.clone())
        .validate_payload(id, &n)
        .unwrap();
    for value in ["0".repeat(33), "a\0".into(), "a\r".into(), "a\n".into()] {
        let mut edit = edit.clone();
        if let InputEdit::Value { value: v, .. } = &mut edit {
            *v = value
        }
        assert!(Event::InputEdit(edit).validate_payload(id, &n).is_err());
    }
    let mut edit = edit.clone();
    if let InputEdit::Value { revision, .. } = &mut edit {
        *revision = 8
    }
    assert!(matches!(
        Event::InputEdit(edit).validate_payload(id, &n),
        Err(Error::InputRevision { .. })
    ));
    Event::InputEdit(InputEdit::Rejected {
        revision: 7,
        reason: InputEditRejection::TooLong,
    })
    .validate_payload(id, &n)
    .unwrap();
}
