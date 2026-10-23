use glory_render::{
    memory::{Memory, Subtree},
    *,
};

fn document(part: DocumentPart) -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.semantics = Some(Box::new(Semantics {
        document_part: Some(part),
        ..Default::default()
    }));
    d
}
fn create(tree: &mut Memory, ids: &mut NodeIds, d: Descriptor) -> NodeId {
    let id = ids.allocate();
    tree.apply(&[Mutation::Create {
        id,
        node: Box::new(d),
    }])
    .unwrap();
    id
}
fn place(tree: &mut Memory, id: NodeId, parent: Option<NodeId>) -> Result<Receipt, Error> {
    tree.apply(&[Mutation::Place {
        id,
        parent,
        before: None,
    }])
}

#[test]
fn document_metadata_rejects_control_role_focus_and_form_impersonation() {
    for part in [
        DocumentPart::Blockquote,
        DocumentPart::OrderedList,
        DocumentPart::UnorderedList,
        DocumentPart::ListItem,
        DocumentPart::DescriptionList,
        DocumentPart::Term,
        DocumentPart::Definition,
    ] {
        document(part).validate().unwrap();
        for props in [
            Props::Button(Default::default()),
            Props::Text(Default::default()),
            Props::Input(Default::default()),
            Props::Group,
        ] {
            let mut d = document(part);
            d.props = props;
            assert!(d.validate().is_err());
        }
        let mut d = document(part);
        d.role = Some(Role::Group);
        assert!(d.validate().is_err());
        let mut d = document(part);
        d.focus = FocusMode::TabStop;
        assert!(d.validate().is_err());
        let mut d = document(part);
        d.semantics.as_mut().unwrap().field_part = Some(FieldPart::Form);
        assert!(d.validate().is_err());
        let mut d = document(part);
        d.name = Some("Document".into());
        assert_eq!(d.effective_role(), None);
    }
}

#[test]
fn live_document_part_changes_rollback_without_stats_or_watermark_changes() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let id = create(&mut tree, &mut ids, document(DocumentPart::Blockquote));
    place(&mut tree, id, None).unwrap();
    let old = tree.node(id).unwrap().descriptor.clone();
    let stats = tree.stats();
    let receipt = tree.receipt();
    for metadata in [
        None,
        Some(Box::new(Semantics {
            document_part: Some(DocumentPart::OrderedList),
            ..Default::default()
        })),
    ] {
        assert!(
            tree.prepare_patch(id, &Patch::Semantics(Change::Set(metadata)))
                .is_err()
        );
        assert_eq!(tree.node(id).unwrap().descriptor, old);
        assert_eq!(tree.stats(), stats);
        assert_eq!(tree.receipt(), receipt);
    }
    tree.apply(&[Mutation::Patch {
        id,
        patch: Patch::Name(Change::Set(Some("Quote".into()))),
    }])
    .unwrap();
    assert_eq!(tree.node(id).unwrap().descriptor.effective_role(), None);
}

#[test]
fn list_and_description_placement_share_transparent_group_rules_and_reject_prefix_safely() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let list = create(&mut tree, &mut ids, document(DocumentPart::OrderedList));
    place(&mut tree, list, None).unwrap();
    let dl = create(&mut tree, &mut ids, document(DocumentPart::DescriptionList));
    place(&mut tree, dl, None).unwrap();
    let group = create(&mut tree, &mut ids, Descriptor::new(Props::Group));
    place(&mut tree, group, Some(list)).unwrap();
    let item = create(&mut tree, &mut ids, document(DocumentPart::ListItem));
    place(&mut tree, item, Some(group)).unwrap();
    let term = create(&mut tree, &mut ids, document(DocumentPart::Term));
    place(&mut tree, term, Some(dl)).unwrap();
    let value = create(&mut tree, &mut ids, document(DocumentPart::Definition));
    place(&mut tree, value, Some(dl)).unwrap();
    let ordinary = create(&mut tree, &mut ids, Descriptor::new(Props::View));
    place(&mut tree, ordinary, None).unwrap();
    for (id, parent) in [
        (ordinary, Some(list)),
        (ordinary, Some(dl)),
        (item, None),
        (term, Some(list)),
        (value, Some(ordinary)),
        (group, Some(dl)),
    ] {
        let stats = tree.stats();
        let receipt = tree.receipt();
        let previous = tree.node(id).unwrap().parent;
        assert!(tree.validate_child(Subtree::Existing(id), parent).is_err());
        assert!(place(&mut tree, id, parent).is_err());
        assert_eq!(tree.stats(), stats);
        assert_eq!(tree.receipt(), receipt);
        assert_eq!(tree.node(id).unwrap().parent, previous);
    }
    tree.apply(&[Mutation::Suspend { id: group }]).unwrap();
    let receipt = tree.receipt();
    assert!(
        tree.apply(&[Mutation::Resume {
            id: group,
            parent: Some(dl),
            before: None
        }])
        .is_err()
    );
    assert_eq!(tree.receipt(), receipt);
    assert!(tree.node(group).unwrap().suspended);
    tree.apply(&[Mutation::Resume {
        id: group,
        parent: Some(list),
        before: None,
    }])
    .unwrap();
}

#[test]
fn plan_document_rules_validate_physical_frontier_before_live_placement() {
    let node = |parent, descriptor| PlanNode { parent, descriptor };
    assert!(
        Plan::new(vec![
            node(None, document(DocumentPart::OrderedList)),
            node(Some(0), Descriptor::new(Props::View))
        ])
        .is_err()
    );
    assert!(
        Plan::new(vec![
            node(None, Descriptor::new(Props::View)),
            node(Some(0), document(DocumentPart::ListItem))
        ])
        .is_err()
    );
    let plan = Plan::new(vec![
        node(None, Descriptor::new(Props::Group)),
        node(Some(0), document(DocumentPart::ListItem)),
    ])
    .unwrap();
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let list = create(&mut tree, &mut ids, document(DocumentPart::UnorderedList));
    place(&mut tree, list, None).unwrap();
    assert!(tree.validate_child(Subtree::Plan(&plan), None).is_err());
    tree.validate_child(Subtree::Plan(&plan), Some(list))
        .unwrap();
    let plan_ids = vec![ids.allocate(), ids.allocate()];
    tree.apply(&[
        Mutation::CreatePlan {
            plan: plan.clone(),
            ids: plan_ids.clone(),
        },
        Mutation::Place {
            id: plan_ids[0],
            parent: Some(list),
            before: None,
        },
    ])
    .unwrap();
    assert_eq!(tree.node(plan_ids[1]).unwrap().parent, Some(plan_ids[0]));
    assert!(Plan::new(vec![node(Some(999), Descriptor::new(Props::View))]).is_err());
}

#[test]
fn inline_document_text_can_live_in_native_buttons_without_interactive_children() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let button = create(
        &mut tree,
        &mut ids,
        Descriptor::new(Props::Button(Default::default())),
    );
    place(&mut tree, button, None).unwrap();
    for kind in [TextKind::Code, TextKind::Kbd, TextKind::Mark] {
        let text = create(
            &mut tree,
            &mut ids,
            Descriptor::new(Props::Text(TextProps {
                kind,
                content: "key".into(),
                ..Default::default()
            })),
        );
        place(&mut tree, text, Some(button)).unwrap();
    }
    let quote = create(&mut tree, &mut ids, document(DocumentPart::Blockquote));
    assert!(place(&mut tree, quote, Some(button)).is_err());
}
