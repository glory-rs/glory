use glory_render::*;
#[test]
fn exact_ascii_selection_requires_live_value_revision_ordered_ranges_and_a_real_input() {
    let id = NodeIds::new(RuntimeId::new()).allocate();
    let node = Descriptor::new(Props::Input(InputProps {
        value: "000001".into(),
        revision: 7,
        ..Default::default()
    }));
    for direction in [
        SelectionDirection::None,
        SelectionDirection::Forward,
        SelectionDirection::Backward,
    ] {
        Action::SelectText {
            node: id,
            revision: 7,
            selection: TextSelection {
                start: 1,
                end: 5,
                direction,
            },
        }
        .validate_target(&node)
        .unwrap();
    }
    for (start, end) in [(5, 1), (0, 7), (usize::MAX, usize::MAX)] {
        assert!(
            Action::SelectText {
                node: id,
                revision: 7,
                selection: TextSelection {
                    start,
                    end,
                    ..Default::default()
                }
            }
            .validate_target(&node)
            .is_err()
        );
    }
    for revision in [0, 6, 8, u64::MAX] {
        assert!(matches!(
            Action::SelectText {
                node: id,
                revision,
                selection: TextSelection::default()
            }
            .validate_target(&node),
            Err(Error::InputRevision { .. })
        ));
    }
    assert!(
        Action::SelectText {
            node: id,
            revision: 7,
            selection: TextSelection::default()
        }
        .validate_target(&Descriptor::new(Props::View))
        .is_err()
    );
    for value in ["🙂", "e\u{301}", "中"] {
        let node = Descriptor::new(Props::Input(InputProps {
            value: value.into(),
            ..Default::default()
        }));
        assert!(matches!(
            Action::SelectText {
                node: id,
                revision: 0,
                selection: TextSelection::default()
            }
            .validate_target(&node),
            Err(Error::Unsupported(_))
        ));
    }
}
