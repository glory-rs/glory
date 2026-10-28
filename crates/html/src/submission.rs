//! Pure bounded preparation, with explicit host-prefix confirmation.
use super::*;
use std::rc::Rc;

pub enum PreparedOp {
    Plan {
        plan: glory_render::Plan,
        ids: Vec<NodeId>,
        keys: Vec<u64>,
        shape: Rc<PlanShape>,
    },
    Op(Op),
}
enum Keys {
    Create(Vec<NodeId>),
    Dispose(Vec<(NodeId, u64)>),
    None,
}
pub struct Submission<'a> {
    tree: glory_render::memory::Submission<'a>,
    keys: &'a mut HashMap<NodeId, u64>,
    next_key: &'a mut u64,
    receipt: &'a mut Receipt,
    plans: &'a mut plan::Plans,
    undo: Vec<(u64, Keys)>,
}
impl Mapper {
    pub fn submission(&mut self) -> Result<Submission<'_>, Error> {
        Ok(Submission {
            tree: self.tree.submission()?,
            keys: &mut self.keys,
            next_key: &mut self.next_key,
            receipt: &mut self.receipt,
            plans: &mut self.plans,
            undo: Vec::new(),
        })
    }
}
impl Submission<'_> {
    pub fn tree(&self) -> &Memory {
        self.tree.tree()
    }
    pub fn len(&self) -> usize {
        self.undo.len()
    }
    pub fn is_empty(&self) -> bool {
        self.undo.is_empty()
    }
    pub fn supports(&self, mutation: &Mutation) -> bool {
        self.tree.supports(mutation)
    }
    pub fn prepare(&mut self, mutation: &Mutation) -> Result<PreparedOp, Error> {
        if !self.supports(mutation) {
            return Err(Error::Unsupported("submission barrier"));
        }
        let old_key = *self.next_key;
        let (op, undo) = match mutation {
            Mutation::CreatePlan { plan, ids } => {
                preflight_place(self.tree.tree(), Subtree::Plan(plan), None)?;
                let shape = self.plans.shape(plan)?;
                let end = self
                    .next_key
                    .checked_add(ids.len() as u64)
                    .ok_or(Error::InvalidProps("HTML key space exhausted"))?;
                self.tree.apply(mutation)?;
                let keys: Vec<_> = (*self.next_key..end).collect();
                for (id, key) in ids.iter().zip(&keys) {
                    self.keys.insert(*id, *key);
                }
                *self.next_key = end;
                (
                    PreparedOp::Plan {
                        plan: plan.clone(),
                        ids: ids.clone(),
                        keys,
                        shape,
                    },
                    Keys::Create(ids.clone()),
                )
            }
            Mutation::Patch { id, patch } => {
                preflight(self.tree.tree().node(*id)?.descriptor.view(), Some(patch))?;
                let edits = self.tree.patch(*id, patch, patch::lower)?;
                (PreparedOp::Op(Op::Patch { id: *id, edits }), Keys::None)
            }
            Mutation::Place { id, parent, before } => {
                self.tree
                    .tree()
                    .validate_placement(*id, *parent, *before, false)?;
                preflight_place(self.tree.tree(), Subtree::Existing(*id), *parent)?;
                self.tree.apply(mutation)?;
                let siblings = if let Some(parent) = parent {
                    &self.tree.tree().node(*parent)?.children
                } else {
                    self.tree.tree().roots()
                };
                let before = before
                    .and_then(|before| siblings.iter().position(|id| *id == before))
                    .and_then(|index| {
                        siblings[index..]
                            .iter()
                            .copied()
                            .find(|id| !self.tree.tree().node(*id).expect("sibling").suspended)
                    });
                (
                    PreparedOp::Op(Op::Place {
                        id: *id,
                        parent: *parent,
                        before,
                    }),
                    Keys::None,
                )
            }
            Mutation::Dispose { id } => {
                let subtree = self.tree.apply(mutation)?.expect("retirement");
                let keys = subtree
                    .iter()
                    .map(|id| (*id, self.keys.remove(id).expect("live key")))
                    .collect();
                (
                    PreparedOp::Op(Op::Dispose { id: *id, subtree }),
                    Keys::Dispose(keys),
                )
            }
            _ => return Err(Error::Unsupported("submission operation")),
        };
        self.undo.push((old_key, undo));
        Ok(op)
    }
    fn rollback_keys(&mut self, accepted: usize) {
        while self.undo.len() > accepted {
            let (next_key, keys) = self.undo.pop().expect("suffix keys");
            match keys {
                Keys::Create(ids) => {
                    for id in ids {
                        self.keys.remove(&id);
                    }
                }
                Keys::Dispose(keys) => self.keys.extend(keys),
                Keys::None => {}
            }
            *self.next_key = next_key;
        }
    }
    pub fn discard_last(&mut self) {
        self.tree.discard_last();
        self.rollback_keys(self.undo.len().saturating_sub(1));
    }
    pub fn finish(mut self, accepted: usize) -> Receipt {
        assert!(accepted <= self.undo.len());
        self.rollback_keys(accepted);
        self.undo.clear();
        // Move the tree guard out while leaving this object's key-drop inert.
        let receipt = self.tree.confirm(accepted);
        *self.receipt = receipt;
        receipt
    }
}
impl Drop for Submission<'_> {
    fn drop(&mut self) {
        self.rollback_keys(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glory_render::{Change, Descriptor, NodeIds, Patch, Plan, PlanNode, Props, RuntimeId};
    fn scenario(runtime: RuntimeId) -> Vec<Mutation> {
        let mut ids = NodeIds::new(runtime);
        let first = ids.allocate();
        let second = ids.allocate();
        let plan = Plan::new(vec![PlanNode {
            parent: None,
            descriptor: Descriptor::new(Props::Text(Default::default())),
        }])
        .unwrap();
        vec![
            Mutation::CreatePlan {
                plan: plan.clone(),
                ids: vec![first],
            },
            Mutation::Place {
                id: first,
                parent: None,
                before: None,
            },
            Mutation::Patch {
                id: first,
                patch: Patch::Text(Change::Set("中文🙂".into())),
            },
            Mutation::Patch {
                id: first,
                patch: Patch::Classes(Change::Set(vec!["one".into()])),
            },
            Mutation::CreatePlan {
                plan,
                ids: vec![second],
            },
            Mutation::Place {
                id: second,
                parent: None,
                before: Some(first),
            },
            Mutation::Patch {
                id: first,
                patch: Patch::Text(Change::Set(String::new())),
            },
            Mutation::Dispose { id: first },
            Mutation::Dispose { id: second },
        ]
    }
    struct Null;
    impl Sink for Null {
        fn apply(&mut self, _: Op) -> Result<(), Error> {
            Ok(())
        }
    }
    #[test]
    fn every_host_prefix_matches_sequential_semantics_keys_and_generation_reuse() {
        let runtime = RuntimeId::new();
        let ops = scenario(runtime);
        for accepted in 0..=ops.len() {
            let mut expected = Mapper::new(runtime);
            expected.apply(&ops[..accepted], &mut Null).unwrap();
            let mut actual = Mapper::new(runtime);
            let mut batch = actual.submission().unwrap();
            for op in &ops {
                batch.prepare(op).unwrap();
            }
            batch.finish(accepted);
            assert_eq!(actual.html().unwrap(), expected.html().unwrap());
            assert_eq!(actual.tree.stats(), expected.tree.stats());
            assert_eq!(actual.tree.receipt(), expected.tree.receipt());
            assert_eq!(actual.receipt(), expected.receipt());
            assert_eq!(actual.keys, expected.keys);
            assert_eq!(actual.next_key, expected.next_key);
            // The unconfirmed suffix must not consume generation watermarks.
            actual.apply(&ops[accepted..], &mut Null).unwrap();
            assert!(actual.tree.is_empty());
        }
    }
    #[test]
    fn dropped_unwound_and_discarded_preparation_restores_fields_and_topology() {
        let runtime = RuntimeId::new();
        let ops = scenario(runtime);
        let mut mapper = Mapper::new(runtime);
        mapper.apply(&ops[..2], &mut Null).unwrap();
        let before = mapper.html().unwrap();
        let stats = mapper.tree.stats();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut batch = mapper.submission().unwrap();
            for op in &ops[2..] {
                batch.prepare(op).unwrap();
            }
            batch.discard_last();
            panic!("host cancelled");
        }));
        assert!(result.is_err());
        assert_eq!(mapper.html().unwrap(), before);
        assert_eq!(mapper.tree.stats(), stats);
        assert_eq!(mapper.tree.receipt().applied, 2);
        assert_eq!(mapper.next_key, 1);
    }

    #[test]
    fn rollback_restores_metadata_sharing_and_retired_slot_generations() {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let first = ids.allocate();
        let plan = Plan::new(vec![PlanNode {
            parent: None,
            descriptor: Descriptor::new(Props::Text(Default::default())),
        }])
        .unwrap();
        let create = Mutation::CreatePlan {
            plan: plan.clone(),
            ids: vec![first],
        };
        let mut mapper = Mapper::new(runtime);
        mapper
            .apply(std::slice::from_ref(&create), &mut Null)
            .unwrap();
        {
            let mut batch = mapper.submission().unwrap();
            batch
                .prepare(&Mutation::Patch {
                    id: first,
                    patch: Patch::Classes(Change::Set(vec!["detached".into()])),
                })
                .unwrap();
        }
        assert!(std::ptr::eq(
            mapper.tree.node(first).unwrap().descriptor.view().classes,
            &plan.nodes()[0].descriptor.classes
        ));
        ids.release(first);
        let next = ids.allocate();
        assert_eq!(first.slot(), next.slot());
        let operations = [
            Mutation::Dispose { id: first },
            Mutation::CreatePlan {
                plan,
                ids: vec![next],
            },
        ];
        for accepted in 0..=2 {
            let mut mapper = Mapper::new(runtime);
            mapper
                .apply(std::slice::from_ref(&create), &mut Null)
                .unwrap();
            let mut batch = mapper.submission().unwrap();
            for op in &operations {
                batch.prepare(op).unwrap();
            }
            batch.finish(accepted);
            if accepted == 0 {
                assert!(mapper.tree.node(first).is_ok());
            }
            if accepted == 1 {
                assert!(mapper.tree.is_empty());
                mapper
                    .apply(std::slice::from_ref(&create), &mut Null)
                    .unwrap_err();
            }
            if accepted == 2 {
                assert!(mapper.tree.node(next).is_ok());
                assert!(mapper.tree.node(first).is_err());
            }
        }
    }

    #[test]
    fn late_semantic_rejection_preserves_prepared_prefix_without_exposing_suffix() {
        let runtime = RuntimeId::new();
        let ops = scenario(runtime);
        let mut mapper = Mapper::new(runtime);
        let mut batch = mapper.submission().unwrap();
        for op in &ops[..4] {
            batch.prepare(op).unwrap();
        }
        assert!(batch.prepare(&ops[0]).is_err());
        batch.finish(3);
        assert_eq!(mapper.receipt().applied, 3);
        assert_eq!(mapper.tree.stats().patched, 1);
        assert!(!mapper.html().unwrap().contains("class=\"one\""));
    }

    #[test]
    fn prefix_rollback_restores_cross_instance_children_and_sibling_links() {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let a: Vec<_> = (0..3).map(|_| ids.allocate()).collect();
        let b: Vec<_> = (0..3).map(|_| ids.allocate()).collect();
        let plan = Plan::new(vec![
            PlanNode {
                parent: None,
                descriptor: Descriptor::new(Props::View),
            },
            PlanNode {
                parent: Some(0),
                descriptor: Descriptor::new(Props::Text(Default::default())),
            },
            PlanNode {
                parent: Some(0),
                descriptor: Descriptor::new(Props::Text(Default::default())),
            },
        ])
        .unwrap();
        let ops = [
            Mutation::CreatePlan {
                plan: plan.clone(),
                ids: a.clone(),
            },
            Mutation::Place {
                id: a[0],
                parent: None,
                before: None,
            },
            Mutation::CreatePlan {
                plan,
                ids: b.clone(),
            },
            Mutation::Place {
                id: b[0],
                parent: None,
                before: Some(a[0]),
            },
            Mutation::Place {
                id: a[1],
                parent: Some(b[0]),
                before: Some(b[1]),
            },
            Mutation::Place {
                id: b[1],
                parent: Some(a[0]),
                before: Some(a[2]),
            },
            Mutation::Dispose { id: a[0] },
            Mutation::Dispose { id: b[0] },
        ];
        for accepted in 0..=ops.len() {
            let mut expected = Mapper::new(runtime);
            expected.apply(&ops[..accepted], &mut Null).unwrap();
            let mut actual = Mapper::new(runtime);
            let mut batch = actual.submission().unwrap();
            for op in &ops {
                batch.prepare(op).unwrap();
            }
            batch.finish(accepted);
            assert_eq!(actual.html().unwrap(), expected.html().unwrap());
            assert_eq!(actual.tree.stats(), expected.tree.stats());
            for id in a.iter().chain(&b) {
                match (actual.tree.node(*id), expected.tree.node(*id)) {
                    (Ok(a), Ok(b)) => {
                        assert_eq!(
                            (a.parent, a.placed, a.suspended),
                            (b.parent, b.placed, b.suspended)
                        );
                        assert_eq!(a.children.as_ref(), b.children.as_ref());
                    }
                    (Err(_), Err(_)) => {}
                    _ => panic!("prefix identity mismatch"),
                }
            }
        }
    }
}
