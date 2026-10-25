use glory_render::{
    memory::{MAX_LAYERS, Memory, Subtree},
    *,
};

fn descriptor(index: usize) -> Descriptor {
    let mut node = Descriptor::new(Props::View);
    node.role = Some(Role::Dialog);
    node.name = Some("Bounded layer".into());
    node.focus = FocusMode::Programmatic;
    node.semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new(format!("layer-{index}")).unwrap()),
        hidden: true,
        layer: Some(Layer::default()),
        ..Default::default()
    }));
    node
}

#[test]
fn plan_layer_admission_uses_each_memorys_current_budget_after_retirement() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut full = Memory::new(runtime);
    let mut empty = Memory::new(runtime);
    let mut last = None;
    for index in 0..MAX_LAYERS - 1 {
        let id = ids.allocate();
        full.apply(&[Mutation::Create {
            id,
            node: Box::new(descriptor(index)),
        }])
        .unwrap();
        last = Some(id);
    }
    let plan = Plan::new(vec![
        PlanNode {
            parent: None,
            descriptor: Descriptor::new(Props::Group),
        },
        PlanNode {
            parent: Some(0),
            descriptor: descriptor(MAX_LAYERS),
        },
        PlanNode {
            parent: Some(0),
            descriptor: descriptor(MAX_LAYERS + 1),
        },
    ])
    .unwrap();
    let instance: Vec<_> = (0..3).map(|_| ids.allocate()).collect();
    let before = (full.stats(), full.receipt(), full.len());
    assert!(full.validate_child(Subtree::Plan(&plan), None).is_err());
    assert!(
        full.apply(&[Mutation::CreatePlan {
            plan: plan.clone(),
            ids: instance.clone()
        }])
        .is_err()
    );
    assert_eq!((full.stats(), full.receipt(), full.len()), before);
    empty.validate_child(Subtree::Plan(&plan), None).unwrap();
    empty
        .apply(&[Mutation::CreatePlan {
            plan: plan.clone(),
            ids: (0..3).map(|_| ids.allocate()).collect(),
        }])
        .unwrap();
    assert_eq!(empty.layers(), 2);
    full.apply(&[Mutation::Dispose { id: last.unwrap() }])
        .unwrap();
    full.validate_child(Subtree::Plan(&plan), None).unwrap();
    full.apply(&[Mutation::CreatePlan {
        plan,
        ids: instance,
    }])
    .unwrap();
    assert_eq!(full.layers(), MAX_LAYERS);
}

#[test]
fn layer_budget_is_live_transactional_and_shared_with_placement_preflight() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let mut live = Vec::new();
    for index in 0..MAX_LAYERS {
        let id = ids.allocate();
        tree.apply(&[Mutation::Create {
            id,
            node: Box::new(descriptor(index)),
        }])
        .unwrap();
        live.push(id);
    }
    let candidate = descriptor(MAX_LAYERS);
    let before = (tree.stats(), tree.receipt());
    assert!(
        tree.validate_child(Subtree::New(candidate.view()), None)
            .is_err()
    );
    assert!(
        tree.apply(&[Mutation::Create {
            id: ids.allocate(),
            node: Box::new(candidate.clone())
        }])
        .is_err()
    );
    assert_eq!((tree.stats(), tree.receipt()), before);
    assert_eq!(tree.layers(), MAX_LAYERS);
    let ordinary = ids.allocate();
    tree.apply(&[Mutation::Create {
        id: ordinary,
        node: Box::new(Descriptor::new(Props::View)),
    }])
    .unwrap();
    tree.apply(&[
        Mutation::Patch {
            id: ordinary,
            patch: Patch::Role(Change::Set(Some(Role::Dialog))),
        },
        Mutation::Patch {
            id: ordinary,
            patch: Patch::Name(Change::Set(Some("Candidate".into()))),
        },
        Mutation::Patch {
            id: ordinary,
            patch: Patch::Focus(Change::Set(FocusMode::Programmatic)),
        },
    ])
    .unwrap();
    assert!(
        tree.prepare_patch(
            ordinary,
            &Patch::Semantics(Change::Set(candidate.semantics.clone()))
        )
        .is_err()
    );
    assert!(
        tree.node(ordinary)
            .unwrap()
            .descriptor
            .view()
            .semantics
            .is_none()
    );
    let old = live.pop().unwrap();
    drop(
        tree.prepare_patch(old, &Patch::Semantics(Change::Clear))
            .unwrap(),
    );
    assert_eq!(tree.layers(), MAX_LAYERS);
    tree.prepare_patch(old, &Patch::Semantics(Change::Clear))
        .unwrap()
        .commit();
    assert_eq!(tree.layers(), MAX_LAYERS - 1);
    let plan = Plan::new(vec![
        PlanNode {
            parent: None,
            descriptor: Descriptor::new(Props::Group),
        },
        PlanNode {
            parent: Some(0),
            descriptor: descriptor(MAX_LAYERS + 2),
        },
        PlanNode {
            parent: Some(0),
            descriptor: descriptor(MAX_LAYERS + 3),
        },
    ])
    .unwrap();
    let plan_ids: Vec<_> = (0..3).map(|_| ids.allocate()).collect();
    let before = (tree.stats(), tree.receipt(), tree.len());
    assert!(tree.validate_child(Subtree::Plan(&plan), None).is_err());
    assert!(
        tree.apply(&[Mutation::CreatePlan {
            plan: plan.clone(),
            ids: plan_ids.clone()
        }])
        .is_err()
    );
    assert_eq!((tree.stats(), tree.receipt(), tree.len()), before);
    tree.apply(&[Mutation::Dispose {
        id: live.pop().unwrap(),
    }])
    .unwrap();
    tree.validate_child(Subtree::Plan(&plan), None).unwrap();
    tree.apply(&[Mutation::CreatePlan {
        plan,
        ids: plan_ids.clone(),
    }])
    .unwrap();
    assert_eq!(tree.layers(), MAX_LAYERS);
    let group = plan_ids[0];
    tree.apply(&[
        Mutation::Place {
            id: group,
            parent: None,
            before: None,
        },
        Mutation::Suspend { id: group },
    ])
    .unwrap();
    assert_eq!(
        tree.layers(),
        MAX_LAYERS,
        "suspension does not release a live facet"
    );
    tree.apply(&[
        Mutation::Resume {
            id: group,
            parent: None,
            before: None,
        },
        Mutation::Dispose { id: group },
    ])
    .unwrap();
    assert_eq!(tree.layers(), MAX_LAYERS - 2);
    let before = tree.receipt();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = tree
            .prepare_patch(
                ordinary,
                &Patch::Semantics(Change::Set(candidate.semantics.clone())),
            )
            .unwrap();
        panic!("rollback");
    }));
    assert!(unwind.is_err());
    assert_eq!(tree.layers(), MAX_LAYERS - 2);
    assert_eq!(tree.receipt(), before);
    tree.prepare_patch(
        ordinary,
        &Patch::Semantics(Change::Set(candidate.semantics)),
    )
    .unwrap()
    .commit();
    assert_eq!(tree.layers(), MAX_LAYERS - 1);
    tree.apply(&[Mutation::Dispose { id: ordinary }]).unwrap();
    assert_eq!(tree.layers(), MAX_LAYERS - 2);
    tree.validate_child(Subtree::New(descriptor(MAX_LAYERS + 1).view()), None)
        .unwrap();
}
