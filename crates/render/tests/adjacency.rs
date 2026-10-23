use glory_render::memory::Memory;
use glory_render::*;

fn check(tree: &Memory, parent: Option<NodeId>) {
    let children = parent.map_or_else(|| tree.roots(), |id| &tree.node(id).unwrap().children);
    for (index, id) in children.iter().enumerate() {
        assert_eq!(
            tree.next_sibling(*id).unwrap(),
            children.get(index + 1).copied()
        );
    }
}

#[test]
fn adjacency_survives_moves_suspension_rejection_removal_and_identity_reuse() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let nodes: Vec<_> = (0..12).map(|_| ids.allocate()).collect();
    for id in &nodes {
        tree.apply(&[Mutation::Create {
            id: *id,
            node: Box::new(Descriptor::new(Props::View)),
        }])
        .unwrap();
        assert_eq!(tree.next_sibling(*id).unwrap(), None);
        tree.apply(&[Mutation::Place {
            id: *id,
            parent: None,
            before: None,
        }])
        .unwrap();
        check(&tree, None);
    }
    // Move first/middle/last nodes, within and between parents. Check the
    // adjacency against the ordered vector after every accepted operation.
    for turn in 0..200 {
        let id = nodes[2 + turn % 10];
        let parent = (turn % 3 != 0).then_some(nodes[turn % 2]);
        let children = parent.map_or_else(|| tree.roots(), |id| &tree.node(id).unwrap().children);
        let before = children.iter().copied().find(|other| *other != id);
        tree.apply(&[Mutation::Place { id, parent, before }])
            .unwrap();
        for parent in [None, Some(nodes[0]), Some(nodes[1])] {
            check(&tree, parent);
        }
        if turn % 7 == 0 {
            tree.apply(&[Mutation::Suspend { id }]).unwrap();
            for parent in [None, Some(nodes[0]), Some(nodes[1])] {
                check(&tree, parent);
            }
            assert!(
                tree.apply(&[Mutation::Resume {
                    id,
                    parent: Some(id),
                    before: None
                }])
                .is_err()
            );
            tree.apply(&[Mutation::Resume {
                id,
                parent: None,
                before: Some(nodes[0]),
            }])
            .unwrap();
        }
    }
    for id in nodes.iter().skip(2) {
        tree.apply(&[Mutation::Dispose { id: *id }]).unwrap();
        assert!(tree.next_sibling(*id).is_err());
        ids.release(*id);
        for parent in [None, Some(nodes[0]), Some(nodes[1])] {
            check(&tree, parent);
        }
    }
    let replacement = ids.allocate();
    assert!(nodes.iter().any(|id| id.slot() == replacement.slot()));
    tree.apply(&[
        Mutation::Create {
            id: replacement,
            node: Box::new(Descriptor::new(Props::View)),
        },
        Mutation::Place {
            id: replacement,
            parent: None,
            before: Some(nodes[0]),
        },
    ])
    .unwrap();
    check(&tree, None);
    assert_eq!(tree.next_sibling(replacement).unwrap(), Some(nodes[0]));
}

#[test]
fn front_retirement_compaction_preserves_live_order_and_rejects_retired_generations() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let parent = ids.allocate();
    let children: Vec<_> = (0..1_024).map(|_| ids.allocate()).collect();
    for id in std::iter::once(&parent).chain(&children) {
        tree.apply(&[
            Mutation::Create {
                id: *id,
                node: Box::new(Descriptor::new(Props::View)),
            },
            Mutation::Place {
                id: *id,
                parent: (*id != parent).then_some(parent),
                before: None,
            },
        ])
        .unwrap();
    }
    for (index, id) in children.iter().take(800).enumerate() {
        tree.apply(&[Mutation::Dispose { id: *id }]).unwrap();
        ids.release(*id);
        assert_eq!(tree.node(parent).unwrap().children, children[index + 1..]);
        if index % 64 == 0 {
            check(&tree, Some(parent));
        }
    }
    let snapshot = tree.node(parent).unwrap().children.clone();
    let fresh = ids.allocate();
    tree.apply(&[
        Mutation::Create {
            id: fresh,
            node: Box::new(Descriptor::new(Props::View)),
        },
        Mutation::Place {
            id: fresh,
            parent: Some(parent),
            before: snapshot.first().copied(),
        },
    ])
    .unwrap();
    assert_eq!(snapshot, children[800..]);
    for id in children.iter().take(800) {
        assert!(tree.apply(&[Mutation::Dispose { id: *id }]).is_err());
    }
    assert_eq!(tree.node(parent).unwrap().children[0], fresh);
    check(&tree, Some(parent));
    tree.apply(&[Mutation::Dispose { id: parent }]).unwrap();
    assert!(tree.is_empty());
    assert!(tree.roots().is_empty());
}
