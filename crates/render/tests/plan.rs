use glory_render::{
    memory::{Memory, Subtree},
    *,
};
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
        matches!(&tree.node(second[1]).unwrap().descriptor.props,Props::Text(v)if v.content=="seed")
    );
    assert!(matches!(&plan.nodes()[1].descriptor.props,Props::Text(v)if v.content=="seed"));
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
