use glory_render::{memory::Memory, *};
use std::collections::HashMap;

fn create(tree: &mut Memory, id: NodeId) {
    tree.apply(&[Mutation::Create {
        id,
        node: Box::new(Descriptor::new(Props::View)),
    }])
    .unwrap();
}

fn move_oracle(
    orders: &mut HashMap<Option<NodeId>, Vec<NodeId>>,
    parents: &mut HashMap<NodeId, Option<NodeId>>,
    id: NodeId,
    parent: Option<NodeId>,
    before: Option<NodeId>,
) {
    if let Some(old) = parents.insert(id, parent) {
        let siblings = orders.get_mut(&old).unwrap();
        siblings.remove(siblings.iter().position(|&other| other == id).unwrap());
    }
    let siblings = orders.entry(parent).or_default();
    let index = before.map_or(siblings.len(), |before| {
        siblings.iter().position(|&other| other == before).unwrap()
    });
    siblings.insert(index, id);
}

fn check(tree: &Memory, orders: &HashMap<Option<NodeId>, Vec<NodeId>>) {
    for (&parent, expected) in orders {
        let actual = parent.map_or_else(
            || tree.roots(),
            |parent| tree.node(parent).unwrap().children.as_ref(),
        );
        assert_eq!(actual, expected);
        for (index, &id) in expected.iter().enumerate() {
            assert_eq!(
                tree.next_sibling(id).unwrap(),
                expected.get(index + 1).copied()
            );
        }
    }
}

#[test]
fn wide_order_matches_vector_across_reparent_suspend_prefix_rejection_and_reused_ids() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let left = ids.allocate();
    let right = ids.allocate();
    let mut orders = HashMap::<_, Vec<_>>::new();
    let mut parents = HashMap::new();
    for id in [left, right] {
        create(&mut tree, id);
        tree.apply(&[Mutation::Place {
            id,
            parent: None,
            before: None,
        }])
        .unwrap();
        move_oracle(&mut orders, &mut parents, id, None, None);
    }
    let rows: Vec<_> = (0..6_000).map(|_| ids.allocate()).collect();
    for (index, &id) in rows.iter().enumerate() {
        create(&mut tree, id);
        let parent = Some(if index % 2 == 0 { left } else { right });
        tree.apply(&[Mutation::Place {
            id,
            parent,
            before: None,
        }])
        .unwrap();
        move_oracle(&mut orders, &mut parents, id, parent, None);
    }
    let mut seed = 29_u64;
    for turn in 0..240 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = rows[seed as usize % rows.len()];
        let parent = match turn % 3 {
            0 => None,
            1 => Some(left),
            _ => Some(right),
        };
        let before = orders
            .entry(parent)
            .or_default()
            .iter()
            .rev()
            .copied()
            .find(|&other| other != id && other != left && other != right);
        if turn % 7 == 0 {
            tree.apply(&[Mutation::Suspend { id }]).unwrap();
            // Suspension keeps logical ordering; only Resume moves the range.
            check(&tree, &orders);
            tree.apply(&[Mutation::Resume { id, parent, before }])
                .unwrap();
        } else {
            tree.apply(&[Mutation::Place { id, parent, before }])
                .unwrap();
        }
        move_oracle(&mut orders, &mut parents, id, parent, before);
        check(&tree, &orders);
    }
    let snapshot = tree.node(left).unwrap().children.clone();
    let before = orders[&Some(left)]
        .first()
        .copied()
        .filter(|&id| id != rows[1]);
    let applied = tree.receipt().applied;
    assert!(
        tree.apply(&[
            Mutation::Place {
                id: rows[1],
                parent: Some(left),
                before
            },
            Mutation::Place {
                id: rows[2],
                parent: Some(rows[2]),
                before: None
            },
            Mutation::Dispose { id: rows[3] },
        ])
        .is_err()
    );
    move_oracle(&mut orders, &mut parents, rows[1], Some(left), before);
    assert_eq!(tree.receipt().applied, applied + 1);
    assert!(tree.node(rows[3]).is_ok());
    assert_ne!(
        tree.node(left).unwrap().children.as_ref(),
        snapshot.as_ref()
    );
    check(&tree, &orders);
    for &id in rows.iter().step_by(31) {
        tree.apply(&[Mutation::Dispose { id }]).unwrap();
        let siblings = orders.get_mut(&parents.remove(&id).unwrap()).unwrap();
        siblings.remove(siblings.iter().position(|&other| other == id).unwrap());
        ids.release(id);
    }
    let replacement = ids.allocate();
    create(&mut tree, replacement);
    tree.apply(&[Mutation::Place {
        id: replacement,
        parent: Some(left),
        before: None,
    }])
    .unwrap();
    move_oracle(&mut orders, &mut parents, replacement, Some(left), None);
    for &id in rows.iter().step_by(31) {
        let receipt = tree.receipt();
        assert!(
            tree.apply(&[Mutation::Place {
                id,
                parent: Some(left),
                before: None
            }])
            .is_err()
        );
        assert_eq!(tree.receipt(), receipt);
    }
    check(&tree, &orders);
}
