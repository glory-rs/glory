use glory_render::{memory::Memory, *};
use std::rc::Rc;
fn document(text: &str) -> Rc<RichDocument> {
    Rc::new(
        RichDocument::new(vec![RichBlock {
            kind: RichBlockKind::Paragraph,
            runs: vec![RichRun {
                text: text.into(),
                marks: RichMarks::default(),
            }],
        }])
        .unwrap(),
    )
}
fn descriptor() -> Descriptor {
    Descriptor::new(Props::RichEditor(RichEditorProps {
        document: document("a😀b"),
        revision: 3,
        ..Default::default()
    }))
}
fn patch(text: &str, revision: u64) -> Patch {
    Patch::RichDocument {
        document: Change::Set(document(text)),
        revision,
    }
}
fn setup() -> (Memory, NodeId) {
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
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
    (tree, id)
}
fn snapshot(revision: u64) -> Box<RichSnapshot> {
    Box::new(RichSnapshot {
        document: document("new😀"),
        selection: RichSelection::collapsed(RichPoint {
            block: 0,
            offset: 5,
        }),
        revision,
        composing: false,
    })
}
#[test]
fn rich_live_guard_rolls_back_canonical_document_and_revision_without_watermark_change() {
    let (mut tree, id) = setup();
    let stats = tree.stats();
    let receipt = tree.receipt();
    for update in [patch("different", 3), patch("older", 2)] {
        assert!(tree.prepare_patch(id, &update).is_err());
    }
    drop(tree.prepare_patch(id, &patch("accepted later", 4)).unwrap());
    assert_eq!(tree.stats(), stats);
    assert_eq!(tree.receipt(), receipt);
    assert!(
        matches!(tree.node(id).unwrap().descriptor.view().props,Props::RichEditor(p) if p.revision==3&&p.document==document("a😀b"))
    );
    tree.apply(&[Mutation::Patch {
        id,
        patch: patch("next", 4),
    }])
    .unwrap();
    assert!(
        tree.prepare_patch(
            id,
            &Patch::RichDocument {
                document: Change::Clear,
                revision: 4
            }
        )
        .is_err()
    );
    tree.apply(&[Mutation::Patch {
        id,
        patch: Patch::RichDocument {
            document: Change::Clear,
            revision: 5,
        },
    }])
    .unwrap();
    assert!(
        matches!(tree.node(id).unwrap().descriptor.view().props,Props::RichEditor(p) if p.revision==5&&*p.document==RichDocument::default())
    );
    let receipt = tree.receipt();
    tree.apply(&[Mutation::Patch {
        id,
        patch: Patch::RichDocument {
            document: Change::Keep,
            revision: 0,
        },
    }])
    .unwrap();
    assert!(tree.receipt().applied > receipt.applied);
    assert!(
        matches!(tree.node(id).unwrap().descriptor.view().props,Props::RichEditor(p) if p.revision==5)
    );
}
#[test]
fn rich_events_validate_finite_selection_and_old_future_readonly_inert_rules() {
    let (mut tree, id) = setup();
    assert!(
        tree.validate_event(id, &Event::RichEdit(snapshot(3)))
            .unwrap()
    );
    assert!(
        !tree
            .validate_event(id, &Event::RichEdit(snapshot(2)))
            .unwrap()
    );
    assert!(matches!(
        tree.validate_event(id, &Event::RichEdit(snapshot(4))),
        Err(Error::InputRevision { .. })
    ));
    let mut invalid = snapshot(3);
    invalid.selection.focus.offset = 4; // middle of the final surrogate pair
    assert!(tree.validate_event(id, &Event::RichEdit(invalid)).is_err());
    tree.apply(&[Mutation::Patch {
        id,
        patch: Patch::Semantics(Change::Set(Some(Box::new(Semantics {
            readonly: true,
            ..Default::default()
        })))),
    }])
    .unwrap();
    assert!(
        tree.validate_event(id, &Event::RichEdit(snapshot(3)))
            .is_err()
    );
    assert!(
        tree.validate_event(id, &Event::RichSelection(snapshot(3)))
            .unwrap()
    );
    let mut ids = NodeIds::new(id.runtime());
    ids.allocate();
    let parent = ids.allocate();
    let mut inert = Descriptor::new(Props::View);
    inert.semantics = Some(Box::new(Semantics {
        inert: true,
        ..Default::default()
    }));
    tree.apply(&[
        Mutation::Create {
            id: parent,
            node: Box::new(inert),
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
    assert!(
        !tree
            .validate_event(id, &Event::RichSelection(snapshot(3)))
            .unwrap()
    );
    assert!(
        EventKind::RichEdit
            .validate_target(&Descriptor::new(Props::Input(Default::default())))
            .is_err()
    );
}
#[test]
fn rich_selection_action_query_and_context_are_intrinsic_and_versioned() {
    let (mut tree, id) = setup();
    let node = tree.node(id).unwrap().descriptor.view();
    Query::RichSelection(id).validate_target(node).unwrap();
    Action::SelectRich {
        node: id,
        revision: 3,
        selection: RichSelection {
            anchor: RichPoint {
                block: 0,
                offset: 4,
            },
            focus: RichPoint {
                block: 0,
                offset: 1,
            },
        },
    }
    .validate_target(node)
    .unwrap();
    for (revision, offset) in [(2, 1), (4, 1), (3, 2), (3, 5)] {
        assert!(
            Action::SelectRich {
                node: id,
                revision,
                selection: RichSelection::collapsed(RichPoint { block: 0, offset })
            }
            .validate_target(node)
            .is_err()
        );
    }
    assert!(
        Plan::new(vec![
            PlanNode {
                parent: None,
                descriptor: Descriptor::new(Props::Button(Default::default()))
            },
            PlanNode {
                parent: Some(0),
                descriptor: descriptor()
            }
        ])
        .is_err()
    );
    tree.apply(&[Mutation::Suspend { id }]).unwrap();
    assert!(
        !tree
            .validate_event(id, &Event::RichEdit(snapshot(3)))
            .unwrap()
    );
}
