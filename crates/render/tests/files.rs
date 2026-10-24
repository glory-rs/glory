use glory_render::{memory::Memory, *};
fn change(configuration: FileConfig, revision: u64) -> Patch {
    Patch::File {
        configuration: Change::Set(Box::new(configuration)),
        revision,
    }
}
fn config() -> FileConfig {
    FileConfig {
        multiple: true,
        max_files: 3,
        filters: vec![
            FileFilter::Mime("image/*".into()),
            FileFilter::Extension(".txt".into()),
        ],
        ..Default::default()
    }
}
fn descriptor() -> Descriptor {
    Descriptor::new(Props::File(FileProps {
        configuration: config(),
        revision: 4,
        ..Default::default()
    }))
}
#[test]
fn file_configuration_live_guard_is_one_field_rollback_default_keep_and_prefix_safe() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        Mutation::Create {
            id,
            node: Box::new(descriptor()),
        },
        Mutation::Place {
            id,
            parent: None,
            before: None,
        },
    ])
    .unwrap();
    let original = tree.node(id).unwrap().descriptor.clone();
    let stats = tree.stats();
    let next = FileConfig {
        max_files: 2,
        ..config()
    };
    assert!(tree.prepare_patch(id, &change(next.clone(), 4)).is_err());
    assert_eq!(tree.stats(), stats);
    assert_eq!(tree.node(id).unwrap().descriptor, original);
    {
        let _guard = tree.prepare_patch(id, &change(next.clone(), 5)).unwrap();
    }
    assert_eq!(tree.stats(), stats);
    assert_eq!(tree.node(id).unwrap().descriptor, original);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = tree.prepare_patch(id, &change(next.clone(), 5)).unwrap();
        panic!("unwind rollback")
    }));
    assert!(caught.is_err());
    assert_eq!(tree.stats(), stats);
    assert_eq!(tree.node(id).unwrap().descriptor, original);
    assert!(
        tree.apply(&[
            Mutation::Patch {
                id,
                patch: Patch::Name(Change::Set(Some("prefix".into())))
            },
            Mutation::Patch {
                id,
                patch: change(next.clone(), 4)
            }
        ])
        .is_err()
    );
    assert_eq!(tree.stats().patched, stats.patched + 1);
    assert_eq!(
        tree.node(id).unwrap().descriptor.view().name.as_deref(),
        Some("prefix")
    );
    tree.apply(&[
        Mutation::Patch {
            id,
            patch: change(next, 5),
        },
        Mutation::Patch {
            id,
            patch: Patch::File {
                configuration: Change::Keep,
                revision: 0,
            },
        },
    ])
    .unwrap();
    let Props::File(value) = tree.node(id).unwrap().descriptor.view().props else {
        panic!()
    };
    assert_eq!(value.revision, 5);
    assert_eq!(value.configuration.max_files, 2);
    tree.apply(&[Mutation::Patch {
        id,
        patch: Patch::File {
            configuration: Change::Clear,
            revision: 6,
        },
    }])
    .unwrap();
    let Props::File(value) = tree.node(id).unwrap().descriptor.view().props else {
        panic!()
    };
    assert_eq!(value.revision, 6);
    assert_eq!(value.configuration, FileConfig::default());
    assert_eq!(value.label, "Choose file");
}
#[test]
fn file_label_and_enabled_patch_do_not_detach_configuration_and_reject_invalid_label() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[Mutation::Create {
        id,
        node: Box::new(descriptor()),
    }])
    .unwrap();
    let pointer = match tree.node(id).unwrap().descriptor.view().props {
        Props::File(value) => value.configuration.filters.as_ptr(),
        _ => panic!(),
    };
    for patch in [
        Patch::Label(Change::Set("Read a file".into())),
        Patch::Enabled(Change::Set(false)),
        Patch::Name(Change::Set(Some("Named file".into()))),
    ] {
        tree.apply(&[Mutation::Patch { id, patch }]).unwrap();
        let Props::File(value) = tree.node(id).unwrap().descriptor.view().props else {
            panic!()
        };
        assert_eq!(value.configuration.filters.as_ptr(), pointer);
        assert_eq!(value.revision, 4);
    }
    let before = tree.stats();
    assert!(
        tree.apply(&[Mutation::Patch {
            id,
            patch: Patch::Label(Change::Set("x".repeat(4097)))
        }])
        .is_err()
    );
    assert_eq!(tree.stats(), before);
    tree.apply(&[
        Mutation::Patch {
            id,
            patch: Patch::Label(Change::Clear),
        },
        Mutation::Patch {
            id,
            patch: Patch::Enabled(Change::Clear),
        },
    ])
    .unwrap();
    let Props::File(value) = tree.node(id).unwrap().descriptor.view().props else {
        panic!()
    };
    assert_eq!(value.label, "Choose file");
    assert!(value.enabled);
}
#[test]
fn raw_revision_validation_and_construction_rules_match_live_file_guards() {
    let mut value = descriptor();
    let next = FileConfig {
        max_files: 2,
        ..config()
    };
    let old = value.clone();
    assert!(value.patch(change(next.clone(), 4)).is_err());
    assert_eq!(value, old);
    value.patch(change(next, 5)).unwrap();
    assert!(
        value
            .patched(Patch::File {
                configuration: Change::Clear,
                revision: 4
            })
            .is_err()
    );
    assert_eq!(
        value
            .patched(Patch::File {
                configuration: Change::Keep,
                revision: 0
            })
            .unwrap(),
        value
    );
    let mut invalid = Descriptor::new(Props::File(FileProps {
        label: "\0".into(),
        ..Default::default()
    }));
    assert!(invalid.validate().is_err());
    invalid = descriptor();
    invalid.role = Some(Role::Group);
    assert!(invalid.validate().is_err());
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let button = ids.allocate();
    let file = ids.allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        Mutation::Create {
            id: button,
            node: Box::new(Descriptor::new(Props::Button(ButtonProps::default()))),
        },
        Mutation::Create {
            id: file,
            node: Box::new(descriptor()),
        },
    ])
    .unwrap();
    assert!(
        tree.validate_placement(file, Some(button), None, false)
            .is_err()
    );
}
