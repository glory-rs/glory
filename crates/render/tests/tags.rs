use glory_render::{memory::Memory, *};
fn editor() -> Descriptor {
    let mut value = Descriptor::new(Props::Input(InputProps {
        revision: 8,
        ..Default::default()
    }));
    value.semantics = Some(Box::new(Semantics {
        tags_input: Some(TagSeparators::default()),
        required: true,
        ..Default::default()
    }));
    value
}
#[test]
fn tags_policy_is_an_intrinsic_independent_input_and_native_edit_keys_stay_native() {
    let mut value = editor();
    value.validate().unwrap();
    value.props = Props::View;
    assert!(value.validate().is_err());
    value.props = Props::Input(InputProps {
        multiline: true,
        ..Default::default()
    });
    assert!(value.validate().is_err());
    value = editor();
    value.semantics.as_mut().unwrap().form_control = Some(FormControl {
        name: "search".into(),
        value: None,
    });
    assert!(value.validate().is_err());
    let policy = TagSeparators::EnterCommaSemicolon;
    for key in ["Home", "End", " ", "Tab", "ArrowLeft", "ArrowRight"] {
        assert_eq!(
            policy.key(
                KeyboardInput {
                    key,
                    ..Default::default()
                },
                false
            ),
            None
        );
    }
    assert_eq!(
        policy.key(
            KeyboardInput {
                key: "Enter",
                ..Default::default()
            },
            true
        ),
        None
    );
    assert_eq!(
        policy.key(
            KeyboardInput {
                key: "Enter",
                ..Default::default()
            },
            false
        ),
        Some(TagKey::Commit)
    );
    assert_eq!(
        policy.key(
            KeyboardInput {
                key: ";",
                ..Default::default()
            },
            false
        ),
        Some(TagKey::Commit)
    );
    assert_eq!(
        TagSeparators::Enter.key(
            KeyboardInput {
                key: ",",
                ..Default::default()
            },
            false
        ),
        None
    );
    assert_eq!(
        policy.key(
            KeyboardInput {
                key: "Backspace",
                ..Default::default()
            },
            true
        ),
        Some(TagKey::RemoveLast)
    );
    assert_eq!(
        policy.key(
            KeyboardInput {
                key: "Backspace",
                composing: true,
                ..Default::default()
            },
            true
        ),
        None
    );
    assert_eq!(
        policy.key(
            KeyboardInput {
                key: "Enter",
                repeat: true,
                ..Default::default()
            },
            false
        ),
        None
    );
}
#[test]
fn tag_event_payload_checks_exact_identity_revision_and_bounded_utf16_paste() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let other = ids.allocate();
    let input = editor();
    let event = |from, revision, command| Event::Tags {
        from,
        revision,
        command,
    };
    let paste = TagsCommand::Paste {
        value: "A😀B".into(),
        text: "Beta\nGamma".into(),
        start: 1,
        end: 3,
    };
    event(id, 8, paste.clone())
        .validate_payload(id, input.view())
        .unwrap();
    assert!(
        event(other, 8, paste.clone())
            .validate_payload(id, input.view())
            .is_err()
    );
    assert_eq!(
        event(id, 9, paste).validate_payload(id, input.view()),
        Err(Error::InputRevision {
            current: 8,
            incoming: 9
        })
    );
    assert!(
        TagsCommand::Paste {
            value: "😀".into(),
            text: "x,y".into(),
            start: 1,
            end: 2
        }
        .validate()
        .is_err()
    );
    assert!(
        TagsCommand::Paste {
            value: "a".into(),
            text: "x".repeat(MAX_TAG_PASTE_BYTES + 1),
            start: 0,
            end: 1
        }
        .validate()
        .is_err()
    );
    assert!(
        TagsCommand::Paste {
            value: "a".into(),
            text: "x,y".into(),
            start: 1,
            end: 0
        }
        .validate()
        .is_err()
    );
    assert!(
        TagsCommand::Commit {
            value: "x".repeat(MAX_TAG_INPUT_BYTES + 1)
        }
        .validate()
        .is_err()
    );
    assert!(
        TagsCommand::Commit {
            value: "a\nb".into()
        }
        .validate()
        .is_err()
    );
}
#[test]
fn tags_memory_routing_and_patch_guard_do_not_reinterpret_retired_or_disabled_inputs() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut memory = Memory::new(runtime);
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(editor()),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    let event = Event::Tags {
        from: id,
        revision: 8,
        command: TagsCommand::RemoveLast,
    };
    assert_eq!(memory.validate_event(id, &event), Ok(true));
    let previous = memory.stats();
    let mut metadata = editor().semantics.unwrap();
    metadata.combobox = Some(ComboboxKind::Multiple);
    assert!(
        memory
            .apply(&[Mutation::Patch {
                id,
                patch: Patch::Semantics(Change::Set(Some(metadata)))
            }])
            .is_err()
    );
    assert_eq!(memory.stats(), previous);
    memory
        .apply(&[Mutation::Patch {
            id,
            patch: Patch::Enabled(Change::Set(false)),
        }])
        .unwrap();
    // Identity validation and edit admission are separate: push_event rejects
    // disabled controls, while a direct App handler rechecks its edit policy.
    assert_eq!(memory.validate_event(id, &event), Ok(true));
    assert!(memory.push_event(id, event.clone()).is_err());
    memory.apply(&[Mutation::Dispose { id }]).unwrap();
    assert_eq!(memory.validate_event(id, &event), Err(Error::NodeGone(id)));
}
