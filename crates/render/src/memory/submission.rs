//! Bounded speculative host preparation. Only touched fields and moved/retired
//! nodes are retained; no model/descriptor snapshot or application callback.
use super::*;

const LIMIT: usize = 256;
const NODE_LIMIT: usize = 4096;

#[derive(Clone, Copy)]
struct Link {
    parent: Option<NodeId>,
    next: Option<NodeId>,
    placed: bool,
    suspended: bool,
}
impl Link {
    fn read(node: &Node) -> Self {
        Self {
            parent: node.parent,
            next: node.next,
            placed: node.placed,
            suspended: node.suspended,
        }
    }
    fn restore(self, tree: &mut Memory, id: NodeId) {
        tree.unlink(id);
        let node = tree.nodes.get_mut(&id).expect("submission node");
        node.placed = false;
        node.suspended = false;
        node.parent = self.parent;
        node.next = self.next;
        if self.placed {
            tree.place(id, self.parent, self.next, false)
                .expect("previous placement");
        }
        tree.nodes.get_mut(&id).expect("submission node").suspended = self.suspended;
    }
}
enum Undo {
    Create(Vec<(NodeId, Option<(u32, bool)>)>),
    Patch {
        id: NodeId,
        inverse: Patch,
        fields: u8,
    },
    Place(NodeId, Link),
    Dispose {
        root: NodeId,
        link: Link,
        nodes: Vec<(NodeId, Node)>,
    },
}
struct Record {
    undo: Undo,
    receipt: Receipt,
    stats: Stats,
    weight: usize,
}

/// A finite preparation segment for stateless Plan construction/fields/retirement.
/// Unsupported operations form a host barrier. Drop restores all unconfirmed
/// operations; `finish` keeps exactly the successful semantic prefix.
pub struct Submission<'a> {
    tree: &'a mut Memory,
    records: Vec<Record>,
    weight: usize,
}
impl Memory {
    pub fn submission(&mut self) -> Result<Submission<'_>, Error> {
        if !self.inputs.is_empty() || self.layers != 0 || !self.focus_scopes.is_empty() {
            return Err(Error::Unsupported(
                "submission requires no pending reference input or scopes",
            ));
        }
        Ok(Submission {
            tree: self,
            records: Vec::new(),
            weight: 0,
        })
    }
}
impl Submission<'_> {
    pub fn tree(&self) -> &Memory {
        self.tree
    }
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    pub fn discard_last(&mut self) {
        self.rollback(self.records.len().saturating_sub(1));
    }
    pub fn supports(&self, mutation: &Mutation) -> bool {
        if self.records.len() == LIMIT || self.weight == NODE_LIMIT {
            return false;
        }
        match mutation {
            Mutation::CreatePlan { plan, .. } => {
                plan.nodes().len() <= NODE_LIMIT - self.weight
                    && plan.nodes().iter().all(|n| {
                        n.descriptor.semantics.is_none()
                            && matches!(
                                n.descriptor.props,
                                Props::View | Props::Text(_) | Props::Button(_) | Props::Link(_)
                            )
                    })
            }
            Mutation::Patch { patch, .. } => {
                matches!(patch, Patch::Text(_) | Patch::Label(_) | Patch::Classes(_))
            }
            Mutation::Place { .. } => true,
            Mutation::Dispose { .. } => true,
            _ => false,
        }
    }
    /// The mapper lowers the prepared field while the canonical guard is borrowed.
    pub fn patch<T>(
        &mut self,
        id: NodeId,
        patch: &Patch,
        lower: impl FnOnce(DescriptorView<'_>, &Patch) -> T,
    ) -> Result<T, Error> {
        if self.records.len() == LIMIT
            || self.weight == NODE_LIMIT
            || !matches!(patch, Patch::Text(_) | Patch::Label(_) | Patch::Classes(_))
        {
            return Err(Error::Unsupported("submission field"));
        }
        let receipt = self.tree.receipt;
        let stats = self.tree.stats;
        let mut prepared = self.tree.prepare_patch(id, patch)?;
        let value = lower(prepared.descriptor(), prepared.previous());
        // These three fields have no registry side effects in accept(). Move
        // the already-owned inverse into the journal; never copy the old body.
        let inverse = prepared.inverse.take().expect("prepared field inverse");
        let fields = prepared.fields_before;
        prepared.commit();
        self.records.push(Record {
            undo: Undo::Patch {
                id,
                inverse,
                fields,
            },
            receipt,
            stats,
            weight: 1,
        });
        self.weight += 1;
        Ok(value)
    }
    pub fn apply(&mut self, mutation: &Mutation) -> Result<Option<Vec<NodeId>>, Error> {
        if !self.supports(mutation) {
            return Err(Error::Unsupported("submission operation"));
        }
        let receipt = self.tree.receipt;
        let stats = self.tree.stats;
        let mut retired = None;
        let mut weight = 1;
        let undo = match mutation {
            Mutation::CreatePlan { ids, .. } => {
                weight = ids.len();
                let old = ids
                    .iter()
                    .map(|id| (*id, self.tree.generations.get(&id.slot()).copied()))
                    .collect();
                self.tree.apply(std::slice::from_ref(mutation))?;
                Undo::Create(old)
            }
            Mutation::Place { id, .. } => {
                let link = Link::read(self.tree.node(*id)?);
                self.tree.apply(std::slice::from_ref(mutation))?;
                Undo::Place(*id, link)
            }
            Mutation::Dispose { id } => {
                let link = Link::read(self.tree.node(*id)?);
                let mut pending = vec![*id];
                let mut ids = Vec::new();
                while let Some(id) = pending.pop() {
                    if ids.len() == NODE_LIMIT - self.weight {
                        return Err(Error::Unsupported("submission node budget"));
                    }
                    let node = self.tree.node(id)?;
                    if node.descriptor.view().semantics.is_some()
                        || node.clipboard.is_some()
                        || node.files.is_some()
                        || node.swipe.is_some()
                    {
                        return Err(Error::Unsupported("submission retirement facets"));
                    }
                    ids.push(id);
                    pending.extend(node.children.iter().copied());
                }
                self.tree.unlink(*id);
                weight = ids.len();
                let nodes = ids
                    .iter()
                    .map(|id| {
                        self.tree
                            .generations
                            .get_mut(&id.slot())
                            .expect("live generation")
                            .1 = false;
                        (*id, self.tree.nodes.remove(id).expect("retired node"))
                    })
                    .collect();
                self.tree.stats.disposed += ids.len() as u64;
                self.tree.receipt.applied += 1;
                retired = Some(ids);
                Undo::Dispose {
                    root: *id,
                    link,
                    nodes,
                }
            }
            _ => return Err(Error::Unsupported("use submission patch")),
        };
        self.records.push(Record {
            undo,
            receipt,
            stats,
            weight,
        });
        self.weight += weight;
        Ok(retired)
    }
    fn rollback(&mut self, accepted: usize) {
        while self.records.len() > accepted {
            let Record {
                undo,
                receipt,
                stats,
                weight,
            } = self.records.pop().expect("suffix");
            self.weight -= weight;
            match undo {
                Undo::Create(ids) => {
                    self.tree.retire(ids[0].0, None).expect("new subtree");
                    for (id, generation) in ids {
                        if let Some(generation) = generation {
                            self.tree.generations.insert(id.slot(), generation);
                        } else {
                            self.tree.generations.remove(&id.slot());
                        }
                    }
                }
                Undo::Patch {
                    id,
                    mut inverse,
                    fields,
                } => {
                    let descriptor = &mut self
                        .tree
                        .nodes
                        .get_mut(&id)
                        .expect("patched node")
                        .descriptor;
                    descriptor.swap_patch(&mut inverse).expect("field inverse");
                    descriptor.restore_fields(fields);
                }
                Undo::Place(id, link) => link.restore(self.tree, id),
                Undo::Dispose { root, link, nodes } => {
                    for (id, mut node) in nodes {
                        if id == root {
                            node.placed = false;
                        }
                        self.tree
                            .generations
                            .get_mut(&id.slot())
                            .expect("retired generation")
                            .1 = true;
                        self.tree.nodes.insert(id, node);
                    }
                    link.restore(self.tree, root);
                }
            }
            self.tree.receipt = receipt;
            self.tree.stats = stats;
        }
    }
    pub fn confirm(&mut self, accepted: usize) -> Receipt {
        assert!(accepted <= self.records.len());
        self.rollback(accepted);
        self.records.clear();
        self.weight = 0;
        self.tree.receipt
    }
    pub fn finish(mut self, accepted: usize) -> Receipt {
        self.confirm(accepted)
    }
}
impl Drop for Submission<'_> {
    fn drop(&mut self) {
        self.rollback(0);
    }
}
