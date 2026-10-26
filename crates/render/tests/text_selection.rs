use glory_render::*;
#[test]
fn exact_selection_requires_live_value_revision_ordered_ranges_and_a_real_input() {
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
        assert!(
            Action::SelectText {
                node: id,
                revision: 0,
                selection: TextSelection::default()
            }
            .validate_target(&node)
            .is_ok()
        );
    }
}

#[test]
fn unicode_selection_converts_utf16_without_splitting_surrogates_or_allocating_a_buffer() {
    for (text, start, end, bytes) in [
        ("", 0, 0, 0..0),
        ("A中😀Z", 1, 4, 1..8),
        ("A中😀Z", 4, 4, 8..8),
        ("A中😀Z", 0, 5, 0..9),
        ("e\u{301}", 1, 2, 1..3),
        ("👩\u{200d}💻", 0, 5, 0..11),
        ("אב", 1, 2, 2..4),
    ] {
        assert_eq!(
            TextSelection {
                start,
                end,
                ..Default::default()
            }
            .byte_range(text)
            .unwrap(),
            bytes
        );
    }
    for (start, end) in [
        (3, 3),
        (1, 3),
        (3, 4),
        (4, 1),
        (0, 6),
        (usize::MAX, usize::MAX),
    ] {
        assert!(
            TextSelection {
                start,
                end,
                ..Default::default()
            }
            .byte_range("A中😀Z")
            .is_err(),
            "{start}..{end}"
        );
    }
}
