//! Reference semantics for adapters. This interpreter performs no layout/paint.
//! It stores live objects and one generation watermark per allocated slot.
//! Child sequences separately bound their unread numeric prefixes.

use std::collections::HashMap;

use crate::*;

mod children;
pub use children::Children;

/// Current live Layer facets, including hidden/detached declarations. Physical
/// hosts may open fewer; this shared admission bound prevents late host failure.
pub const MAX_LAYERS: usize = 1024;

fn layer(descriptor: DescriptorView<'_>) -> bool {
    descriptor
        .semantics
        .as_ref()
        .is_some_and(|s| s.layer.is_some())
}

#[derive(Clone, Debug)]
pub struct Node {
    pub descriptor: LiveDescriptor,
    pub parent: Option<NodeId>,
    pub children: Children,
    pub placed: bool,
    pub suspended: bool,
    // Logical adjacency, including suspended siblings. No historical identities.
    next: Option<NodeId>,
    clipboard: Option<Box<crate::clipboard::Requests>>,
    files: Option<Box<crate::files::state::State>>,
    swipe: Option<Box<crate::swipe::State>>,
}

/// A proposed child of a logical parent. New descriptors have no identity or
/// descendants yet; existing ranges include their suspended descendants.
/// This borrowed view is for validation, never a snapshot or host-tree lookup.
#[derive(Clone, Copy, Debug)]
pub enum Subtree<'a> {
    New(DescriptorView<'a>),
    Plan(&'a Plan),
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
    nodes: NodeMap<Node>,
    names: HashMap<SemanticId, NodeId>,
    generations: HashMap<u32, (u32, bool)>,
    roots: Children,
    inputs: Vec<Input>,
    receipt: Receipt,
    stats: Stats,
    layers: usize,
    pub(crate) focus_scopes: std::collections::BTreeMap<NodeId, usize>,
}

/// One validated field update. Its descriptor is a borrowed view of the staged
/// value; the inverse contains only the replaced field. Drop rolls it back,
/// including on unwinding. No stats/watermark advance until commit.
#[must_use = "commit the prepared patch, or drop it to restore the original"]
pub struct PreparedPatch<'a> {
    tree: &'a mut Memory,
    id: NodeId,
    inverse: Option<Patch>,
    fields_before: u8,
    focus_size: Option<usize>,
}
impl PreparedPatch<'_> {
    pub fn descriptor(&self) -> DescriptorView<'_> {
        self.tree.nodes[&self.id].descriptor.view()
    }
    /// Inverse field patch, with Keep preserved and Clear materialized as Set.
    pub fn previous(&self) -> &Patch {
        self.inverse.as_ref().expect("uncommitted patch")
    }
    fn accept(&mut self) {
        if matches!(self.inverse, Some(Patch::Semantics(_)))
            && let Some(capture) = self
                .descriptor()
                .semantics
                .as_ref()
                .and_then(|s| s.swipe_capture)
        {
            let target = self
                .tree
                .nodes
                .get_mut(&self.id)
                .expect("prepared swipe node");
            if let Some(state) = target.swipe.as_mut() {
                state.revision = capture.revision;
            } else {
                target.swipe = Some(crate::swipe::State::new(capture));
            }
        }
        if let Some(Patch::Semantics(Change::Set(old))) = &self.inverse {
            let before = old.as_ref().is_some_and(|s| s.focus_scope.is_some());
            let after = crate::focus_scope::declared(self.descriptor());
            if after && !before {
                self.tree
                    .focus_scopes
                    .insert(self.id, self.focus_size.expect("validated FocusScope size"));
            } else if before && !after {
                self.tree.focus_scopes.remove(&self.id);
            }
        }
        if let Some(Patch::File {
            configuration: Change::Set(previous),
            revision,
        }) = &self.inverse
        {
            let Props::File(current) = self.descriptor().props else {
                unreachable!()
            };
            if (**previous != current.configuration || *revision != current.revision)
                && let Some(state) = self
                    .tree
                    .nodes
                    .get_mut(&self.id)
                    .expect("prepared file node")
                    .files
                    .as_mut()
            {
                state.clear();
            }
        }
        if let Some(Patch::Semantics(Change::Set(previous))) = &self.inverse {
            let before = previous.as_ref().and_then(|s| s.clipboard.as_ref());
            let after = self
                .descriptor()
                .semantics
                .as_ref()
                .and_then(|s| s.clipboard.as_ref());
            if before != after
                && let Some(requests) = self
                    .tree
                    .nodes
                    .get_mut(&self.id)
                    .and_then(|n| n.clipboard.as_mut())
            {
                requests.cancel();
            }
        }
        if let Some(Patch::Semantics(Change::Set(old))) = &self.inverse {
            let old = old.as_ref().is_some_and(|s| s.layer.is_some());
            let new = layer(self.descriptor());
            self.tree.layers = self.tree.layers + usize::from(new) - usize::from(old);
        }
        if let Some(Patch::Semantics(Change::Set(old))) = &self.inverse {
            let old = old.as_ref().and_then(|s| s.id.as_ref());
            let new = self.descriptor().semantic_id();
            if old != new {
                let old = old.cloned();
                let new = new.cloned();
                if let Some(old) = old {
                    self.tree.names.remove(&old);
                }
                if let Some(new) = new {
                    self.tree.names.insert(new, self.id);
                }
            }
        }
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
            self.tree
                .nodes
                .get_mut(&self.id)
                .expect("prepared node")
                .descriptor
                .restore_fields(self.fields_before);
        }
    }
}

impl Memory {
    pub fn file_pending(&self, node: NodeId, request: u64, revision: u64) -> Result<bool, Error> {
        let node = self.node(node)?;
        Ok(node
            .files
            .as_ref()
            .is_some_and(|state| state.pending(request, revision))
            && matches!(node.descriptor.view().props,Props::File(props) if props.revision==revision))
    }
    pub fn validate_file_action(&self, action: Action) -> Result<(), Error> {
        if !matches!(
            action,
            Action::PickFiles { .. } | Action::ClearFiles { .. } | Action::RemoveFile(_)
        ) {
            return Ok(());
        }
        let node = action.node();
        action.validate_target(&self.node(node)?.descriptor)?;
        if matches!(action, Action::PickFiles { .. } | Action::RemoveFile(_))
            && (!self.active(node) || !self.enabled(node) || self.inert(node))
        {
            return Err(Error::Unsupported(
                "file control is inactive, disabled or inert",
            ));
        }
        if let Action::RemoveFile(file) = action {
            self.selected_file(file)?;
        }
        Ok(())
    }
    pub fn admit_file_action(&mut self, action: Action) -> Result<(), Error> {
        self.validate_file_action(action)?;
        match action {
            Action::PickFiles {
                node,
                revision,
                request,
            } => self.admit_file_picker(node, revision, request),
            Action::ClearFiles { node, revision } => self.clear_files(node, revision),
            Action::RemoveFile(file) => self.remove_file(file),
            _ => Ok(()),
        }
    }

    pub fn next_file_request(&self, node: NodeId) -> Result<u64, Error> {
        self.node(node)?
            .files
            .as_ref()
            .map_or(Ok(1), |state| state.next_request())
    }
    pub fn admit_file_picker(
        &mut self,
        node: NodeId,
        revision: u64,
        request: u64,
    ) -> Result<(), Error> {
        let Props::File(props) = self.node(node)?.descriptor.view().props else {
            return Err(Error::Unsupported("file picker requires a File control"));
        };
        if revision != props.revision {
            return Err(Error::InputRevision {
                current: props.revision,
                incoming: revision,
            });
        }
        if !self.active(node) || !self.enabled(node) || self.inert(node) {
            return Err(Error::Unsupported(
                "file picker is inactive, disabled or inert",
            ));
        }
        self.nodes
            .get_mut(&node)
            .expect("validated file node")
            .files
            .get_or_insert_with(Default::default)
            .admit(request, revision)
    }
    pub fn validate_file_event(
        &self,
        node: NodeId,
        document: FileDocumentId,
        event: &FileEvent,
    ) -> Result<bool, Error> {
        let target = self.node(node)?;
        let Props::File(props) = target.descriptor.view().props else {
            return Err(Error::InvalidEvent("file event requires a File control"));
        };
        if matches!(&event.outcome, FileOutcome::Selected(_))
            && (!self.active(node) || !self.enabled(node) || self.inert(node))
        {
            return Ok(false);
        }
        match target.files.as_ref() {
            Some(state) => state.validate_event(node, props, document, event),
            None => {
                crate::files::state::State::default().validate_event(node, props, document, event)
            }
        }
    }
    pub fn finish_files(
        &mut self,
        node: NodeId,
        document: FileDocumentId,
        event: &FileEvent,
    ) -> Result<bool, Error> {
        if !self.validate_file_event(node, document, event)? {
            return Ok(false);
        }
        let target = self.nodes.get_mut(&node).expect("validated file node");
        let Props::File(props) = target.descriptor.view().props else {
            unreachable!()
        };
        target
            .files
            .get_or_insert_with(Default::default)
            .observe(node, props, document, event)
    }
    pub fn selected_file(&self, token: FileToken) -> Result<&FileInfo, Error> {
        token.validate()?;
        let target = self.node(token.node)?;
        let Props::File(props) = target.descriptor.view().props else {
            return Err(Error::Unsupported("file token requires a File control"));
        };
        if props.revision != token.revision {
            return Err(Error::Unsupported(
                "file token belongs to a retired configuration",
            ));
        }
        target
            .files
            .as_ref()
            .ok_or(Error::Unsupported("file token is not selected"))?
            .file(token)
    }
    pub fn clear_files(&mut self, node: NodeId, revision: u64) -> Result<(), Error> {
        let target = self.node(node)?;
        let Props::File(props) = target.descriptor.view().props else {
            return Err(Error::Unsupported("clear files requires a File control"));
        };
        if revision != props.revision {
            return Err(Error::InputRevision {
                current: props.revision,
                incoming: revision,
            });
        }
        if let Some(state) = self
            .nodes
            .get_mut(&node)
            .expect("validated file node")
            .files
            .as_mut()
        {
            state.clear();
        }
        Ok(())
    }
    pub fn remove_file(&mut self, token: FileToken) -> Result<(), Error> {
        self.selected_file(token)?;
        self.nodes
            .get_mut(&token.node)
            .expect("validated file node")
            .files
            .as_mut()
            .expect("validated selection")
            .remove(token)
    }
    /// Numeric identity belongs to this live NodeId, even if the source facet
    /// is cleared and declared again. Merely reading it does not reserve it.
    pub fn next_clipboard_request(&self, node: NodeId) -> Result<u64, Error> {
        self.node(node)?
            .clipboard
            .as_ref()
            .map_or(Ok(1), |state| state.next())
    }
    pub fn clipboard_pending(
        &self,
        node: NodeId,
        request: u64,
        revision: u64,
    ) -> Result<bool, Error> {
        let node = self.node(node)?;
        Ok(node
            .clipboard
            .as_ref()
            .is_some_and(|state| state.matches(request, revision))
            && node
                .descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|s| s.clipboard.as_ref())
                .is_some_and(|source| source.revision == revision))
    }
    pub fn validate_clipboard_action(&self, action: Action) -> Result<(), Error> {
        if !matches!(action, Action::CopyText { .. }) {
            return Ok(());
        }
        let node = action.node();
        action.validate_target(&self.node(node)?.descriptor)?;
        if !self.active(node) || !self.enabled(node) || self.inert(node) {
            return Err(Error::Unsupported(
                "clipboard source is inactive, disabled or inert",
            ));
        }
        Ok(())
    }
    /// Shared request admission. Call after capability checks, immediately
    /// before queuing/performing the host action. No payload or callback cache.
    pub fn admit_clipboard(&mut self, action: Action) -> Result<(), Error> {
        let Action::CopyText {
            node,
            revision,
            request,
        } = action
        else {
            return Ok(());
        };
        self.validate_clipboard_action(action)?;
        self.nodes
            .get_mut(&node)
            .expect("validated node")
            .clipboard
            .get_or_insert_with(Default::default)
            .admit(request, revision)
    }
    /// First terminal delivery consumes the current pending numeric request.
    pub fn finish_clipboard(
        &mut self,
        node: NodeId,
        request: u64,
        revision: u64,
    ) -> Result<bool, Error> {
        self.node(node)?;
        self.nodes
            .get_mut(&node)
            .expect("validated node")
            .clipboard
            .as_mut()
            .ok_or(Error::InvalidEvent(
                "clipboard completion names an unadmitted request",
            ))?
            .observe(request, revision)
    }
    pub fn cancel_clipboard(&mut self, node: NodeId) -> Result<(), Error> {
        self.node(node)?;
        if let Some(state) = self
            .nodes
            .get_mut(&node)
            .expect("validated node")
            .clipboard
            .as_mut()
        {
            state.cancel()
        }
        Ok(())
    }
    pub fn new(runtime: RuntimeId) -> Self {
        Self {
            runtime,
            nodes: NodeMap::default(),
            names: HashMap::new(),
            generations: HashMap::new(),
            roots: Children::default(),
            inputs: Vec::new(),
            receipt: Receipt::default(),
            stats: Stats::default(),
            layers: 0,
            focus_scopes: std::collections::BTreeMap::new(),
        }
    }

    pub fn node(&self, id: NodeId) -> Result<&Node, Error> {
        self.check_runtime(id)?;
        self.nodes.get(&id).ok_or(Error::NodeGone(id))
    }

    /// Resolve a symbolic relation to its live identity in O(1), without owning
    /// the target or retaining names after disposal.
    pub fn named(&self, name: &SemanticId) -> Option<NodeId> {
        self.names.get(name).copied()
    }

    /// Nearest explicit collection boundary of a registered trigger. Walk only
    /// semantic ancestors; never infer membership for descendant editors.
    pub fn navigation(&self, id: NodeId) -> Option<(NodeId, Orientation)> {
        let node = self.node(id).ok()?;
        if !node.descriptor.view().semantics.as_ref()?.navigation_item {
            return None;
        }
        let mut parent = node.parent;
        while let Some(id) = parent {
            let node = self.node(id).ok()?;
            if let Some(axis) = node
                .descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|s| s.navigation)
            {
                return Some((id, axis));
            }
            parent = node.parent;
        }
        None
    }

    fn validate_name<'a>(
        &self,
        descriptor: impl Into<DescriptorView<'a>>,
        own: Option<NodeId>,
    ) -> Result<(), Error> {
        let descriptor = descriptor.into();
        self.validate_choice_reference(descriptor)?;
        if let Some(name) = descriptor.view().semantic_id()
            && self.names.get(name).is_some_and(|id| Some(*id) != own)
        {
            return Err(Error::InvalidProps("duplicate semantic id"));
        }
        Ok(())
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
    /// Numeric current declaration count; no source or historical layer scan.
    pub fn layers(&self) -> usize {
        self.layers
    }
    fn admit_layers(&self, additional: usize) -> Result<(), Error> {
        if additional > MAX_LAYERS - self.layers {
            return Err(Error::InvalidProps("live Layer budget exceeded"));
        }
        Ok(())
    }
    pub fn stats(&self) -> Stats {
        self.stats
    }
    /// Accepted operation watermark, without forcing observation or layout.
    pub fn receipt(&self) -> Receipt {
        self.receipt
    }

    /// Prepare against this validated/canonical live tree without cloning the
    /// descriptor or scanning unrelated payloads. The guard can be inspected by
    /// a lowering adapter, then committed, or dropped if adapter checks fail.
    pub fn prepare_patch(&mut self, id: NodeId, patch: &Patch) -> Result<PreparedPatch<'_>, Error> {
        self.node(id)?;
        if let Patch::Semantics(Change::Set(next)) = patch
            && let Some(capture) = next.as_ref().and_then(|s| s.swipe_capture)
        {
            let target = &self.nodes[&id];
            if target
                .descriptor
                .view()
                .semantics
                .as_ref()
                .is_none_or(|s| s.swipe_capture.is_none())
                && let Some(history) = &target.swipe
                && capture.revision <= history.revision
            {
                return Err(Error::InvalidProps(
                    "reactivating swipe capture must advance its node watermark",
                ));
            }
        }
        let fields_before = self.nodes[&id].descriptor.field_mask();
        let mut inverse = patch.clone();
        self.nodes
            .get_mut(&id)
            .expect("validated node")
            .descriptor
            .patch_live(&mut inverse)?;
        let mut prepared = PreparedPatch {
            tree: self,
            id,
            inverse: Some(inverse),
            fields_before,
            focus_size: None,
        };
        if matches!(prepared.previous(), Patch::Semantics(Change::Set(old)) if !old.as_ref().is_some_and(|s| s.focus_scope.is_some()))
            && crate::focus_scope::declared(prepared.descriptor())
        {
            prepared.tree.admit_focus_scopes(1)?;
            prepared.focus_size = Some(
                prepared
                    .tree
                    .scope_subtree_size(id, crate::MAX_FOCUS_SCOPE_NODES)?,
            );
        }
        if matches!(prepared.previous(), Patch::Semantics(Change::Set(old)) if !old.as_ref().is_some_and(|s| s.layer.is_some()))
            && layer(prepared.descriptor())
        {
            prepared.tree.admit_layers(1)?;
        }
        if matches!(patch, Patch::Role(_))
            && prepared.descriptor().role == Some(Role::Separator)
            && !prepared.tree.nodes[&id].children.is_empty()
        {
            return Err(Error::InvalidProps(
                "passive separators cannot contain children",
            ));
        }
        if matches!(patch, Patch::Role(_))
            && matches!(
                prepared.descriptor().role,
                Some(Role::Tooltip | Role::ProgressBar | Role::Slider)
            )
        {
            for child in prepared.tree.nodes[&id].children.iter().copied() {
                prepared
                    .tree
                    .validate_child(Subtree::Existing(child), Some(id))?;
            }
        }
        if matches!(patch, Patch::Semantics(_) | Patch::Role(_)) {
            let descriptor = prepared.descriptor();
            prepared.tree.validate_choice_patch(id)?;
            let role_transition = matches!(prepared.previous(), Patch::Role(Change::Set(old)) if *old == Some(Role::Listbox) || descriptor.role == Some(Role::Listbox));
            let changed_owner = matches!(prepared.previous(), Patch::Semantics(Change::Set(previous)) if previous.as_ref().and_then(|s| s.id.as_ref()) != descriptor.semantic_id());
            if role_transition || descriptor.role == Some(Role::Listbox) && changed_owner {
                prepared.tree.validate_choice_contents(id)?;
            }
        }
        if matches!(patch, Patch::Semantics(_)) {
            if let Patch::Semantics(Change::Set(previous)) = prepared.previous()
                && previous.as_ref().and_then(|s| s.document_part)
                    != prepared.descriptor().document_part()
            {
                return Err(Error::InvalidProps("live document structure cannot change"));
            }
            prepared
                .tree
                .validate_name(prepared.descriptor(), Some(id))?;
            if let Patch::Semantics(Change::Set(previous)) = prepared.previous()
                && previous.as_ref().and_then(|s| s.field_part)
                    != prepared.descriptor().field_part()
            {
                // Structural transitions must also validate existing content.
                // State/association updates never scan the child collection.
                if prepared.descriptor().field_part() == Some(crate::FieldPart::Form) {
                    let mut pending: Vec<_> =
                        prepared.tree.nodes[&id].children.iter().copied().collect();
                    while let Some(child) = pending.pop() {
                        let node = &prepared.tree.nodes[&child];
                        if node.descriptor.view().field_part() == Some(crate::FieldPart::Form) {
                            return Err(Error::InvalidPlacement("forms cannot be nested"));
                        }
                        pending.extend(node.children.iter().copied());
                    }
                }
                for child in prepared.tree.nodes[&id].children.iter().copied() {
                    prepared
                        .tree
                        .validate_child(Subtree::Existing(child), Some(id))?;
                }
            }
        }
        prepared.tree.validate_child_inner(
            Subtree::Existing(id),
            prepared.tree.nodes[&id].parent,
            false,
        )?;
        Ok(prepared)
    }

    /// Suspension is inherited, but each node retains its own logical owner
    /// and order. Disposing a parent also destroys its suspended children.
    pub fn active(&self, id: NodeId) -> bool {
        Self::active_in(&self.nodes, id)
    }
    /// Resolve one current named anchor and its ancestry. Missing or late-bound
    /// relations remain valid configuration, but cannot open a physical layer.
    pub fn layer_anchor_live(&self, id: NodeId) -> bool {
        Self::layer_anchor_live_in(&self.nodes, &self.names, id)
    }
    fn layer_anchor_live_in(
        nodes: &NodeMap<Node>,
        names: &HashMap<SemanticId, NodeId>,
        id: NodeId,
    ) -> bool {
        let Some(node) = nodes.get(&id) else {
            return false;
        };
        let Some(anchor) = (|| {
            node.descriptor
                .view()
                .semantics
                .as_ref()?
                .layer
                .as_ref()?
                .anchor
                .as_ref()
        })() else {
            return true;
        };
        let Some(anchor) = names.get(anchor).copied() else {
            return false;
        };
        if !Self::active_in(nodes, anchor)
            || Self::inert_in(nodes, anchor)
            || !nodes
                .get(&anchor)
                .is_some_and(|node| !matches!(node.descriptor.view().props, crate::Props::Group))
        {
            return false;
        }
        let mut cursor = Some(anchor);
        while let Some(id) = cursor {
            let Some(node) = nodes.get(&id) else {
                return false;
            };
            if node.descriptor.view().style.display == Some(crate::style::Display::None) {
                return false;
            }
            cursor = node.parent;
        }
        true
    }
    /// Logical group disabling also applies before a host has committed markup.
    /// No source scan: only the live target and its ancestors are inspected.
    pub fn enabled(&self, id: NodeId) -> bool {
        Self::enabled_in(&self.nodes, id)
    }
    fn enabled_in(nodes: &NodeMap<Node>, id: NodeId) -> bool {
        let mut cursor = Some(id);
        while let Some(id) = cursor {
            let Some(node) = nodes.get(&id) else {
                return false;
            };
            if !node.descriptor.view().props.enabled()
                || node.descriptor.view().semantics.as_ref().is_some_and(|s| {
                    s.inert
                        || matches!(
                            s.field_part,
                            Some(crate::FieldPart::Fieldset | crate::FieldPart::Form)
                        ) && s.disabled
                        || (s.splitter.is_some()
                            || s.slider.is_some()
                            || s.context_menu.is_some()
                            || s.drag_capture.is_some()
                            || s.swipe_capture.is_some()
                            || matches!(
                                node.descriptor.view().role,
                                Some(
                                    crate::Role::Menubar
                                        | crate::Role::Tree
                                        | crate::Role::TreeItem
                                )
                            ))
                            && s.disabled
                })
            {
                return false;
            }
            cursor = node.parent;
        }
        true
    }

    fn active_in(nodes: &NodeMap<Node>, id: NodeId) -> bool {
        let mut cursor = Some(id);
        while let Some(id) = cursor {
            let Some(node) = nodes.get(&id) else {
                return false;
            };
            if !node.placed || node.suspended || node.descriptor.view().hidden() {
                return false;
            }
            cursor = node.parent;
        }
        true
    }

    /// Current semantic ancestry only; inert keeps layout, owners and queries.
    pub fn inert(&self, id: NodeId) -> bool {
        Self::inert_in(&self.nodes, id)
    }
    fn inert_in(nodes: &NodeMap<Node>, id: NodeId) -> bool {
        let mut cursor = Some(id);
        while let Some(id) = cursor {
            let Some(node) = nodes.get(&id) else {
                return false;
            };
            if node
                .descriptor
                .view()
                .semantics
                .as_ref()
                .is_some_and(|s| s.inert)
            {
                return true;
            }
            cursor = node.parent;
        }
        false
    }

    fn prune_events(&mut self) {
        let nodes = &self.nodes;
        let names = &self.names;
        let runtime = self.runtime;
        self.inputs.retain(|input| {
            !matches!(input, Input::Event { node, event } if (!Self::active_in(nodes, *node) && !event.is_terminal_loss()) || !Self::event_in(nodes, runtime, *node, event).unwrap_or(false) || matches!(event, Event::Dismiss(DismissReason::AnchorLost)) && Self::layer_anchor_live_in(nodes,names,*node))
        });
    }

    /// False means an embedded origin was retired or is no longer active.
    /// Malformed live origins/payloads are errors; no callback or mutation runs.
    pub fn validate_event(&self, id: NodeId, event: &Event) -> Result<bool, Error> {
        if !Self::event_in(&self.nodes, self.runtime, id, event)? {
            return Ok(false);
        }
        if let Event::Clipboard {
            request, revision, ..
        } = event
        {
            event.validate_payload(id, &self.node(id)?.descriptor)?;
            let Some(requests) = self.node(id)?.clipboard.as_ref() else {
                return Err(Error::InvalidEvent(
                    "clipboard completion names an unadmitted request",
                ));
            };
            requests.validate(*request)?;
            return Ok(requests.matches(*request, *revision));
        }
        if matches!(event, Event::Dismiss(crate::DismissReason::AnchorLost)) {
            event.validate_payload(id, &self.node(id)?.descriptor)?;
            return Ok(self.active(id) && !self.layer_anchor_live(id));
        }
        if matches!(
            event,
            Event::Choice {
                command: crate::ChoiceCommand::RemoveLast,
                ..
            }
        ) {
            let owner = self
                .node(id)?
                .descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|value| value.controls.as_ref())
                .and_then(|name| self.named(name));
            let Some(owner) = owner else { return Ok(false) };
            let owner = self.node(owner)?.descriptor.view();
            if owner.role != Some(crate::Role::Listbox)
                || !owner
                    .semantics
                    .as_ref()
                    .is_some_and(|value| value.listbox_multiple)
            {
                return Err(Error::InvalidEvent(
                    "remove-last requires its live multiple listbox",
                ));
            }
        }
        Ok(true)
    }

    fn event_in(
        nodes: &NodeMap<Node>,
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
        if let Event::Clipboard {
            request, revision, ..
        } = event
        {
            let state = target.clipboard.as_ref().ok_or(Error::InvalidEvent(
                "clipboard completion names an unadmitted request",
            ))?;
            state.validate(*request)?;
            let Some(source) = target
                .descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|s| s.clipboard.as_ref())
            else {
                return Ok(false);
            };
            if *revision < source.revision {
                return Ok(false);
            }
            event.validate_payload(id, &target.descriptor)?;
            return Ok(state.matches(*request, *revision));
        }
        if let Event::CollapseComplete { revision } = event {
            event.validate_payload(id, &target.descriptor)?;
            if target
                .descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|s| s.collapse.as_ref())
                .is_some_and(|config| *revision < config.revision)
            {
                return Ok(false);
            }
        }
        let interactive = || {
            !event.is_user_interaction()
                || !(Self::inert_in(nodes, id)
                    || event
                        .origin()
                        .filter(|origin| *origin != id)
                        .is_some_and(|origin| Self::inert_in(nodes, origin)))
        };
        if let Event::RichEdit(snapshot) | Event::RichSelection(snapshot) = event {
            let Props::RichEditor(editor) = target.descriptor.view().props else {
                return Err(Error::InvalidEvent("rich input requires its RichEditor"));
            };
            event.validate_payload(id, &target.descriptor)?;
            if snapshot.revision < editor.revision {
                return Ok(false);
            }
            if matches!(event, Event::RichEdit(_)) && target.descriptor.view().readonly() {
                return Err(Error::InvalidEvent(
                    "readonly rich document cannot be edited",
                ));
            }
            return Ok(Self::active_in(nodes, id) && Self::enabled_in(nodes, id) && interactive());
        }
        if let Event::RichCommand { revision, .. } | Event::RichRejected { revision } = event {
            event.validate_payload(id, &target.descriptor)?;
            let Props::RichEditor(editor) = target.descriptor.view().props else {
                unreachable!()
            };
            return Ok(*revision == editor.revision
                && Self::active_in(nodes, id)
                && Self::enabled_in(nodes, id)
                && interactive()
                && (!matches!(event, Event::RichCommand { .. })
                    || !target.descriptor.view().readonly()));
        }
        if let Event::Swipe {
            from,
            x,
            y,
            revision,
            ..
        } = event
        {
            if from.runtime() != id.runtime() {
                return Err(Error::ForeignRuntime);
            }
            if *from != id || !x.is_finite() || !y.is_finite() {
                return Err(Error::InvalidEvent(
                    "swipe requires its exact finite boxed origin",
                ));
            }
            let capture = target
                .descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|s| s.swipe_capture);
            let Some(capture) = capture else {
                if target
                    .swipe
                    .as_ref()
                    .is_some_and(|state| *revision <= state.revision)
                {
                    return Ok(false);
                }
                return Err(Error::InvalidEvent("swipe target has no active capture"));
            };
            event.validate_payload(id, &target.descriptor)?;
            let mut visible = true;
            let mut cursor = Some(id);
            while let Some(id) = cursor {
                let node = get(id)?;
                let view = node.descriptor.view();
                if view.hidden() || view.style.display == Some(crate::style::Display::None) {
                    visible = false;
                    break;
                }
                cursor = node.parent;
            }
            return Ok(*revision == capture.revision
                && (Self::active_in(nodes, id) || event.is_terminal_loss())
                && (Self::enabled_in(nodes, id) || event.is_terminal_loss())
                && (visible || event.is_terminal_loss())
                && interactive());
        }
        if let Event::TreeNavigate { from, .. } = event {
            let source = match get(*from) {
                Ok(source) => source,
                Err(Error::NodeGone(_)) => return Ok(false),
                Err(error) => return Err(error),
            };
            event.validate_payload(id, &target.descriptor)?;
            let descriptor = source.descriptor.view();
            if descriptor.role != Some(crate::Role::TreeItem)
                || !matches!(descriptor.props, Props::View)
                || descriptor
                    .semantics
                    .as_ref()
                    .is_none_or(|s| s.tree_item.is_none())
            {
                return Err(Error::InvalidEvent(
                    "tree command origin must be a declared TreeItem",
                ));
            }
            if !Self::active_in(nodes, *from)
                || !Self::active_in(nodes, id)
                || !Self::enabled_in(nodes, *from)
                || !Self::enabled_in(nodes, id)
            {
                return Ok(false);
            }
            let mut cursor = source.parent;
            while let Some(current) = cursor {
                let ancestor = get(current)?;
                if ancestor.descriptor.view().role == Some(crate::Role::Tree) {
                    return if current == id {
                        Ok(interactive())
                    } else {
                        Err(Error::InvalidEvent("tree origin belongs to a nearer Tree"))
                    };
                }
                cursor = ancestor.parent;
            }
            return Err(Error::InvalidEvent("tree origin is outside its target"));
        }
        if let Event::GridNavigate { from, .. } = event {
            let source = match get(*from) {
                Ok(source) => source,
                Err(Error::NodeGone(_)) => return Ok(false),
                Err(error) => return Err(error),
            };
            event.validate_payload(id, &target.descriptor)?;
            if !matches!(source.descriptor.view().props, Props::Button(_)) {
                return Err(Error::InvalidEvent("grid origin must be a physical Button"));
            }
            if !Self::active_in(nodes, *from)
                || !Self::active_in(nodes, id)
                || !Self::enabled_in(nodes, *from)
            {
                return Ok(false);
            }
            let mut cursor = source.parent;
            while let Some(current) = cursor {
                let ancestor = get(current)?;
                if ancestor.descriptor.view().role == Some(crate::Role::Grid) {
                    return if current == id {
                        Ok(interactive())
                    } else {
                        Err(Error::InvalidEvent("grid origin belongs to a nearer Grid"))
                    };
                }
                cursor = ancestor.parent;
            }
            return Err(Error::InvalidEvent("grid origin is outside its target"));
        }
        if let Event::Sheet { revision, .. } = event {
            event.validate_payload(id, &target.descriptor)?;
            if target
                .descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|s| s.layer.as_ref())
                .and_then(|l| l.sheet.as_ref())
                .is_some_and(|sheet| *revision < sheet.revision)
            {
                return Ok(false);
            }
        }
        if let Event::Pointer { related, .. } | Event::PointerWithin { related, .. } = event {
            event.validate_payload(id, &target.descriptor)?;
            if let Some(related) = related {
                let source = match get(*related) {
                    Ok(node) => node,
                    Err(Error::NodeGone(_)) => return Ok(false),
                    Err(error) => return Err(error),
                };
                if matches!(source.descriptor.view().props, Props::Group) {
                    return Err(Error::InvalidEvent(
                        "pointer related origin has no physical box",
                    ));
                }
                if !Self::active_in(nodes, *related) {
                    return Ok(false);
                }
                let mut cursor = Some(*related);
                while let Some(current) = cursor {
                    if current == id {
                        return Ok(false);
                    }
                    cursor = nodes.get(&current).and_then(|node| node.parent);
                }
            }
            if matches!(event, Event::Pointer { .. }) {
                return Ok(
                    (Self::active_in(nodes, id) || event.is_terminal_loss()) && interactive()
                );
            }
        }
        if let Event::Submit {
            submitter: Some(submitter),
        } = event
        {
            event.validate_payload(id, &target.descriptor)?;
            let source = match get(*submitter) {
                Ok(source) => source,
                Err(Error::NodeGone(_)) => return Ok(false),
                Err(error) => return Err(error),
            };
            if source
                .descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|s| s.form_action)
                != Some(crate::FormAction::Submit)
            {
                return Err(Error::InvalidEvent(
                    "submit origin requires a submit button",
                ));
            }
            if !source.descriptor.view().activation_enabled() || !Self::active_in(nodes, *submitter)
            {
                return Ok(false);
            }
        }
        if target.descriptor.view().readonly()
            && matches!(
                event,
                Event::ValueChanged { .. }
                    | Event::InputEdit(_)
                    | Event::Commit { .. }
                    | Event::CheckedChanged(_)
                    | Event::SelectChanged { .. }
                    | Event::Range { .. }
                    | Event::Spin { .. }
                    | Event::Drag {
                        phase: crate::DragPhase::Start
                            | crate::DragPhase::Move
                            | crate::DragPhase::Drop,
                        ..
                    }
            )
        {
            event.validate_payload(id, &target.descriptor)?;
            return Ok(false);
        }
        if let Event::Drag { from, phase, .. } = event {
            event.validate_payload(id, &target.descriptor)?;
            let source = match get(*from) {
                Ok(source) => source,
                Err(Error::NodeGone(_)) => return Ok(false),
                Err(error) => return Err(error),
            };
            EventKind::Drag.validate_target(&source.descriptor)?;
            return Ok(
                (Self::active_in(nodes, *from) || *phase == crate::DragPhase::Cancel)
                    && interactive(),
            );
        }
        // Navigation's explicit origin must be exactly the target, even if a
        // mismatched embedded identity has already been retired.
        if let Event::NavigationPanel { command, .. } = event {
            let mut cursor = Some(id);
            let mut inside = false;
            while let Some(current) = cursor {
                let node = get(current)?;
                if node
                    .descriptor
                    .view()
                    .semantics
                    .as_ref()
                    .and_then(|s| s.navigation_panel)
                    == Some(crate::NavigationPanel::Content)
                {
                    inside = true;
                    break;
                }
                cursor = node.parent;
            }
            if matches!(
                command,
                crate::NavigationPanelCommand::Close | crate::NavigationPanelCommand::Open
            ) && !inside
                || matches!(
                    command,
                    crate::NavigationPanelCommand::First | crate::NavigationPanelCommand::Last
                ) && inside
            {
                return Err(Error::InvalidEvent(
                    "navigation panel command disagrees with its mounted content level",
                ));
            }
        }
        if matches!(
            event,
            Event::Navigate { .. }
                | Event::Keyboard { .. }
                | Event::Palette { .. }
                | Event::ScrollKeyboard { .. }
                | Event::Range { .. }
                | Event::Spin { .. }
                | Event::Typeahead { .. }
                | Event::Menu { .. }
                | Event::NavigationPanel { .. }
                | Event::Choice { .. }
                | Event::Tags { .. }
        ) {
            event.validate_payload(id, &target.descriptor)?;
            return Ok(interactive()
                && (!matches!(event, Event::Palette { .. }) || Self::active_in(nodes, id)));
        }
        if let Some(origin) = event.origin() {
            let source = match get(origin) {
                Ok(node) => node,
                Err(Error::NodeGone(_)) => return Ok(false),
                Err(error) => return Err(error),
            };
            if matches!(source.descriptor.view().props, Props::Group) {
                return Err(Error::InvalidEvent("event origin has no physical box"));
            }
            if matches!(event, Event::ContextMenu { .. }) {
                if !Self::active_in(nodes, origin) {
                    return Ok(false);
                }
                if target
                    .descriptor
                    .view()
                    .semantics
                    .as_ref()
                    .and_then(|s| s.context_menu)
                    == Some(crate::ContextEditPolicy::Native)
                    && matches!(
                        source.descriptor.view().props,
                        Props::Input(_) | Props::Select(_)
                    )
                {
                    return Ok(false);
                }
            }
            if matches!(event, Event::Anchor { .. }) && origin == id {
                return Err(Error::InvalidEvent("anchor must originate in a descendant"));
            }
            let mut cursor = Some(origin);
            while cursor != Some(id) {
                if matches!(event, Event::ContextMenu { .. })
                    && cursor.and_then(|id| nodes.get(&id)).is_some_and(|node| {
                        node.descriptor
                            .view()
                            .semantics
                            .as_ref()
                            .is_some_and(|s| s.context_menu.is_some())
                    })
                {
                    return Err(Error::InvalidEvent(
                        "context origin belongs to a nearer region",
                    ));
                }
                if matches!(event, Event::Submit { .. })
                    && cursor.and_then(|id| nodes.get(&id)).is_some_and(|node| {
                        node.descriptor.view().field_part() == Some(crate::FieldPart::Fieldset)
                            && node
                                .descriptor
                                .view()
                                .semantics
                                .as_ref()
                                .is_some_and(|s| s.disabled)
                    })
                {
                    return Ok(false);
                }
                cursor = cursor
                    .and_then(|id| nodes.get(&id))
                    .and_then(|node| node.parent);
                if cursor.is_none() {
                    return Err(Error::InvalidEvent("event origin is outside its target"));
                }
            }
            if !Self::active_in(nodes, origin) && !event.is_terminal_loss() {
                return Ok(false);
            }
        }
        event.validate_payload(id, &target.descriptor)?;
        Ok(interactive())
    }

    pub fn push_event(&mut self, id: NodeId, event: Event) -> Result<(), Error> {
        let node = self.node(id)?;
        if matches!(event, Event::Activate) && !node.descriptor.view().activation_enabled() {
            return Err(Error::InvalidProps("activation is disabled"));
        }
        if !self.validate_event(id, &event)? {
            return Ok(());
        }
        if !self.active(id) && !event.is_terminal_loss() {
            return Err(Error::InvalidPlacement("event target is not active"));
        }
        if matches!(
            event,
            Event::Activate
                | Event::Submit { .. }
                | Event::Reset
                | Event::Keyboard { .. }
                | Event::Palette { .. }
                | Event::ScrollKeyboard { .. }
                | Event::Range { .. }
                | Event::Spin { .. }
                | Event::Typeahead { .. }
                | Event::Menu { .. }
                | Event::NavigationPanel { .. }
                | Event::Choice { .. }
                | Event::Tags { .. }
                | Event::ContextMenu { .. }
                | Event::Drag {
                    phase: crate::DragPhase::Start
                        | crate::DragPhase::Move
                        | crate::DragPhase::Drop,
                    ..
                }
                | Event::CheckedChanged(_)
                | Event::SelectChanged { .. }
                | Event::ValueChanged { .. }
                | Event::Commit { .. }
        ) && !self.enabled(id)
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
        let next = node.next;
        let siblings = if let Some(parent) = node.parent {
            &mut self
                .nodes
                .get_mut(&parent)
                .expect("validated parent")
                .children
        } else {
            &mut self.roots
        };
        let index = if siblings.first() == Some(&id) {
            0
        } else if siblings.last() == Some(&id) {
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
                .next = next;
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
        self.validate_child_inner(child, parent, true)
    }

    fn validate_child_inner(
        &self,
        child: Subtree<'_>,
        parent: Option<NodeId>,
        choice: bool,
    ) -> Result<(), Error> {
        let descriptor = match child {
            Subtree::New(d) => d,
            Subtree::Plan(plan) => plan.nodes()[0].descriptor.view(),
            Subtree::Existing(id) => self.node(id)?.descriptor.view(),
        };
        if let Some(parent) = parent
            && self.node(parent)?.descriptor.view().role == Some(Role::Separator)
        {
            return Err(Error::InvalidProps(
                "passive separators cannot contain children",
            ));
        }
        self.validate_document_placement(child, parent)?;
        if choice {
            self.validate_choice_child(child, parent)?;
        }
        if descriptor.field_part() == Some(crate::FieldPart::Legend)
            && parent.and_then(|id| self.node(id).ok()).is_none_or(|node| {
                node.descriptor.view().field_part() != Some(crate::FieldPart::Fieldset)
            })
        {
            return Err(Error::InvalidPlacement(
                "legend requires a direct fieldset parent",
            ));
        }
        if let Subtree::Existing(id) = child {
            self.node(id)?;
        }
        let mut cursor = parent;
        let mut interactive_ancestor = false;
        let mut button_ancestor = false;
        let mut form_ancestor = false;
        let mut tooltip_ancestor = false;
        while let Some(ancestor) = cursor {
            if matches!(child, Subtree::Existing(id) if ancestor == id) {
                return Err(Error::InvalidPlacement("tree cycle"));
            }
            let node = self.node(ancestor)?;
            if cursor == parent && !node.descriptor.view().props.accepts_children() {
                return Err(Error::InvalidProps(
                    "leaf component cannot contain children",
                ));
            }
            interactive_ancestor |= node.descriptor.view().props.interactive()
                || matches!(
                    node.descriptor.view().field_part(),
                    Some(crate::FieldPart::Label | crate::FieldPart::Legend)
                );
            button_ancestor |= matches!(
                node.descriptor.view().props,
                Props::Button(_) | Props::File(_)
            );
            form_ancestor |= node.descriptor.view().field_part() == Some(crate::FieldPart::Form);
            tooltip_ancestor |= matches!(
                node.descriptor.view().role,
                Some(Role::Tooltip | Role::ProgressBar | Role::Slider)
            );
            cursor = node.parent;
        }
        // Most placements (including ordinary list moves) need no subtree scan.
        self.validate_focus_child(child, parent)?;
        if let Subtree::New(descriptor) = child {
            self.validate_name(descriptor, None)?;
        }
        if let Subtree::Plan(plan) = child {
            for node in plan.nodes() {
                self.validate_name(&node.descriptor, None)?;
            }
        }
        match child {
            Subtree::New(descriptor) => self.admit_layers(usize::from(layer(descriptor)))?,
            Subtree::Plan(plan) => self.admit_layers(
                plan.nodes()
                    .iter()
                    .filter(|node| layer(node.descriptor.view()))
                    .count(),
            )?,
            Subtree::Existing(_) => {}
        }
        if form_ancestor && descriptor.field_part() == Some(crate::FieldPart::Form) {
            return Err(Error::InvalidPlacement("forms cannot be nested"));
        }
        // A same-parent property update/reorder cannot introduce a new nested
        // form. Only a changed ancestor context needs the moved live subtree.
        let form_scan = form_ancestor
            && match child {
                Subtree::Existing(id) => self.node(id)?.parent != parent,
                _ => true,
            };
        if !interactive_ancestor && !button_ancestor && !form_scan && !tooltip_ancestor {
            return Ok(());
        }
        let check = |descriptor: DescriptorView<'_>| {
            if form_scan && descriptor.field_part() == Some(crate::FieldPart::Form) {
                return Err(Error::InvalidPlacement("forms cannot be nested"));
            }
            crate::plan::validate_context(
                descriptor,
                interactive_ancestor,
                button_ancestor,
                tooltip_ancestor,
            )
        };
        match child {
            Subtree::New(descriptor) => check(descriptor)?,
            Subtree::Plan(plan) => {
                for node in plan.nodes() {
                    check(node.descriptor.view())?;
                }
            }
            Subtree::Existing(id) => {
                let mut pending = vec![id];
                while let Some(id) = pending.pop() {
                    let node = self.node(id)?;
                    check(node.descriptor.view())?;
                    pending.extend(node.children.iter().copied());
                }
            }
        }
        Ok(())
    }

    fn validate_document_placement(
        &self,
        child: Subtree<'_>,
        parent: Option<NodeId>,
    ) -> Result<(), Error> {
        let mut boxed_parent = parent;
        while let Some(id) = boxed_parent {
            let node = self.node(id)?;
            if !matches!(node.descriptor.view().props, Props::Group) {
                break;
            }
            boxed_parent = node.parent;
        }
        let parent = boxed_parent
            .map(|id| self.node(id).map(|node| node.descriptor.view()))
            .transpose()?;
        let check = |descriptor: DescriptorView<'_>| {
            crate::document::validate_child(descriptor, parent, false)
        };
        match child {
            Subtree::New(descriptor) => check(descriptor)?,
            Subtree::Existing(id) => {
                // Only transparent ranges change the physical parent of their
                // contents. Stop at boxed nodes; never scan business sources.
                let node = self.node(id)?;
                check(node.descriptor.view())?;
                if matches!(node.descriptor.view().props, Props::Group) {
                    let mut pending = node.children.iter().copied().collect::<Vec<_>>();
                    while let Some(id) = pending.pop() {
                        let node = self.node(id)?;
                        check(node.descriptor.view())?;
                        if matches!(node.descriptor.view().props, Props::Group) {
                            pending.extend(node.children.iter().copied());
                        }
                    }
                }
            }
            Subtree::Plan(plan) => {
                for node in plan.nodes() {
                    let mut cursor = node.parent;
                    while let Some(index) = cursor {
                        if !matches!(plan.nodes()[index].descriptor.view().props, Props::Group) {
                            break;
                        }
                        cursor = plan.nodes()[index].parent;
                    }
                    if cursor.is_none() {
                        check(node.descriptor.view())?;
                    }
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
        let focus_movement = self.scope_movement(id, parent)?;
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
                if siblings.first() == Some(&before) {
                    return 0;
                }
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
        if let Some(movement) = focus_movement {
            movement.commit(self);
        }
        self.stats.placed += 1;
        Ok(())
    }

    fn apply_one(&mut self, mutation: &Mutation) -> Result<(), Error> {
        match mutation {
            Mutation::CreatePlan { plan, ids } => {
                if ids.len() != plan.nodes().len() {
                    return Err(Error::InvalidProps("plan identity count"));
                }
                let mut slots = std::collections::HashSet::with_capacity(ids.len());
                for (id, node) in ids.iter().zip(plan.nodes()) {
                    self.check_runtime(*id)?;
                    if !slots.insert(id.slot()) {
                        return Err(Error::InvalidPlacement("duplicate plan slot"));
                    }
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
                    self.validate_name(&node.descriptor, None)?;
                }
                self.admit_layers(
                    plan.nodes()
                        .iter()
                        .filter(|node| layer(node.descriptor.view()))
                        .count(),
                )?;
                let scope_count = plan
                    .nodes()
                    .iter()
                    .filter(|node| crate::focus_scope::declared(node.descriptor.view()))
                    .count();
                self.admit_focus_scopes(scope_count)?;
                let mut focus_sizes = Vec::new();
                if scope_count > 0 {
                    focus_sizes.resize(ids.len(), 1usize);
                    for (index, node) in plan.nodes().iter().enumerate().rev() {
                        if let Some(parent) = node.parent {
                            focus_sizes[parent] += focus_sizes[index];
                        }
                    }
                }
                for (index, node) in plan.nodes().iter().enumerate() {
                    // The immutable declaration checked content/topology once;
                    // the complete identity/name check above precedes mutation.
                    // Fresh nodes need no unlink, ancestor walk or temporary Box.
                    let id = ids[index];
                    let descriptor = LiveDescriptor::plan(plan, index);
                    self.layers += usize::from(layer(descriptor.view()));
                    if crate::focus_scope::declared(descriptor.view()) {
                        self.focus_scopes.insert(id, focus_sizes[index]);
                    }
                    if let Some(name) = descriptor.view().semantic_id() {
                        self.names.insert(name.clone(), id);
                    }
                    self.generations.insert(id.slot(), (id.generation(), true));
                    let swipe = descriptor
                        .view()
                        .semantics
                        .as_ref()
                        .and_then(|s| s.swipe_capture)
                        .map(crate::swipe::State::new);
                    self.nodes.insert(
                        id,
                        Node {
                            descriptor,
                            parent: node.parent.map(|parent| ids[parent]),
                            children: Children::default(),
                            placed: node.parent.is_some(),
                            suspended: false,
                            next: None,
                            clipboard: None,
                            files: None,
                            swipe,
                        },
                    );
                    self.stats.created += 1;
                    if let Some(parent) = node.parent {
                        let siblings = &mut self
                            .nodes
                            .get_mut(&ids[parent])
                            .expect("plan parent")
                            .children;
                        let previous = siblings.last().copied();
                        siblings.insert(siblings.len(), id);
                        if let Some(previous) = previous {
                            self.nodes.get_mut(&previous).expect("plan sibling").next = Some(id);
                        }
                        self.stats.placed += 1;
                    }
                }
            }
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
                self.validate_name(node.as_ref(), None)?;
                self.admit_layers(usize::from(layer(node.view())))?;
                self.admit_focus_scopes(usize::from(crate::focus_scope::declared(node.view())))?;
                descriptor.normalize_input()?;
                self.layers += usize::from(layer(descriptor.view()));
                if crate::focus_scope::declared(descriptor.view()) {
                    self.focus_scopes.insert(*id, 1);
                }
                if let Some(name) = descriptor.view().semantic_id() {
                    self.names.insert(name.clone(), *id);
                }
                self.generations.insert(id.slot(), (id.generation(), true));
                let swipe = descriptor
                    .view()
                    .semantics
                    .as_ref()
                    .and_then(|s| s.swipe_capture)
                    .map(crate::swipe::State::new);
                self.nodes.insert(
                    *id,
                    Node {
                        descriptor: LiveDescriptor::owned(descriptor),
                        parent: None,
                        children: Children::default(),
                        placed: false,
                        suspended: false,
                        next: None,
                        clipboard: None,
                        files: None,
                        swipe,
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
                let focus_ancestors = self.scope_ancestors(self.node(*id)?.parent)?;
                self.unlink(*id);
                let mut pending = vec![*id];
                let mut focus_removed = 0usize;
                while let Some(id) = pending.pop() {
                    let node = self.nodes.remove(&id).expect("owned subtree");
                    self.focus_scopes.remove(&id);
                    focus_removed += 1;
                    self.layers -= usize::from(layer(node.descriptor.view()));
                    if let Some(name) = node.descriptor.view().semantic_id() {
                        self.names.remove(name);
                    }
                    self.generations
                        .get_mut(&id.slot())
                        .expect("known identity")
                        .1 = false;
                    pending.extend(node.children);
                    self.stats.disposed += 1;
                }
                for root in focus_ancestors {
                    *self
                        .focus_scopes
                        .get_mut(&root)
                        .expect("surviving scope ancestor") -= focus_removed;
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
        let descriptor = self.node(node)?.descriptor.view();
        if enabled {
            kind.validate_target(descriptor)?;
        }
        Ok(())
    }
    fn action(&mut self, action: Action) -> Result<(), Error> {
        action.validate_target(&self.node(action.node())?.descriptor)?;
        self.validate_clipboard_action(action)?;
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
