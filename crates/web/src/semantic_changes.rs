//! One borrowed view of the batch's final committed metadata. This is not a
//! node/visit cache: it lives only through one adapter update and stores no
//! owned business payloads. Live feature membership remains owned by each driver.
use glory_render::{Mutation, NodeId, Patch, Semantics, memory::Memory};
use std::collections::BTreeMap;

pub(crate) type Changes<'a> = BTreeMap<NodeId, Option<&'a Semantics>>;

/// Resolve only metadata-bearing creations and explicit metadata patches.
/// Read the accepted tree, not payloads from a rejected suffix. Keep None for
/// Clear so existing controllers can retire. Retired generations are omitted.
pub(crate) fn collect<'a>(tree: &'a Memory, mutations: &[Mutation]) -> Changes<'a> {
    let mut changed = BTreeMap::new();
    let mut record = |id| {
        if let Ok(node) = tree.node(id) {
            changed.insert(id, node.descriptor.view().semantics.as_deref());
        }
    };
    for mutation in mutations {
        match mutation {
            Mutation::Create { id, node } if node.semantics.is_some() => record(*id),
            Mutation::CreatePlan { plan, ids } => {
                for &index in plan.semantic_nodes() {
                    // A rejected suffix may contain a malformed identity list.
                    if let Some(&id) = ids.get(index) {
                        record(id);
                    }
                }
            }
            Mutation::Patch {
                id,
                patch: Patch::Semantics(_),
            } => record(*id),
            _ => {}
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use glory_render::*;

    fn descriptor(metadata: bool) -> Descriptor {
        let mut node = Descriptor::new(Props::View);
        if metadata {
            node.semantics = Some(Box::new(Semantics {
                busy: Some(true),
                ..Default::default()
            }));
        }
        node
    }
    fn legacy<'a>(tree: &'a Memory, mutations: &[Mutation]) -> Changes<'a> {
        mutations
            .iter()
            .flat_map(|mutation| match mutation {
                Mutation::Create { id, .. }
                | Mutation::Patch {
                    id,
                    patch: Patch::Semantics(_),
                } => std::slice::from_ref(id),
                Mutation::CreatePlan { ids, .. } => ids.as_slice(),
                _ => &[],
            })
            .filter_map(|id| {
                tree.node(*id)
                    .ok()
                    .map(|node| (*id, node.descriptor.view().semantics.as_deref()))
            })
            .collect()
    }
    fn replay(
        tree: &Memory,
        mut live: BTreeMap<NodeId, Semantics>,
        changes: Changes<'_>,
    ) -> BTreeMap<NodeId, Semantics> {
        for (id, value) in changes {
            if let Some(value) = value {
                live.insert(id, value.clone());
            } else {
                live.remove(&id);
            }
        }
        live.retain(|id, _| tree.node(*id).is_ok());
        live
    }
    #[test]
    fn ordinary_plans_do_not_build_a_per_instance_metadata_inventory() {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let plan = Plan::new(
            (0..MAX_PLAN_NODES)
                .map(|index| PlanNode {
                    parent: (index != 0).then_some(0),
                    descriptor: descriptor(false),
                })
                .collect(),
        )
        .unwrap();
        assert!(plan.semantic_nodes().is_empty());
        let commands: Vec<_> = (0..64)
            .map(|_| Mutation::CreatePlan {
                plan: plan.clone(),
                ids: plan.nodes().iter().map(|_| ids.allocate()).collect(),
            })
            .collect();
        let mut tree = Memory::new(runtime);
        tree.apply(&commands).unwrap();
        assert_eq!(legacy(&tree, &commands).len(), 64 * MAX_PLAN_NODES);
        assert!(collect(&tree, &commands).is_empty());
    }
    #[test]
    fn final_borrowed_metadata_and_clear_match_the_old_driver_replay() {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let plan = Plan::new(vec![
            PlanNode {
                parent: None,
                descriptor: descriptor(false),
            },
            PlanNode {
                parent: Some(0),
                descriptor: descriptor(true),
            },
        ])
        .unwrap();
        assert_eq!(plan.semantic_nodes(), &[1]);
        let (root, child, ordinary, retired) = (
            ids.allocate(),
            ids.allocate(),
            ids.allocate(),
            ids.allocate(),
        );
        let mut tree = Memory::new(runtime);
        let commands = vec![
            Mutation::CreatePlan {
                plan,
                ids: vec![root, child],
            },
            Mutation::Patch {
                id: root,
                patch: Patch::Semantics(Change::Set(descriptor(true).semantics)),
            },
            Mutation::Patch {
                id: child,
                patch: Patch::Semantics(Change::Clear),
            },
            Mutation::Create {
                id: ordinary,
                node: Box::new(descriptor(false)),
            },
            Mutation::Create {
                id: retired,
                node: Box::new(descriptor(true)),
            },
            Mutation::Dispose { id: retired },
            Mutation::Patch {
                id: root,
                patch: Patch::Semantics(Change::Keep),
            },
        ];
        tree.apply(&commands).unwrap();
        let changed = collect(&tree, &commands);
        assert_eq!(changed.len(), 2);
        assert!(changed[&child].is_none());
        assert!(std::ptr::eq(
            changed[&root].unwrap(),
            tree.node(root)
                .unwrap()
                .descriptor
                .view()
                .semantics
                .as_deref()
                .unwrap()
        ));
        let live = BTreeMap::from([
            (child, Semantics::default()),
            (retired, Semantics::default()),
        ]);
        assert_eq!(
            replay(&tree, live.clone(), legacy(&tree, &commands)),
            replay(&tree, live, changed)
        );
    }
    #[test]
    fn rejected_suffix_and_recycled_slots_read_only_the_accepted_generation() {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let old = ids.allocate();
        let mut tree = Memory::new(runtime);
        tree.apply(&[
            Mutation::Create {
                id: old,
                node: Box::new(descriptor(true)),
            },
            Mutation::Dispose { id: old },
        ])
        .unwrap();
        ids.release(old);
        let current = ids.allocate();
        let plan = Plan::new(vec![PlanNode {
            parent: None,
            descriptor: descriptor(true),
        }])
        .unwrap();
        let commands = vec![
            Mutation::Create {
                id: current,
                node: Box::new(descriptor(true)),
            },
            Mutation::CreatePlan { plan, ids: vec![] },
            Mutation::Patch {
                id: old,
                patch: Patch::Semantics(Change::Clear),
            },
        ];
        assert!(tree.apply(&commands).is_err());
        let receipt = tree.receipt();
        let changed = collect(&tree, &commands);
        assert_eq!(changed.len(), 1);
        assert!(changed[&current].is_some());
        assert!(!changed.contains_key(&old));
        assert_eq!(tree.receipt(), receipt);
        assert_eq!(
            replay(&tree, BTreeMap::new(), changed),
            replay(&tree, BTreeMap::new(), legacy(&tree, &commands))
        );
    }
}
