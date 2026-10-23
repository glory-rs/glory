//! Reference semantics for adapters. This interpreter performs no layout/paint.
//! It stores only live objects and at most one tombstone per allocated slot.

use std::collections::HashMap;

use crate::*;

#[derive(Clone, Debug)]
pub struct Node {
    pub descriptor: Descriptor,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub placed: bool,
    pub suspended: bool,
    // Logical adjacency, including suspended siblings. No historical identities.
    next: Option<NodeId>,
}

/// A proposed child of a logical parent. New descriptors have no identity or
/// descendants yet; existing ranges include their suspended descendants.
/// This borrowed view is for validation, never a snapshot or host-tree lookup.
#[derive(Clone, Copy, Debug)]
pub enum Subtree<'a> {
    New(&'a Descriptor),
    Existing(NodeId),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub created: u64,
    pub patched: u64,
    pub placed: u64,
    pub suspended: u64,
    pub disposed: u64,
}

#[derive(Debug)]
pub struct Memory {
    runtime: RuntimeId,
    nodes: HashMap<NodeId, Node>,
    generations: HashMap<u32, (u32, bool)>,
    roots: Vec<NodeId>,
    inputs: Vec<Input>,
    receipt: Receipt,
    stats: Stats,
}

/// One validated field update. Its descriptor is a borrowed view of the staged
/// value; the inverse contains only the replaced field. Drop rolls it back,
/// including on unwinding. No stats/watermark advance until commit.
#[must_use = "commit the prepared patch, or drop it to restore the original"]
pub struct PreparedPatch<'a> {
    tree: &'a mut Memory,
    id: NodeId,
    inverse: Option<Patch>,
}
impl PreparedPatch<'_> {
    pub fn descriptor(&self) -> &Descriptor {
        &self.tree.nodes[&self.id].descriptor
    }
    /// Inverse field patch, with Keep preserved and Clear materialized as Set.
    pub fn previous(&self) -> &Patch {
        self.inverse.as_ref().expect("uncommitted patch")
    }
    fn accept(&mut self) {
        self.inverse = None;
        self.tree.stats.patched += 1;
    }
    /// Accept exactly one mutation, using the same watermark as Render::apply.
    pub fn commit(mut self) -> Receipt {
        self.accept();
        self.tree.receipt.applied += 1;
        self.tree.receipt
    }
}
impl Drop for PreparedPatch<'_> {
    fn drop(&mut self) {
        if let Some(mut inverse) = self.inverse.take() {
            // The exclusive Memory borrow prevents disposal or replacement of
            // this node. Undo swaps only built-in field values, not user code.
            self.tree
                .nodes
                .get_mut(&self.id)
                .expect("prepared node is live")
                .descriptor
                .swap_patch(&mut inverse)
                .expect("validated inverse");
        }
    }
}

impl Memory {
    pub fn new(runtime: RuntimeId) -> Self {
        Self {
            runtime,
            nodes: HashMap::new(),
            generations: HashMap::new(),
            roots: Vec::new(),
            inputs: Vec::new(),
            receipt: Receipt::default(),
            stats: Stats::default(),
        }
    }

    pub fn node(&self, id: NodeId) -> Result<&Node, Error> {
        self.check_runtime(id)?;
        self.nodes.get(&id).ok_or(Error::NodeGone(id))
    }

    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    pub fn retained_slots(&self) -> usize {
        self.generations.len()
    }
    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Prepare against this validated/canonical live tree without cloning the
    /// descriptor or scanning unrelated payloads. The guard can be inspected by
    /// a lowering adapter, then committed, or dropped if adapter checks fail.
    pub fn prepare_patch(&mut self, id: NodeId, patch: &Patch) -> Result<PreparedPatch<'_>, Error> {
        self.node(id)?;
        let mut inverse = patch.clone();
        self.nodes
            .get_mut(&id)
            .expect("validated node")
            .descriptor
            .patch_live(&mut inverse)?;
        let prepared = PreparedPatch {
            tree: self,
            id,
            inverse: Some(inverse),
        };
        prepared
            .tree
            .validate_child(Subtree::Existing(id), prepared.tree.nodes[&id].parent)?;
        Ok(prepared)
    }

    /// Suspension is inherited, but each node retains its own logical owner
    /// and order. Disposing a parent also destroys its suspended children.
    pub fn active(&self, id: NodeId) -> bool {
        Self::active_in(&self.nodes, id)
    }

    fn active_in(nodes: &HashMap<NodeId, Node>, id: NodeId) -> bool {
        let mut cursor = Some(id);
        while let Some(id) = cursor {
            let Some(node) = nodes.get(&id) else {
                return false;
            };
            if !node.placed || node.suspended {
                return false;
            }
            cursor = node.parent;
        }
        true
    }

    fn prune_events(&mut self) {
        let nodes = &self.nodes;
        let runtime = self.runtime;
        self.inputs.retain(|input| {
            !matches!(input, Input::Event { node, event } if (!Self::active_in(nodes, *node) && !event.is_terminal_loss()) || !Self::event_in(nodes, runtime, *node, event).unwrap_or(false))
        });
    }

    /// False means an embedded origin was retired or is no longer active.
    /// Malformed live origins/payloads are errors; no callback or mutation runs.
    pub fn validate_event(&self, id: NodeId, event: &Event) -> Result<bool, Error> {
        Self::event_in(&self.nodes, self.runtime, id, event)
    }

    fn event_in(
        nodes: &HashMap<NodeId, Node>,
        runtime: RuntimeId,
        id: NodeId,
        event: &Event,
    ) -> Result<bool, Error> {
        let get = |id: NodeId| {
            if id.runtime() != runtime {
                return Err(Error::ForeignRuntime);
            }
            nodes.get(&id).ok_or(Error::NodeGone(id))
        };
        let target = get(id)?;
        if let Event::Drag { from, phase, .. } = event {
            event.validate_payload(id, &target.descriptor)?;
            let source = match get(*from) {
                Ok(source) => source,
                Err(Error::NodeGone(_)) => return Ok(false),
                Err(error) => return Err(error),
            };
            EventKind::Drag.validate_target(&source.descriptor)?;
            return Ok(Self::active_in(nodes, *from) || *phase == crate::DragPhase::Cancel);
        }
        // Navigation's explicit origin must be exactly the target, even if a
        // mismatched embedded identity has already been retired.
        if matches!(event, Event::Navigate { .. } | Event::Keyboard { .. }) {
            event.validate_payload(id, &target.descriptor)?;
            return Ok(true);
        }
        if let Some(origin) = event.origin() {
            let source = match get(origin) {
                Ok(node) => node,
                Err(Error::NodeGone(_)) => return Ok(false),
                Err(error) => return Err(error),
            };
            if matches!(source.descriptor.props, Props::Group) {
                return Err(Error::InvalidEvent("event origin has no physical box"));
            }
            if matches!(event, Event::Anchor { .. }) && origin == id {
                return Err(Error::InvalidEvent("anchor must originate in a descendant"));
            }
            let mut cursor = Some(origin);
            while cursor != Some(id) {
                cursor = cursor
                    .and_then(|id| nodes.get(&id))
                    .and_then(|node| node.parent);
                if cursor.is_none() {
                    return Err(Error::InvalidEvent("event origin is outside its target"));
                }
            }
            if !Self::active_in(nodes, origin)
                && !matches!(event, Event::FocusWithin { focused: false, .. })
            {
                return Ok(false);
            }
        }
        event.validate_payload(id, &target.descriptor)?;
        Ok(true)
    }

    pub fn push_event(&mut self, id: NodeId, event: Event) -> Result<(), Error> {
        let node = self.node(id)?;
        if !self.validate_event(id, &event)? {
            return Ok(());
        }
        if !self.active(id) && !event.is_terminal_loss() {
            return Err(Error::InvalidPlacement("event target is not active"));
        }
        if matches!(
            event,
            Event::Activate
                | Event::Keyboard { .. }
                | Event::CheckedChanged(_)
                | Event::ValueChanged { .. }
                | Event::Commit { .. }
        ) && !node.descriptor.props.enabled()
        {
            return Err(Error::InvalidProps(
                "disabled control cannot receive user edits or activation",
            ));
        }
        self.inputs.push(Input::Event { node: id, event });
        Ok(())
    }

    fn check_runtime(&self, id: NodeId) -> Result<(), Error> {
        if id.runtime() == self.runtime {
            Ok(())
        } else {
            Err(Error::ForeignRuntime)
        }
    }

    fn unlink(&mut self, id: NodeId) {
        let node = &self.nodes[&id];
        if !node.placed {
            return;
        }
        let siblings = if let Some(parent) = node.parent {
            &mut self
                .nodes
                .get_mut(&parent)
                .expect("validated parent")
                .children
        } else {
            &mut self.roots
        };
        let index = if siblings.last() == Some(&id) {
            siblings.len() - 1
        } else {
            siblings
                .iter()
                .position(|child| *child == id)
                .expect("placed sibling")
        };
        let previous = index.checked_sub(1).map(|index| siblings[index]);
        siblings.remove(index);
        if let Some(previous) = previous {
            self.nodes
                .get_mut(&previous)
                .expect("previous sibling")
                .next = self.nodes[&id].next;
        }
    }

    /// Next logical sibling of a placed node, including suspended siblings.
    /// Identity validation precedes the constant-time adjacency lookup.
    pub fn next_sibling(&self, id: NodeId) -> Result<Option<NodeId>, Error> {
        let node = self.node(id)?;
        Ok(node.placed.then_some(node.next).flatten())
    }

    /// Check a proposed parent/child relation without allocating identities or
    /// modifying state. Ordinary moves visit ancestors only; only restricted
    /// interactive/button contexts require examining the child's live subtree.
    pub fn validate_child(&self, child: Subtree<'_>, parent: Option<NodeId>) -> Result<(), Error> {
        if let Subtree::Existing(id) = child {
            self.node(id)?;
        }
        let mut cursor = parent;
        let mut interactive_ancestor = false;
        let mut button_ancestor = false;
        while let Some(ancestor) = cursor {
            if matches!(child, Subtree::Existing(id) if ancestor == id) {
                return Err(Error::InvalidPlacement("tree cycle"));
            }
            let node = self.node(ancestor)?;
            if cursor == parent && !node.descriptor.props.accepts_children() {
                return Err(Error::InvalidProps(
                    "leaf component cannot contain children",
                ));
            }
            interactive_ancestor |= node.descriptor.props.interactive();
            button_ancestor |= matches!(node.descriptor.props, Props::Button(_));
            cursor = node.parent;
        }
        // Most placements (including ordinary list moves) need no subtree scan.
        if !interactive_ancestor && !button_ancestor {
            return Ok(());
        }
        let check = |descriptor: &Descriptor| {
            if interactive_ancestor
                && (descriptor.props.interactive() || descriptor.focus != FocusMode::Auto)
            {
                return Err(Error::InvalidProps("interactive controls cannot be nested"));
            }
            if button_ancestor
                && !matches!(
                    descriptor.props,
                    Props::Group
                        | Props::Text(TextProps {
                            kind: TextKind::Inline,
                            ..
                        })
                        | Props::Image(_)
                )
            {
                return Err(Error::InvalidProps(
                    "button children must be inline text, images or groups",
                ));
            }
            Ok(())
        };
        match child {
            Subtree::New(descriptor) => check(descriptor)?,
            Subtree::Existing(id) => {
                let mut pending = vec![id];
                while let Some(id) = pending.pop() {
                    let node = self.node(id)?;
                    check(&node.descriptor)?;
                    pending.extend(node.children.iter().copied());
                }
            }
        }
        Ok(())
    }

    /// Read-only validation shared by App staging and direct reference apply.
    pub fn validate_placement(
        &self,
        id: NodeId,
        parent: Option<NodeId>,
        before: Option<NodeId>,
        resume: bool,
    ) -> Result<(), Error> {
        let node = self.node(id)?;
        if resume != node.suspended {
            return Err(Error::InvalidPlacement(
                "use Resume for suspended nodes and Place otherwise",
            ));
        }
        self.validate_child(Subtree::Existing(id), parent)?;
        if let Some(before) = before {
            let sibling = self.node(before)?;
            if before == id || !sibling.placed || sibling.parent != parent {
                return Err(Error::InvalidPlacement(
                    "before must be a different child of the target parent",
                ));
            }
        }
        Ok(())
    }

    fn place(
        &mut self,
        id: NodeId,
        parent: Option<NodeId>,
        before: Option<NodeId>,
        resume: bool,
    ) -> Result<(), Error> {
        self.validate_placement(id, parent, before, resume)?;
        self.unlink(id);
        let siblings = if let Some(parent) = parent {
            &mut self
                .nodes
                .get_mut(&parent)
                .expect("validated parent")
                .children
        } else {
            &mut self.roots
        };
        let index = before
            .map(|before| {
                siblings
                    .iter()
                    .position(|child| *child == before)
                    .expect("validated sibling")
            })
            .unwrap_or(siblings.len());
        let previous = index.checked_sub(1).map(|index| siblings[index]);
        siblings.insert(index, id);
        if let Some(previous) = previous {
            self.nodes
                .get_mut(&previous)
                .expect("previous sibling")
                .next = Some(id);
        }
        let node = self.nodes.get_mut(&id).expect("validated node");
        node.next = before;
        node.parent = parent;
        node.placed = true;
        node.suspended = false;
        self.stats.placed += 1;
        Ok(())
    }

    fn apply_one(&mut self, mutation: &Mutation) -> Result<(), Error> {
        match mutation {
            Mutation::Create { id, node } => {
                self.check_runtime(*id)?;
                node.validate()?;
                if self.nodes.contains_key(id) {
                    return Err(Error::DuplicateNode(*id));
                }
                if let Some((generation, live)) = self.generations.get(&id.slot()) {
                    if *live {
                        return Err(Error::InvalidPlacement("slot is still occupied"));
                    }
                    if *generation >= id.generation() {
                        return Err(Error::StaleNode(*id));
                    }
                }
                let mut descriptor = node.as_ref().clone();
                descriptor.normalize_input()?;
                self.generations.insert(id.slot(), (id.generation(), true));
                self.nodes.insert(
                    *id,
                    Node {
                        descriptor,
                        parent: None,
                        children: Vec::new(),
                        placed: false,
                        suspended: false,
                        next: None,
                    },
                );
                self.stats.created += 1;
            }
            Mutation::Patch { id, patch } => {
                self.prepare_patch(*id, patch)?.accept();
            }
            Mutation::Place { id, parent, before } => self.place(*id, *parent, *before, false)?,
            Mutation::Resume { id, parent, before } => self.place(*id, *parent, *before, true)?,
            Mutation::Suspend { id } => {
                let node = self.node(*id)?;
                if !node.placed || node.suspended {
                    return Err(Error::InvalidPlacement(
                        "only placed active ranges can be suspended",
                    ));
                }
                self.nodes.get_mut(id).expect("validated node").suspended = true;
                self.prune_events();
                self.stats.suspended += 1;
            }
            Mutation::Dispose { id } => {
                self.node(*id)?;
                self.unlink(*id);
                let mut pending = vec![*id];
                while let Some(id) = pending.pop() {
                    let node = self.nodes.remove(&id).expect("owned subtree");
                    self.generations
                        .get_mut(&id.slot())
                        .expect("known identity")
                        .1 = false;
                    pending.extend(node.children);
                    self.stats.disposed += 1;
                }
                // Pending native/browser events carry identities too. Do not
                // deliver an event whose target was destroyed in this batch.
                self.prune_events();
            }
        }
        Ok(())
    }
}

impl Render for Memory {
    fn listen(&mut self, node: NodeId, kind: EventKind, enabled: bool) -> Result<(), Error> {
        let descriptor = &self.node(node)?.descriptor;
        if enabled {
            kind.validate_target(descriptor)?;
        }
        Ok(())
    }
    fn action(&mut self, action: Action) -> Result<(), Error> {
        action.validate_target(&self.node(action.node())?.descriptor)?;
        Err(Error::Unsupported(
            "headless reference interpreter has no physical actions",
        ))
    }
    fn needs_observe(&self) -> bool {
        !self.inputs.is_empty()
    }
    fn style_profile(&self) -> style::Profile {
        style::Profile::RECORD
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }

    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, Error> {
        for mutation in mutations {
            self.apply_one(mutation)?;
            self.receipt.applied += 1;
        }
        Ok(self.receipt)
    }

    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        let result = query
            .validate_target(&self.node(query.node())?.descriptor)
            .and(Err(Error::Unsupported(
                "headless reference interpreter has no platform geometry, selection or focus",
            )));
        self.inputs.push(Input::Response {
            token,
            applied: self.receipt.applied,
            result,
        });
        Ok(())
    }

    fn drain(&mut self, inputs: &mut Vec<Input>) {
        self.prune_events();
        inputs.append(&mut self.inputs);
    }
}
