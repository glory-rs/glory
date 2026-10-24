use glory_render::{memory::Memory, *};

fn selected(
    node: NodeId,
    document: FileDocumentId,
    revision: u64,
    request: Option<u64>,
    batch: u64,
) -> FileEvent {
    FileEvent {
        document,
        revision,
        request,
        batch,
        origin: if request.is_some() {
            FileOrigin::Picker
        } else {
            FileOrigin::Drop
        },
        outcome: FileOutcome::Selected(vec![FileInfo {
            token: FileToken {
                document,
                node,
                revision,
                serial: batch,
            },
            name: "bounded.txt".into(),
            media_type: "text/plain".into(),
            size: 5,
        }]),
    }
}
fn file(event: &FileEvent) -> FileToken {
    match &event.outcome {
        FileOutcome::Selected(files) => files[0].token,
        _ => panic!(),
    }
}

#[test]
fn configuration_guard_revokes_only_on_committed_change_and_never_reuses_request_identity() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let document = FileDocumentId::new().unwrap();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        Mutation::Create {
            id,
            node: Box::new(Descriptor::new(Props::File(FileProps::default()))),
        },
        Mutation::Place {
            id,
            parent: None,
            before: None,
        },
    ])
    .unwrap();
    tree.admit_file_picker(id, 0, 1).unwrap();
    let event = selected(id, document, 0, Some(1), 1);
    assert!(tree.finish_files(id, document, &event).unwrap());
    assert!(tree.selected_file(file(&event)).is_ok());
    assert!(!tree.finish_files(id, document, &event).unwrap());
    tree.prepare_patch(
        id,
        &Patch::File {
            configuration: Change::Set(Box::default()),
            revision: 0,
        },
    )
    .unwrap()
    .commit();
    assert!(
        tree.selected_file(file(&event)).is_ok(),
        "equivalent declaration preserves current read rights"
    );
    let changed = FileConfig {
        filters: vec![FileFilter::Mime("image/png".into())],
        ..Default::default()
    };
    drop(
        tree.prepare_patch(
            id,
            &Patch::File {
                configuration: Change::Set(Box::new(changed.clone())),
                revision: 1,
            },
        )
        .unwrap(),
    );
    assert!(
        tree.selected_file(file(&event)).is_ok(),
        "rollback must not revoke current rights"
    );
    tree.prepare_patch(
        id,
        &Patch::File {
            configuration: Change::Set(Box::new(changed)),
            revision: 1,
        },
    )
    .unwrap()
    .commit();
    assert!(tree.selected_file(file(&event)).is_err());
    assert!(
        !tree.finish_files(id, document, &event).unwrap(),
        "old filter result is discarded without validating against the new filter"
    );
    assert_eq!(tree.next_file_request(id).unwrap(), 2);
    assert!(tree.admit_file_picker(id, 1, 1).is_err());
    tree.admit_file_picker(id, 1, 2).unwrap();
    tree.clear_files(id, 1).unwrap();
    assert_eq!(tree.next_file_request(id).unwrap(), 3);
}

#[test]
fn document_identity_target_lifetime_inert_disabled_and_clear_have_explicit_boundaries() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let id = ids.allocate();
    let parent = ids.allocate();
    let document = FileDocumentId::new().unwrap();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        Mutation::Create {
            id: parent,
            node: Box::new(Descriptor::new(Props::View)),
        },
        Mutation::Create {
            id,
            node: Box::new(Descriptor::new(Props::File(FileProps::default()))),
        },
        Mutation::Place {
            id: parent,
            parent: None,
            before: None,
        },
        Mutation::Place {
            id,
            parent: Some(parent),
            before: None,
        },
    ])
    .unwrap();
    tree.admit_file_picker(id, 0, 1).unwrap();
    let event = selected(id, document, 0, Some(1), 1);
    assert!(
        tree.finish_files(id, FileDocumentId::new().unwrap(), &event)
            .is_err()
    );
    assert!(tree.finish_files(id, document, &event).unwrap());
    let token = file(&event);
    tree.remove_file(token).unwrap();
    assert!(tree.selected_file(token).is_err());
    assert!(tree.remove_file(token).is_err());
    tree.prepare_patch(id, &Patch::Enabled(Change::Set(false)))
        .unwrap()
        .commit();
    assert!(tree.admit_file_picker(id, 0, 2).is_err());
    tree.prepare_patch(id, &Patch::Enabled(Change::Set(true)))
        .unwrap()
        .commit();
    tree.prepare_patch(
        parent,
        &Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            inert: true,
            ..Default::default()
        })))),
    )
    .unwrap()
    .commit();
    assert!(tree.admit_file_picker(id, 0, 2).is_err());
    tree.apply(&[Mutation::Dispose { id: parent }]).unwrap();
    assert!(tree.next_file_request(id).is_err());
    assert!(tree.finish_files(id, document, &event).is_err());
}
#[test]
fn adoption_is_fresh_lifetime_only_and_reuses_neither_picker_requests_nor_retired_read_rights() {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let document = FileDocumentId::new().unwrap();
    let mut tree = Memory::new(runtime);
    tree.apply(&[
        Mutation::Create {
            id,
            node: Box::new(Descriptor::new(Props::File(FileProps::default()))),
        },
        Mutation::Place {
            id,
            parent: None,
            before: None,
        },
    ])
    .unwrap();
    let mut event = selected(id, document, 0, None, 1);
    event.origin = FileOrigin::Adopted;
    assert!(tree.finish_files(id, document, &event).unwrap());
    assert!(tree.selected_file(file(&event)).is_ok());
    assert!(!tree.finish_files(id, document, &event).unwrap());
    tree.clear_files(id, 0).unwrap();
    assert!(!tree.finish_files(id, document, &event).unwrap());
    event.batch = 2;
    assert!(matches!(
        tree.finish_files(id, document, &event),
        Err(Error::InvalidEvent(_))
    ));
    assert!(tree.selected_file(file(&event)).is_err());
}
