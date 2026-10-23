use glory_render::memory::Memory;
use glory_render::*;

fn create(memory: &mut Memory, id: NodeId, descriptor: Descriptor, parent: Option<NodeId>) {
    memory
        .apply(&[
            Mutation::Create {
                id,
                node: Box::new(descriptor),
            },
            Mutation::Place {
                id,
                parent,
                before: None,
            },
        ])
        .unwrap();
}

#[test]
fn unrelated_fields_and_keep_preserve_large_value_and_class_buffers() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut memory = Memory::new(runtime);
    let mut descriptor = Descriptor::new(Props::Input(InputProps {
        value: "x".repeat(1_048_576),
        ..Default::default()
    }));
    descriptor.classes = (0..500).map(|index| format!("token-{index}")).collect();
    create(&mut memory, id, descriptor, None);
    let pointers = |memory: &Memory| {
        let descriptor = &memory.node(id).unwrap().descriptor;
        let Props::Input(input) = &descriptor.props else {
            panic!()
        };
        (input.value.as_ptr(), descriptor.classes.as_ptr())
    };
    let original = pointers(&memory);
    for _ in 0..20 {
        for patch in [
            Patch::Name(Change::Set(Some("Name".into()))),
            Patch::Name(Change::Keep),
            Patch::Focus(Change::Set(FocusMode::Programmatic)),
            Patch::Enabled(Change::Set(false)),
            Patch::Classes(Change::Keep),
            Patch::Style(Change::Set(Box::new(
                style::Style::new().width(style::Length::Px(10.)),
            ))),
        ] {
            memory.apply(&[Mutation::Patch { id, patch }]).unwrap();
            assert_eq!(
                pointers(&memory),
                original,
                "unrelated payload was replaced"
            );
        }
    }
}

#[test]
fn prepared_patch_drop_unwind_and_commit_preserve_atomicity_and_watermarks() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut memory = Memory::new(runtime);
    create(&mut memory, id, Descriptor::new(Props::View), None);
    let receipt = memory.apply(&[]).unwrap();
    let stats = memory.stats();
    {
        let prepared = memory
            .prepare_patch(id, &Patch::Name(Change::Set(Some("staged".into()))))
            .unwrap();
        assert_eq!(prepared.descriptor().name.as_deref(), Some("staged"));
        assert_eq!(prepared.previous(), &Patch::Name(Change::Set(None)));
    }
    assert_eq!(memory.node(id).unwrap().descriptor.name, None);
    assert_eq!(memory.stats(), stats);
    assert_eq!(memory.apply(&[]).unwrap(), receipt);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _prepared = memory
                .prepare_patch(id, &Patch::Role(Change::Set(Some(Role::Status))))
                .unwrap();
            panic!("adapter lowering failed");
        }))
        .is_err()
    );
    assert_eq!(memory.node(id).unwrap().descriptor.role, None);
    assert_eq!(memory.stats(), stats);
    assert_eq!(memory.apply(&[]).unwrap(), receipt);
    let committed = memory
        .prepare_patch(id, &Patch::Name(Change::Set(Some("accepted".into()))))
        .unwrap()
        .commit();
    assert_eq!(committed.applied, receipt.applied + 1);
    assert_eq!(memory.stats().patched, stats.patched + 1);
    assert_eq!(
        memory.node(id).unwrap().descriptor.name.as_deref(),
        Some("accepted")
    );
}

#[test]
fn failed_ancestor_content_and_input_revisions_restore_the_original_buffers() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut memory = Memory::new(runtime);
    let button = ids.allocate();
    let text = ids.allocate();
    let input = ids.allocate();
    create(
        &mut memory,
        button,
        Descriptor::new(Props::Button(Default::default())),
        None,
    );
    create(
        &mut memory,
        text,
        Descriptor::new(Props::Text(TextProps {
            content: "x".repeat(1024),
            ..Default::default()
        })),
        Some(button),
    );
    create(
        &mut memory,
        input,
        Descriptor::new(Props::Input(InputProps {
            value: "x".repeat(1024),
            revision: 100,
            ..Default::default()
        })),
        None,
    );
    let pointers = |memory: &Memory| {
        let Props::Text(text) = &memory.node(text).unwrap().descriptor.props else {
            panic!()
        };
        let Props::Input(input) = &memory.node(input).unwrap().descriptor.props else {
            panic!()
        };
        (text.content.as_ptr(), input.value.as_ptr())
    };
    let original = pointers(&memory);
    let stats = memory.stats();
    let receipt = memory.apply(&[]).unwrap();
    for (id, patch) in [
        (text, Patch::TextKind(Change::Set(TextKind::Paragraph))),
        (
            input,
            Patch::Value {
                value: Change::Set("wrong".into()),
                revision: 100,
            },
        ),
        (
            input,
            Patch::Value {
                value: Change::Clear,
                revision: 99,
            },
        ),
    ] {
        assert!(memory.apply(&[Mutation::Patch { id, patch }]).is_err());
        assert_eq!(pointers(&memory), original);
        assert_eq!(memory.stats(), stats);
        assert_eq!(memory.apply(&[]).unwrap(), receipt);
    }
    memory
        .apply(&[Mutation::Patch {
            id: input,
            patch: Patch::Value {
                value: Change::Set("next\0".into()),
                revision: 101,
            },
        }])
        .unwrap();
    assert!(
        matches!(&memory.node(input).unwrap().descriptor.props, Props::Input(input) if input.value == "next�" && input.revision == 101)
    );
}

#[test]
fn live_field_validation_matches_full_descriptor_validation_for_all_patch_variants() {
    let mut bases = vec![
        Props::View,
        Props::Group,
        Props::Text(Default::default()),
        Props::Text(TextProps {
            kind: TextKind::Heading(2),
            ..Default::default()
        }),
        Props::Button(Default::default()),
        Props::Input(InputProps {
            value: "old".into(),
            revision: 1,
            ..Default::default()
        }),
        Props::Input(InputProps {
            value: "a\nb".into(),
            multiline: true,
            revision: 1,
            ..Default::default()
        }),
        Props::Check(Default::default()),
        Props::Image(Default::default()),
        Props::Image(ImageProps {
            alternative: Some("description".into()),
            ..Default::default()
        }),
        Props::Link(Default::default()),
    ]
    .into_iter()
    .map(Descriptor::new)
    .collect::<Vec<_>>();
    let mut item = Descriptor::new(Props::View);
    item.role = Some(Role::ListItem);
    item.item = Some(ItemPosition {
        index: 1,
        count: Some(3),
    });
    bases.push(item);
    let mut patches = vec![
        Patch::Semantics(Change::Keep),
        Patch::Semantics(Change::Clear),
        Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            selected: Some(true),
            tab: true,
            controls: Some(SemanticId::new("panel").unwrap()),
            ..Default::default()
        })))),
        Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            hidden: true,
            ..Default::default()
        })))),
        Patch::Text(Change::Set("new\r\n\0".into())),
        Patch::TextKind(Change::Set(TextKind::Heading(7))),
        Patch::TextKind(Change::Clear),
        Patch::Selectable(Change::Clear),
        Patch::Label(Change::Set("label".into())),
        Patch::Enabled(Change::Set(false)),
        Patch::Enabled(Change::Clear),
        Patch::Value {
            value: Change::Set("new\r\n\0".into()),
            revision: 2,
        },
        Patch::Value {
            value: Change::Set("old".into()),
            revision: 1,
        },
        Patch::Value {
            value: Change::Set("new".into()),
            revision: 1,
        },
        Patch::Value {
            value: Change::Keep,
            revision: u64::MAX,
        },
        Patch::Value {
            value: Change::Clear,
            revision: 0,
        },
        Patch::Purpose(Change::Set(Purpose::Password)),
        Patch::Multiline(Change::Set(true)),
        Patch::Multiline(Change::Clear),
        Patch::Checked(Change::Set(true)),
        Patch::Source(Change::Set("image.png".into())),
        Patch::Alternative(Change::Set(Some("alt".into()))),
        Patch::Alternative(Change::Clear),
        Patch::Target(Change::Clear),
        Patch::Classes(Change::Set(vec!["bad token".into()])),
        Patch::Classes(Change::Clear),
        Patch::Style(Change::Set(Box::new(
            style::Style::new().width(style::Length::Px(f64::NAN)),
        ))),
        Patch::Style(Change::Clear),
        Patch::Role(Change::Set(Some(Role::List))),
        Patch::Role(Change::Clear),
        Patch::Name(Change::Set(Some("Name".into()))),
        Patch::Name(Change::Clear),
        Patch::Focus(Change::Set(FocusMode::TabStop)),
        Patch::Focus(Change::Clear),
        Patch::Item(Change::Set(Some(ItemPosition {
            index: usize::MAX,
            count: None,
        }))),
        Patch::Item(Change::Clear),
    ];
    patches.extend([
        Patch::Text(Change::Keep),
        Patch::TextKind(Change::Keep),
        Patch::Selectable(Change::Keep),
        Patch::Label(Change::Keep),
        Patch::Enabled(Change::Keep),
        Patch::Purpose(Change::Keep),
        Patch::Multiline(Change::Keep),
        Patch::Checked(Change::Keep),
        Patch::Source(Change::Keep),
        Patch::Alternative(Change::Keep),
        Patch::Target(Change::Keep),
        Patch::Classes(Change::Keep),
        Patch::Style(Change::Keep),
        Patch::Role(Change::Keep),
        Patch::Name(Change::Keep),
        Patch::Focus(Change::Keep),
        Patch::Item(Change::Keep),
    ]);
    for base in bases {
        for patch in &patches {
            let runtime = RuntimeId::new();
            let id = NodeIds::new(runtime).allocate();
            let mut memory = Memory::new(runtime);
            create(&mut memory, id, base.clone(), None);
            let expected = base.patched(patch.clone());
            let actual = memory.apply(&[Mutation::Patch {
                id,
                patch: patch.clone(),
            }]);
            match expected {
                Ok(expected) => {
                    actual.unwrap();
                    assert_eq!(memory.node(id).unwrap().descriptor, expected, "{patch:?}");
                }
                Err(error) => {
                    assert_eq!(actual, Err(error), "{patch:?}");
                    assert_eq!(memory.node(id).unwrap().descriptor, base);
                }
            }
        }
    }
}
