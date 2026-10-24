use glory_render::{
    memory::{Memory, Subtree},
    *,
};

fn scope() -> Descriptor {
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.focus = FocusMode::Programmatic;
    descriptor.semantics = Some(Box::new(Semantics {
        focus_scope: Some(FocusScopeConfig::default()),
        ..Default::default()
    }));
    descriptor
}
fn create(tree: &mut Memory, ids: &mut NodeIds, descriptor: Descriptor) -> NodeId {
    let id = ids.allocate();
    tree.apply(&[Mutation::Create {
        id,
        node: Box::new(descriptor),
    }])
    .unwrap();
    id
}
fn place(tree: &mut Memory, id: NodeId, parent: Option<NodeId>) {
    tree.apply(&[Mutation::Place {
        id,
        parent,
        before: None,
    }])
    .unwrap();
}

#[test]
fn current_scope_counts_include_nested_suspended_and_unplaced_ranges() {
    let runtime = RuntimeId::new();
    let mut tree = Memory::new(runtime);
    let mut ids = NodeIds::new(runtime);
    let outer = create(&mut tree, &mut ids, scope());
    let inner = create(&mut tree, &mut ids, scope());
    let leaf = create(&mut tree, &mut ids, Descriptor::new(Props::View));
    assert_eq!(tree.focus_scope_nodes(outer).unwrap(), Some(1));
    place(&mut tree, leaf, Some(inner));
    place(&mut tree, inner, Some(outer));
    assert_eq!(tree.focus_scope_nodes(outer).unwrap(), Some(3));
    assert_eq!(tree.focus_scope_nodes(inner).unwrap(), Some(2));
    tree.apply(&[Mutation::Suspend { id: inner }]).unwrap();
    assert_eq!(tree.focus_scope_nodes(outer).unwrap(), Some(3));
    tree.apply(&[Mutation::Resume {
        id: inner,
        parent: None,
        before: None,
    }])
    .unwrap();
    assert_eq!(tree.focus_scope_nodes(outer).unwrap(), Some(1));
    assert_eq!(tree.focus_scope_nodes(inner).unwrap(), Some(2));
    place(&mut tree, inner, Some(outer));
    tree.apply(&[
        Mutation::Suspend { id: leaf },
        Mutation::Dispose { id: inner },
    ])
    .unwrap();
    assert_eq!(tree.focus_scopes(), 1);
    assert_eq!(tree.focus_scope_nodes(outer).unwrap(), Some(1));
    assert!(tree.node(leaf).is_err());
}

#[test]
fn declaration_budget_preflight_guard_drop_unwind_and_disposal_are_atomic() {
    let runtime = RuntimeId::new();
    let mut tree = Memory::new(runtime);
    let mut ids = NodeIds::new(runtime);
    let mut roots = Vec::new();
    for _ in 0..MAX_FOCUS_SCOPES {
        roots.push(create(&mut tree, &mut ids, scope()));
    }
    let before = (tree.stats(), tree.receipt());
    assert!(
        tree.validate_child(Subtree::New(scope().view()), None)
            .is_err()
    );
    assert!(
        tree.apply(&[Mutation::Create {
            id: ids.allocate(),
            node: Box::new(scope())
        }])
        .is_err()
    );
    assert_eq!((tree.stats(), tree.receipt()), before);
    let root = roots[0];
    drop(
        tree.prepare_patch(root, &Patch::Semantics(Change::Clear))
            .unwrap(),
    );
    assert_eq!(tree.focus_scopes(), MAX_FOCUS_SCOPES);
    tree.prepare_patch(root, &Patch::Semantics(Change::Clear))
        .unwrap()
        .commit();
    assert_eq!(tree.focus_scopes(), MAX_FOCUS_SCOPES - 1);
    let before = (tree.stats(), tree.receipt());
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = tree
            .prepare_patch(root, &Patch::Semantics(Change::Set(scope().semantics)))
            .unwrap();
        panic!("rollback");
    }));
    assert!(outcome.is_err());
    assert_eq!((tree.stats(), tree.receipt()), before);
    assert_eq!(tree.focus_scopes(), MAX_FOCUS_SCOPES - 1);
    tree.prepare_patch(root, &Patch::Semantics(Change::Set(scope().semantics)))
        .unwrap()
        .commit();
    tree.apply(&[Mutation::Dispose { id: root }]).unwrap();
    assert_eq!(tree.focus_scopes(), MAX_FOCUS_SCOPES - 1);
}

#[test]
fn restricted_subtree_limit_rejects_move_and_metadata_before_changes() {
    let runtime = RuntimeId::new();
    let mut tree = Memory::new(runtime);
    let mut ids = NodeIds::new(runtime);
    let destination = create(&mut tree, &mut ids, scope());
    let mut candidate = Descriptor::new(Props::View);
    candidate.focus = FocusMode::Programmatic;
    let large = create(&mut tree, &mut ids, candidate);
    for _ in 1..MAX_FOCUS_SCOPE_NODES {
        let child = create(&mut tree, &mut ids, Descriptor::new(Props::View));
        place(&mut tree, child, Some(large));
    }
    let before = (tree.stats(), tree.receipt());
    assert!(
        tree.validate_child(Subtree::Existing(large), Some(destination))
            .is_err()
    );
    assert!(
        tree.apply(&[Mutation::Place {
            id: large,
            parent: Some(destination),
            before: None
        }])
        .is_err()
    );
    assert_eq!((tree.stats(), tree.receipt()), before);
    assert_eq!(tree.node(large).unwrap().parent, None);
    tree.prepare_patch(large, &Patch::Semantics(Change::Set(scope().semantics)))
        .unwrap()
        .commit();
    assert_eq!(
        tree.focus_scope_nodes(large).unwrap(),
        Some(MAX_FOCUS_SCOPE_NODES)
    );
    let extra = create(&mut tree, &mut ids, Descriptor::new(Props::View));
    assert!(
        tree.validate_child(Subtree::Existing(extra), Some(large))
            .is_err()
    );
    tree.prepare_patch(large, &Patch::Semantics(Change::Clear))
        .unwrap()
        .commit();
    place(&mut tree, extra, Some(large));
    let before = (tree.stats(), tree.receipt());
    assert!(
        tree.prepare_patch(large, &Patch::Semantics(Change::Set(scope().semantics)))
            .is_err()
    );
    assert_eq!((tree.stats(), tree.receipt()), before);
    assert_eq!(tree.focus_scope_nodes(large).unwrap(), None);
}

#[test]
fn plan_nested_scope_sizes_remain_exact_when_moving_the_whole_plan() {
    let runtime = RuntimeId::new();
    let mut tree = Memory::new(runtime);
    let mut ids = NodeIds::new(runtime);
    let outer = create(&mut tree, &mut ids, scope());
    let plan = Plan::new(vec![
        PlanNode {
            parent: None,
            descriptor: Descriptor::new(Props::Group),
        },
        PlanNode {
            parent: Some(0),
            descriptor: scope(),
        },
        PlanNode {
            parent: Some(1),
            descriptor: Descriptor::new(Props::View),
        },
    ])
    .unwrap();
    let plan_ids: Vec<_> = (0..3).map(|_| ids.allocate()).collect();
    tree.apply(&[Mutation::CreatePlan {
        plan,
        ids: plan_ids.clone(),
    }])
    .unwrap();
    assert_eq!(tree.focus_scope_nodes(plan_ids[1]).unwrap(), Some(2));
    place(&mut tree, plan_ids[0], Some(outer));
    assert_eq!(tree.focus_scope_nodes(outer).unwrap(), Some(4));
    place(&mut tree, plan_ids[2], Some(outer));
    assert_eq!(tree.focus_scope_nodes(outer).unwrap(), Some(4));
    assert_eq!(tree.focus_scope_nodes(plan_ids[1]).unwrap(), Some(1));
    tree.apply(&[Mutation::Dispose { id: plan_ids[0] }])
        .unwrap();
    assert_eq!(tree.focus_scope_nodes(outer).unwrap(), Some(2));
}

#[test]
fn scope_refinement_and_rejection_preserve_successful_prefix_and_query_watermark() {
    let runtime = RuntimeId::new();
    let mut tree = Memory::new(runtime);
    let mut ids = NodeIds::new(runtime);
    let first = ids.allocate();
    let invalid = ids.allocate();
    let mut bad = scope();
    bad.focus = FocusMode::TabStop;
    assert!(bad.validate().is_err());
    assert!(
        tree.apply(&[
            Mutation::Create {
                id: first,
                node: Box::new(Descriptor::new(Props::View))
            },
            Mutation::Create {
                id: invalid,
                node: Box::new(bad)
            },
        ])
        .is_err()
    );
    assert!(tree.node(first).is_ok());
    assert!(tree.node(invalid).is_err());
    assert_eq!(tree.focus_scopes(), 0);
    let accepted = tree.receipt().applied;
    tree.request(Query::Bounds(first), QueryToken(5)).unwrap();
    let mut events = Vec::new();
    tree.drain(&mut events);
    assert!(
        matches!(events.as_slice(), [Input::Response { token: QueryToken(5), applied, .. }] if *applied == accepted)
    );
    let mut bad = scope();
    bad.props = Props::Input(Default::default());
    assert!(bad.validate().is_err());
    let before = (tree.stats(), tree.receipt());
    assert!(
        tree.prepare_patch(first, &Patch::Semantics(Change::Set(scope().semantics)))
            .is_err()
    );
    assert_eq!((tree.stats(), tree.receipt()), before);
}
