use glory_render::{
    memory::{Memory, Subtree},
    *,
};
#[test]
fn submission_checks_shared_declaration_without_admitting_controls_or_semantics() {
    let mut memory = Memory::new(RuntimeId::new());
    let batch = memory.submission().unwrap();
    for (props, expected) in [
        (Props::View, true),
        (Props::Text(Default::default()), true),
        (Props::Button(Default::default()), true),
        (Props::Input(Default::default()), false),
        (Props::Group, false),
    ] {
        let plan = Plan::new(vec![node(None, props)]).unwrap();
        for declaration in [plan.clone(), plan] {
            assert_eq!(
                batch.supports(&Mutation::CreatePlan {
                    plan: declaration,
                    ids: vec![]
                }),
                expected
            );
        }
    }
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.semantics = Some(Default::default());
    let plan = Plan::new(vec![PlanNode {
        parent: None,
        descriptor,
    }])
    .unwrap();
    assert!(!batch.supports(&Mutation::CreatePlan { plan, ids: vec![] }));
}

fn node(parent: Option<usize>, props: Props) -> PlanNode {
    PlanNode {
        parent,
        descriptor: Descriptor::new(props),
    }
}
fn declaration() -> Plan {
    Plan::new(vec![
        node(None, Props::View),
        node(
            Some(0),
            Props::Text(TextProps {
                content: "seed".into(),
                ..Default::default()
            }),
        ),
    ])
    .unwrap()
}

#[test]
fn planned_siblings_preserve_order_after_moves_and_new_instances() {
    let plan = Plan::new(vec![
        node(None, Props::View),
        node(Some(0), Props::View),
        node(Some(0), Props::View),
        node(Some(1), Props::View),
        node(Some(2), Props::View),
        node(Some(1), Props::View),
    ])
    .unwrap();
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let a: Vec<_> = (0..6).map(|_| ids.allocate()).collect();
    tree.apply(&[
        Mutation::CreatePlan {
            plan: plan.clone(),
            ids: a.clone(),
        },
        Mutation::Place {
            id: a[0],
            parent: None,
            before: None,
        },
    ])
    .unwrap();
    assert_eq!(tree.node(a[0]).unwrap().children.as_ref(), &[a[1], a[2]]);
    assert_eq!(tree.node(a[1]).unwrap().children.as_ref(), &[a[3], a[5]]);
    tree.apply(&[
        Mutation::Place {
            id: a[5],
            parent: Some(a[2]),
            before: Some(a[4]),
        },
        Mutation::Dispose { id: a[3] },
        Mutation::Place {
            id: a[4],
            parent: Some(a[2]),
            before: Some(a[5]),
        },
    ])
    .unwrap();
    assert!(tree.node(a[1]).unwrap().children.is_empty());
    assert_eq!(tree.node(a[2]).unwrap().children.as_ref(), &[a[4], a[5]]);
    let b: Vec<_> = (0..6).map(|_| ids.allocate()).collect();
    tree.apply(&[
        Mutation::CreatePlan {
            plan,
            ids: b.clone(),
        },
        Mutation::Place {
            id: b[0],
            parent: None,
            before: None,
        },
    ])
    .unwrap();
    assert_eq!(tree.node(b[1]).unwrap().children.as_ref(), &[b[3], b[5]]);
    assert_eq!(tree.node(b[2]).unwrap().children.as_ref(), &[b[4]]);
    tree.apply(&[Mutation::Dispose { id: a[0] }]).unwrap();
    assert_eq!(tree.node(b[1]).unwrap().children.as_ref(), &[b[3], b[5]]);
    assert_eq!(tree.len(), 6);
}

#[test]
fn plan_instances_keep_independent_descriptors_and_generational_subtrees() {
    let runtime = RuntimeId::new();
    let mut allocator = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let plan = declaration();
    let first = vec![allocator.allocate(), allocator.allocate()];
    let second = vec![allocator.allocate(), allocator.allocate()];
    for ids in [&first, &second] {
        tree.apply(&[
            Mutation::CreatePlan {
                plan: plan.clone(),
                ids: ids.clone(),
            },
            Mutation::Place {
                id: ids[0],
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    }
    assert_eq!(tree.apply(&[]).unwrap().applied, 4);
    tree.apply(&[Mutation::Patch {
        id: first[1],
        patch: Patch::Text(Change::Set("changed".into())),
    }])
    .unwrap();
    assert!(
        matches!(&tree.node(second[1]).unwrap().descriptor.view().props,Props::Text(v)if v.content=="seed")
    );
    assert!(matches!(&plan.nodes()[1].descriptor.view().props,Props::Text(v)if v.content=="seed"));
    tree.apply(&[
        Mutation::Suspend { id: first[0] },
        Mutation::Dispose { id: first[0] },
    ])
    .unwrap();
    assert!(tree.node(first[1]).is_err());
    assert!(tree.active(second[1]));
    assert_eq!(tree.stats().created, 4);
    assert_eq!(tree.stats().disposed, 2);
}

#[test]
fn a_late_invalid_plan_identity_rejects_the_unit_and_keeps_the_accepted_prefix() {
    let runtime = RuntimeId::new();
    let mut allocator = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let plan = declaration();
    let prefix = allocator.allocate();
    let unused = allocator.allocate();
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    let error = tree.apply(&[
        Mutation::Create {
            id: prefix,
            node: Box::new(Descriptor::new(Props::View)),
        },
        Mutation::CreatePlan {
            plan: plan.clone(),
            ids: vec![unused, foreign],
        },
    ]);
    assert_eq!(error, Err(Error::ForeignRuntime));
    assert_eq!(tree.apply(&[]).unwrap().applied, 1);
    assert_eq!(tree.len(), 1);
    assert!(tree.node(unused).is_err());
    let stats = tree.stats();
    for ids in [vec![unused, unused], vec![unused, prefix], vec![unused]] {
        assert!(
            tree.apply(&[Mutation::CreatePlan {
                plan: plan.clone(),
                ids
            }])
            .is_err()
        );
        assert_eq!(tree.stats(), stats);
        assert_eq!(tree.apply(&[]).unwrap().applied, 1);
    }
    tree.apply(&[Mutation::CreatePlan {
        plan,
        ids: vec![unused, allocator.allocate()],
    }])
    .unwrap();
    assert_eq!(tree.len(), 3);
}

#[test]
fn plan_identity_batch_checks_cover_both_orders_sparse_fallback_and_reused_generations() {
    let plan = Plan::new(vec![
        node(None, Props::View),
        node(Some(0), Props::View),
        node(Some(0), Props::View),
    ])
    .unwrap();
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let low = ids.allocate();
    for _ in 0..300 {
        ids.allocate();
    }
    let middle = ids.allocate();
    let high = ids.allocate();
    for batch in [
        [low, middle, high],
        [high, middle, low],
        [middle, low, high],
    ] {
        let mut tree = Memory::new(runtime);
        tree.apply(&[Mutation::CreatePlan {
            plan: plan.clone(),
            ids: batch.to_vec(),
        }])
        .unwrap();
        assert_eq!(tree.node(batch[0]).unwrap().children.as_ref(), &batch[1..]);
    }
    assert!(ids.release(middle));
    let reused = ids.allocate();
    assert_eq!(reused.slot(), middle.slot());
    assert_ne!(reused.generation(), middle.generation());
    for batch in [
        [low, middle, middle],
        [middle, low, middle],
        [middle, reused, high],
    ] {
        let mut tree = Memory::new(runtime);
        assert_eq!(
            tree.apply(&[Mutation::CreatePlan {
                plan: plan.clone(),
                ids: batch.to_vec()
            }]),
            Err(Error::InvalidPlacement("duplicate plan slot"))
        );
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.receipt().applied, 0);
    }
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    let mut tree = Memory::new(runtime);
    assert_eq!(
        tree.apply(&[Mutation::CreatePlan {
            plan,
            ids: vec![high, foreign, low]
        }]),
        Err(Error::ForeignRuntime)
    );
    assert!(tree.is_empty());
}

#[test]
fn construction_validates_the_closed_tree_and_placement_checks_the_entire_new_subtree() {
    assert!(Plan::new(vec![]).is_err());
    assert!(
        Plan::new(
            (0..=MAX_PLAN_NODES)
                .map(|i| node((i > 0).then_some(0), Props::View))
                .collect()
        )
        .is_err()
    );
    assert!(Plan::new(vec![node(None, Props::View), node(Some(2), Props::View)]).is_err());
    assert!(
        Plan::new(vec![
            node(None, Props::Input(InputProps::default())),
            node(Some(0), Props::View)
        ])
        .is_err()
    );
    assert!(
        Plan::new(vec![
            node(None, Props::Button(ButtonProps::default())),
            node(Some(0), Props::Group),
            node(Some(1), Props::Input(InputProps::default()))
        ])
        .is_err()
    );
    let plan = Plan::new(vec![node(None, Props::Group), node(Some(0), Props::View)]).unwrap();
    let runtime = RuntimeId::new();
    let button = NodeIds::new(runtime).allocate();
    let mut tree = Memory::new(runtime);
    tree.apply(&[Mutation::Create {
        id: button,
        node: Box::new(Descriptor::new(Props::Button(ButtonProps::default()))),
    }])
    .unwrap();
    assert!(
        tree.validate_child(Subtree::Plan(&plan), Some(button))
            .is_err()
    );
}

#[test]
fn declaration_boundaries_still_use_the_current_boxed_document_parent() {
    let part = |part| {
        let mut d = Descriptor::new(Props::View);
        d.semantics = Some(Box::new(Semantics {
            document_part: Some(part),
            ..Default::default()
        }));
        d
    };
    let plan = Plan::new(vec![
        node(None, Props::Group),
        node(Some(0), Props::Group),
        PlanNode {
            parent: Some(1),
            descriptor: part(DocumentPart::TableRow),
        },
        PlanNode {
            parent: Some(2),
            descriptor: part(DocumentPart::TableCell),
        },
    ])
    .unwrap();
    let runtime = RuntimeId::new();
    let mut tree = Memory::new(runtime);
    let mut ids = NodeIds::new(runtime);
    let body = ids.allocate();
    let group = ids.allocate();
    let ordinary = ids.allocate();
    tree.apply(&[
        Mutation::Create {
            id: body,
            node: Box::new(part(DocumentPart::TableBody)),
        },
        Mutation::Create {
            id: group,
            node: Box::new(Descriptor::new(Props::Group)),
        },
        Mutation::Place {
            id: group,
            parent: Some(body),
            before: None,
        },
        Mutation::Create {
            id: ordinary,
            node: Box::new(Descriptor::new(Props::View)),
        },
    ])
    .unwrap();
    for _ in 0..2 {
        assert!(
            tree.validate_child(Subtree::Plan(&plan), Some(group))
                .is_ok()
        );
        assert!(
            tree.validate_child(Subtree::Plan(&plan), Some(ordinary))
                .is_err()
        );
    }
    // The root still crosses two transparent declaration nodes at attachment.
    let instance: Vec<_> = (0..4).map(|_| ids.allocate()).collect();
    tree.apply(&[
        Mutation::CreatePlan {
            plan,
            ids: instance.clone(),
        },
        Mutation::Place {
            id: instance[0],
            parent: Some(group),
            before: None,
        },
    ])
    .unwrap();
    assert_eq!(tree.node(instance[2]).unwrap().parent, Some(instance[1]));
}

#[test]
fn shared_focus_declarations_keep_instance_overrides_and_memory_budgets_independent() {
    let focus = || {
        let mut d = Descriptor::new(Props::View);
        d.focus = FocusMode::Programmatic;
        d.semantics = Some(Box::new(Semantics {
            focus_scope: Some(FocusScopeConfig::default()),
            ..Default::default()
        }));
        d
    };
    let plan = Plan::new(vec![
        PlanNode {
            parent: None,
            descriptor: focus(),
        },
        PlanNode {
            parent: Some(0),
            descriptor: focus(),
        },
        node(Some(1), Props::View),
    ])
    .unwrap();
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut first = Memory::new(runtime);
    let mut second = Memory::new(runtime);
    let a: Vec<_> = (0..3).map(|_| ids.allocate()).collect();
    first
        .apply(&[Mutation::CreatePlan {
            plan: plan.clone(),
            ids: a.clone(),
        }])
        .unwrap();
    assert_eq!(first.focus_scope_nodes(a[0]).unwrap(), Some(3));
    assert_eq!(first.focus_scope_nodes(a[1]).unwrap(), Some(2));
    first
        .prepare_patch(a[1], &Patch::Semantics(Change::Clear))
        .unwrap()
        .commit();
    assert_eq!(first.focus_scopes(), 1);
    let b: Vec<_> = (0..3).map(|_| ids.allocate()).collect();
    second
        .apply(&[Mutation::CreatePlan {
            plan: plan.clone(),
            ids: b.clone(),
        }])
        .unwrap();
    assert_eq!(second.focus_scopes(), 2);
    assert_eq!(second.focus_scope_nodes(b[1]).unwrap(), Some(2));
    for _ in 0..MAX_FOCUS_SCOPES - 3 {
        let id = ids.allocate();
        second
            .apply(&[Mutation::Create {
                id,
                node: Box::new(focus()),
            }])
            .unwrap();
    }
    let before = (second.stats(), second.receipt(), second.len());
    assert!(second.validate_child(Subtree::Plan(&plan), None).is_err());
    let rejected: Vec<_> = (0..3).map(|_| ids.allocate()).collect();
    assert!(
        second
            .apply(&[Mutation::CreatePlan {
                plan,
                ids: rejected.clone()
            }])
            .is_err()
    );
    assert_eq!((second.stats(), second.receipt(), second.len()), before);
    assert!(rejected.iter().all(|id| second.node(*id).is_err()));
    assert_eq!(first.focus_scope_nodes(a[0]).unwrap(), Some(3));
}
