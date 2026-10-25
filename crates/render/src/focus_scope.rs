//! Current focus boundaries and bounded live-subtree admission, without layout.
use crate::{
    DescriptorView, Error, NodeId, SemanticId,
    memory::{Memory, Subtree},
};

pub const MAX_FOCUS_SCOPES: usize = 1024;
pub const MAX_FOCUS_SCOPE_NODES: usize = 4096;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FocusScopeConfig {
    pub active: bool,
    pub loop_tab: bool,
    pub contain_focus: bool,
    pub auto_focus: bool,
    pub restore_focus: bool,
    pub initial_focus: Option<SemanticId>,
    pub return_focus: Option<SemanticId>,
}
impl Default for FocusScopeConfig {
    fn default() -> Self {
        Self {
            active: true,
            loop_tab: true,
            contain_focus: true,
            auto_focus: true,
            restore_focus: true,
            initial_focus: None,
            return_focus: None,
        }
    }
}
pub(crate) fn declared(descriptor: DescriptorView<'_>) -> bool {
    descriptor
        .semantics
        .as_ref()
        .is_some_and(|semantics| semantics.focus_scope.is_some())
}
pub(crate) struct Movement {
    old: Vec<NodeId>,
    new: Vec<NodeId>,
    size: usize,
}
impl Movement {
    pub(crate) fn commit(self, tree: &mut Memory) {
        for root in self.old {
            *tree.focus_scopes.get_mut(&root).expect("old scope") -= self.size;
        }
        for root in self.new {
            *tree.focus_scopes.get_mut(&root).expect("new scope") += self.size;
        }
    }
}
impl Memory {
    pub fn focus_scopes(&self) -> usize {
        self.focus_scopes.len()
    }
    /// Only current declared roots; no visited nodes or business data.
    pub fn focus_scope_roots(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.focus_scopes.keys().copied()
    }
    pub fn focus_scope_nodes(&self, root: NodeId) -> Result<Option<usize>, Error> {
        self.node(root)?;
        Ok(self.focus_scopes.get(&root).copied())
    }
    pub(crate) fn admit_focus_scopes(&self, additional: usize) -> Result<(), Error> {
        if additional > MAX_FOCUS_SCOPES - self.focus_scopes.len() {
            return Err(Error::InvalidProps("live FocusScope budget exceeded"));
        }
        Ok(())
    }
    pub(crate) fn scope_ancestors(&self, mut parent: Option<NodeId>) -> Result<Vec<NodeId>, Error> {
        let mut scopes = Vec::new();
        if self.focus_scopes.is_empty() {
            return Ok(scopes);
        }
        while let Some(id) = parent {
            let node = self.node(id)?;
            if self.focus_scopes.contains_key(&id) {
                scopes.push(id);
            }
            parent = node.parent;
        }
        Ok(scopes)
    }
    pub(crate) fn scope_subtree_size(&self, root: NodeId, limit: usize) -> Result<usize, Error> {
        if let Some(size) = self.focus_scopes.get(&root) {
            return if *size <= limit {
                Ok(*size)
            } else {
                Err(Error::InvalidPlacement(
                    "restricted FocusScope subtree exceeds its live node budget",
                ))
            };
        }
        let mut pending = vec![root];
        let mut size = 0usize;
        while let Some(id) = pending.pop() {
            if id != root
                && let Some(nested) = self.focus_scopes.get(&id)
            {
                size = size
                    .checked_add(*nested)
                    .ok_or(Error::InvalidPlacement("FocusScope size overflow"))?;
            } else {
                let node = self.node(id)?;
                size += 1;
                if size + pending.len() + node.children.len() > limit {
                    return Err(Error::InvalidPlacement(
                        "restricted FocusScope subtree exceeds its live node budget",
                    ));
                }
                pending.extend(node.children.iter().copied());
            }
            if size > limit {
                return Err(Error::InvalidPlacement(
                    "restricted FocusScope subtree exceeds its live node budget",
                ));
            }
        }
        Ok(size)
    }
    pub(crate) fn scope_movement(
        &self,
        id: NodeId,
        parent: Option<NodeId>,
    ) -> Result<Option<Movement>, Error> {
        if self.focus_scopes.is_empty() {
            return Ok(None);
        }
        let old = self.scope_ancestors(self.node(id)?.parent)?;
        let new = self.scope_ancestors(parent)?;
        let leaving = old
            .iter()
            .filter(|id| !new.contains(id))
            .copied()
            .collect::<Vec<_>>();
        let entering = new
            .iter()
            .filter(|id| !old.contains(id))
            .copied()
            .collect::<Vec<_>>();
        if leaving.is_empty() && entering.is_empty() {
            return Ok(None);
        }
        let limit = entering
            .iter()
            .map(|id| MAX_FOCUS_SCOPE_NODES - self.focus_scopes[id])
            .min()
            .unwrap_or(MAX_FOCUS_SCOPE_NODES);
        let size = self.scope_subtree_size(id, limit)?;
        Ok(Some(Movement {
            old: leaving,
            new: entering,
            size,
        }))
    }
    pub(crate) fn validate_focus_child(
        &self,
        child: Subtree<'_>,
        parent: Option<NodeId>,
    ) -> Result<(), Error> {
        let additional = match child {
            Subtree::New(descriptor) => usize::from(declared(descriptor)),
            Subtree::Plan(plan) => plan.focus_scopes(),
            Subtree::Existing(_) => 0,
        };
        self.admit_focus_scopes(additional)?;
        if self.focus_scopes.is_empty() {
            return Ok(());
        }
        match child {
            Subtree::Existing(id) => {
                self.scope_movement(id, parent)?;
            }
            Subtree::New(_) | Subtree::Plan(_) => {
                let size = match child {
                    Subtree::Plan(plan) => plan.nodes().len(),
                    _ => 1,
                };
                for root in self.scope_ancestors(parent)? {
                    if size > MAX_FOCUS_SCOPE_NODES - self.focus_scopes[&root] {
                        return Err(Error::InvalidPlacement(
                            "restricted FocusScope subtree exceeds its live node budget",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
