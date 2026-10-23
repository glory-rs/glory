use glory_render::memory::Memory;
use glory_render::*;
use std::borrow::Cow;

#[test]
fn canonical_text_is_borrowed_and_host_normalization_handles_crlf_nul_and_unicode() {
    assert!(matches!(normalize_text("a🙂\n中", true), Cow::Borrowed(_)));
    assert!(matches!(normalize_text("a🙂中", false), Cow::Borrowed(_)));
    assert_eq!(normalize_text("\r\na\rb\0🙂\n", true), "\na\nb�🙂\n");
    assert_eq!(normalize_text("\r\na\rb\0🙂\n", false), "ab�🙂");
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut memory = Memory::new(runtime);
    memory
        .apply(&[Mutation::Create {
            id,
            node: Box::new(Descriptor::new(Props::Input(InputProps {
                value: "\r\ninitial\r\0🙂".into(),
                multiline: true,
                ..Default::default()
            }))),
        }])
        .unwrap();
    assert!(
        matches!(&memory.node(id).unwrap().descriptor.props, Props::Input(input) if input.value == "\ninitial\n�🙂" && input.revision == 0)
    );
}

#[test]
fn revisions_are_monotonic_keep_is_inert_and_equal_revisions_require_canonical_equal_values() {
    let mut descriptor = Descriptor::new(Props::Input(InputProps {
        value: "a\nb".into(),
        multiline: true,
        revision: 4,
        ..Default::default()
    }));
    let initial = descriptor.clone();
    for (value, revision) in [("same", 3), ("different", 4)] {
        assert_eq!(
            descriptor.patch(Patch::Value {
                value: Change::Set(value.into()),
                revision
            }),
            Err(Error::InputRevision {
                current: 4,
                incoming: revision
            })
        );
        assert_eq!(descriptor, initial);
    }
    descriptor
        .patch(Patch::Value {
            value: Change::Set("a\r\nb".into()),
            revision: 4,
        })
        .unwrap();
    assert_eq!(descriptor, initial);
    descriptor
        .patch(Patch::Value {
            value: Change::Set("a\r\nb".into()),
            revision: 5,
        })
        .unwrap();
    descriptor
        .patch(Patch::Value {
            value: Change::Keep,
            revision: 0,
        })
        .unwrap();
    descriptor
        .patch(Patch::Value {
            value: Change::Clear,
            revision: 6,
        })
        .unwrap();
    assert!(
        matches!(&descriptor.props, Props::Input(input) if input.value.is_empty() && input.revision == 6)
    );
    descriptor
        .patch(Patch::Value {
            value: Change::Set("last".into()),
            revision: u64::MAX,
        })
        .unwrap();
    descriptor
        .patch(Patch::Value {
            value: Change::Set("last".into()),
            revision: u64::MAX,
        })
        .unwrap();
    assert_eq!(
        descriptor.patch(Patch::Value {
            value: Change::Clear,
            revision: 0
        }),
        Err(Error::InputRevision {
            current: u64::MAX,
            incoming: 0
        })
    );
}

#[test]
fn rejected_program_writes_preserve_the_reference_node_and_next_commit_can_progress() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut memory = Memory::new(runtime);
    let first = memory
        .apply(&[Mutation::Create {
            id,
            node: Box::new(Descriptor::new(Props::Input(InputProps {
                value: "initial".into(),
                ..Default::default()
            }))),
        }])
        .unwrap();
    assert_eq!(
        memory.apply(&[Mutation::Patch {
            id,
            patch: Patch::Value {
                value: Change::Set("bad".into()),
                revision: 0
            }
        }]),
        Err(Error::InputRevision {
            current: 0,
            incoming: 0
        })
    );
    assert!(
        matches!(&memory.node(id).unwrap().descriptor.props, Props::Input(input) if input.value == "initial")
    );
    let next = memory
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::Value {
                value: Change::Set("next\0".into()),
                revision: 1,
            },
        }])
        .unwrap();
    assert_eq!(next.applied, first.applied + 1);
    assert!(
        matches!(&memory.node(id).unwrap().descriptor.props, Props::Input(input) if input.value == "next�")
    );
}

#[test]
fn future_input_events_and_noncanonical_text_are_rejected_before_callbacks() {
    let id = NodeIds::new(RuntimeId::new()).allocate();
    let node = Descriptor::new(Props::Input(InputProps {
        revision: 8,
        multiline: true,
        ..Default::default()
    }));
    let event = |revision, value: &str| Event::ValueChanged {
        value: value.into(),
        composing: false,
        revision,
    };
    assert_eq!(
        event(9, "future").validate_payload(id, &node),
        Err(Error::InputRevision {
            current: 8,
            incoming: 9
        })
    );
    assert_eq!(
        Event::Commit {
            value: "future".into(),
            revision: 9
        }
        .validate_payload(id, &node),
        Err(Error::InputRevision {
            current: 8,
            incoming: 9
        })
    );
    Event::Commit {
        value: "current".into(),
        revision: 8,
    }
    .validate_payload(id, &node)
    .unwrap();
    event(8, "current\n🙂").validate_payload(id, &node).unwrap();
    // Past versions are structurally valid; App discards them by its barrier.
    event(7, "past").validate_payload(id, &node).unwrap();
    for value in ["bad\0", "bad\rline"] {
        assert!(matches!(
            event(8, value).validate_payload(id, &node),
            Err(Error::InvalidEvent(_))
        ));
    }
}
