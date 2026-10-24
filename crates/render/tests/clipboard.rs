use glory_render::{memory::Memory, *};
fn descriptor(text: &str, revision: u64) -> Descriptor {
    let mut d = Descriptor::new(Props::Button(ButtonProps::default()));
    d.semantics = Some(Box::new(Semantics {
        clipboard: Some(ClipboardText {
            text: text.into(),
            revision,
        }),
        ..Default::default()
    }));
    d
}
fn patch(text: &str, revision: u64) -> Patch {
    Patch::Semantics(Change::Set(descriptor(text, revision).semantics))
}
#[test]
fn declaration_is_finite_canonical_and_intrinsic_to_an_independent_button() {
    let d = descriptor("a\n😀", 7);
    d.validate().unwrap();
    for text in [
        "\r".into(),
        "\0".into(),
        "x".repeat(MAX_CLIPBOARD_BYTES + 1),
    ] {
        assert!(descriptor(&text, 8).validate().is_err())
    }
    for props in [
        Props::Group,
        Props::View,
        Props::Text(TextProps::default()),
        Props::Input(InputProps::default()),
    ] {
        let mut n = d.clone();
        n.props = props;
        assert!(n.validate().is_err())
    }
    let mut n = d.clone();
    n.semantics.as_mut().unwrap().tab = true;
    assert!(n.validate().is_err());
    assert!(d.patched(patch("different", 7)).is_err());
    assert!(d.patched(patch("a\n😀", 6)).is_err());
    let disabled = {
        let mut n = d.clone();
        if let Props::Button(b) = &mut n.props {
            b.enabled = false
        }
        n
    };
    let id = NodeIds::new(RuntimeId::new()).allocate();
    assert!(
        Action::CopyText {
            node: id,
            revision: 7,
            request: 1
        }
        .validate_target(&disabled)
        .is_err()
    );
    assert!(
        Action::CopyText {
            node: id,
            revision: 6,
            request: 1
        }
        .validate_target(&d)
        .is_err()
    );
    assert!(
        Action::CopyText {
            node: id,
            revision: 7,
            request: 0
        }
        .validate_target(&d)
        .is_err()
    );
}
#[test]
fn guard_rollback_prefix_and_source_cancellation_preserve_request_watermark() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        Mutation::Create {
            id,
            node: Box::new(descriptor("one", 3)),
        },
        Mutation::Place {
            id,
            parent: None,
            before: None,
        },
    ])
    .unwrap();
    tree.admit_clipboard(Action::CopyText {
        node: id,
        revision: 3,
        request: 5,
    })
    .unwrap();
    let before = tree.stats();
    assert!(tree.prepare_patch(id, &patch("bad", 3)).is_err());
    assert_eq!(tree.stats(), before);
    assert!(tree.clipboard_pending(id, 5, 3).unwrap());
    {
        let _rollback = tree.prepare_patch(id, &patch("two", 4)).unwrap();
    }
    assert!(tree.clipboard_pending(id, 5, 3).unwrap());
    assert!(
        tree.apply(&[
            Mutation::Patch {
                id,
                patch: Patch::Classes(Change::Set(vec!["prefix".into()]))
            },
            Mutation::Patch {
                id,
                patch: patch("bad", 2)
            }
        ])
        .is_err()
    );
    assert_eq!(
        tree.node(id).unwrap().descriptor.view().classes,
        &["prefix"]
    );
    assert!(tree.clipboard_pending(id, 5, 3).unwrap());
    tree.apply(&[Mutation::Patch {
        id,
        patch: patch("two", 4),
    }])
    .unwrap();
    assert!(!tree.clipboard_pending(id, 5, 3).unwrap());
    assert_eq!(tree.next_clipboard_request(id), Ok(6));
    assert!(
        tree.admit_clipboard(Action::CopyText {
            node: id,
            revision: 4,
            request: 5
        })
        .is_err()
    );
    tree.admit_clipboard(Action::CopyText {
        node: id,
        revision: 4,
        request: 6,
    })
    .unwrap();
    assert!(
        tree.validate_event(
            id,
            &Event::Clipboard {
                request: 7,
                revision: 4,
                outcome: ClipboardOutcome::Written
            }
        )
        .is_err()
    );
    assert!(
        !tree
            .validate_event(
                id,
                &Event::Clipboard {
                    request: 5,
                    revision: 3,
                    outcome: ClipboardOutcome::Written
                }
            )
            .unwrap()
    );
    assert!(tree.finish_clipboard(id, 6, 4).unwrap());
    assert!(!tree.finish_clipboard(id, 6, 4).unwrap());
    assert!(
        !tree
            .validate_event(
                id,
                &Event::Clipboard {
                    request: 6,
                    revision: 4,
                    outcome: ClipboardOutcome::Written
                }
            )
            .unwrap()
    );
    tree.apply(&[Mutation::Patch {
        id,
        patch: Patch::Semantics(Change::Clear),
    }])
    .unwrap();
    assert_eq!(tree.next_clipboard_request(id), Ok(7));
    assert!(
        !tree
            .validate_event(
                id,
                &Event::Clipboard {
                    request: 6,
                    revision: 4,
                    outcome: ClipboardOutcome::Written
                }
            )
            .unwrap()
    );
    tree.apply(&[Mutation::Dispose { id }]).unwrap();
    assert!(tree.next_clipboard_request(id).is_err());
}
