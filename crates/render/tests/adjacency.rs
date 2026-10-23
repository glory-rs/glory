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
