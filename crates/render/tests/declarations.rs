use glory_render::memory::Memory;
use glory_render::*;
use std::{cell::Cell, rc::Rc};

fn declaration() -> Plan {
    let mut root = Descriptor::new(Props::View);
    root.classes = vec!["static".into()];
    root.style = style::Style::new().width(style::Length::Px(120.));
    Plan::new(vec![
        PlanNode {
            parent: None,
            descriptor: root,
        },
        PlanNode {
            parent: Some(0),
            descriptor: Descriptor::new(Props::Input(InputProps {
                value: "seed".into(),
                ..Default::default()
            })),
        },
    ])
    .unwrap()
}
fn create(tree: &mut Memory, plan: &Plan, ids: &[NodeId]) {
    tree.apply(&[Mutation::CreatePlan {
        plan: plan.clone(),
        ids: ids.to_vec(),
    }])
    .unwrap();
}

#[test]
fn memories_share_declaration_fields_but_never_editable_values_or_overrides() {
    let runtime = RuntimeId::new();
    let mut allocator = NodeIds::new(runtime);
    let ids = [allocator.allocate(), allocator.allocate()];
    let plan = declaration();
    let mut staged = Memory::new(runtime);
    let mut committed = Memory::new(runtime);
    create(&mut staged, &plan, &ids);
    create(&mut committed, &plan, &ids);
    let left = staged.node(ids[0]).unwrap().descriptor.view();
    let right = committed.node(ids[0]).unwrap().descriptor.view();
    assert!(std::ptr::eq(left.style, right.style));
    assert!(std::ptr::eq(left.style, &plan.nodes()[0].descriptor.style));
    assert!(std::ptr::eq(left.classes, right.classes));
    assert!(std::ptr::eq(left.options, right.options));
    let input = |tree: &Memory| match tree.node(ids[1]).unwrap().descriptor.view().props {
        Props::Input(p) => (p.value.clone(), p.revision, p.value.as_ptr()),
        _ => panic!(),
    };
    assert_ne!(input(&staged).2, input(&committed).2);
    staged
        .apply(&[Mutation::Patch {
            id: ids[1],
            patch: Patch::Value {
                value: Change::Set("changed".into()),
                revision: 1,
            },
        }])
        .unwrap();
    assert_eq!((input(&staged).0, input(&staged).1), ("changed".into(), 1));
    assert_eq!(
        (input(&committed).0, input(&committed).1),
        ("seed".into(), 0)
    );
    staged
        .apply(&[Mutation::Patch {
            id: ids[0],
            patch: Patch::Classes(Change::Set(vec!["edited".into()])),
        }])
        .unwrap();
    let left = staged.node(ids[0]).unwrap().descriptor.view();
    let right = committed.node(ids[0]).unwrap().descriptor.view();
    assert_eq!(left.classes, &["edited"]);
    assert_eq!(right.classes, &["static"]);
    assert!(!std::ptr::eq(left.classes, right.classes));
    assert!(std::ptr::eq(left.style, right.style));
    assert!(std::ptr::eq(left.options, right.options));
}

#[test]
fn rejected_dropped_and_unwound_patches_rejoin_shared_fields_without_advancing_receipts() {
    let runtime = RuntimeId::new();
    let mut allocator = NodeIds::new(runtime);
    let ids = [allocator.allocate(), allocator.allocate()];
    let plan = declaration();
    let mut tree = Memory::new(runtime);
    create(&mut tree, &plan, &ids);
    let receipt = tree.apply(&[]).unwrap();
    let stats = tree.stats();
    assert!(
        tree.prepare_patch(
            ids[0],
            &Patch::Classes(Change::Set(vec!["bad token".into()]))
        )
        .is_err()
    );
    assert!(
        tree.prepare_patch(ids[0], &Patch::Role(Change::Set(Some(Role::Separator))))
            .is_err()
    );
    {
        let guard = tree
            .prepare_patch(
                ids[0],
                &Patch::Classes(Change::Set(vec!["temporary".into()])),
            )
            .unwrap();
        assert_eq!(guard.descriptor().classes, &["temporary"]);
    }
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = tree
                .prepare_patch(
                    ids[0],
                    &Patch::Style(Change::Set(Box::new(
                        style::Style::new().width(style::Length::Px(190.)),
                    ))),
                )
                .unwrap();
            panic!("exercise rollback");
        }))
        .is_err()
    );
    let view = tree.node(ids[0]).unwrap().descriptor.view();
    assert!(std::ptr::eq(view.style, &plan.nodes()[0].descriptor.style));
    assert!(std::ptr::eq(
        view.classes,
        &plan.nodes()[0].descriptor.classes
    ));
    assert_eq!(tree.stats(), stats);
    assert_eq!(tree.apply(&[]).unwrap(), receipt);
    tree.apply(&[Mutation::Patch {
        id: ids[0],
        patch: Patch::Classes(Change::Set(vec!["kept".into()])),
    }])
    .unwrap();
    let classes = tree
        .node(ids[0])
        .unwrap()
        .descriptor
        .view()
        .classes
        .as_ptr();
    drop(
        tree.prepare_patch(
            ids[0],
            &Patch::Classes(Change::Set(vec!["rolled-back".into()])),
        )
        .unwrap(),
    );
    assert_eq!(
        tree.node(ids[0])
            .unwrap()
            .descriptor
            .view()
            .classes
            .as_ptr(),
        classes
    );
    assert_eq!(
        tree.node(ids[0]).unwrap().descriptor.view().classes,
        &["kept"]
    );
}

#[test]
fn declarations_release_options_after_the_last_memory_without_retaining_instance_values() {
    struct DropCount(Rc<Cell<usize>>);
    impl Drop for DropCount {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    let mut descriptor = Descriptor::new(Props::View);
    descriptor.options.insert(DropCount(drops.clone()));
    let plan = Plan::new(vec![PlanNode {
        parent: None,
        descriptor,
    }])
    .unwrap();
    let weak = plan.downgrade();
    let runtime = RuntimeId::new();
    let id = NodeIds::new(runtime).allocate();
    let mut staged = Memory::new(runtime);
    let mut committed = Memory::new(runtime);
    create(&mut staged, &plan, &[id]);
    create(&mut committed, &plan, &[id]);
    drop(plan);
    staged.apply(&[Mutation::Dispose { id }]).unwrap();
    assert_eq!(drops.get(), 0);
    assert!(weak.upgrade().is_some());
    committed.apply(&[Mutation::Dispose { id }]).unwrap();
    assert_eq!(drops.get(), 1);
    assert!(weak.upgrade().is_none());
    assert_eq!(staged.len(), 0);
    assert_eq!(committed.len(), 0);
}
