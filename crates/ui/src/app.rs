use std::any::Any;
use std::cell::{Cell, Ref, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::{Rc, Weak};

use glory_core::{AccessError, Host, InstanceId, Runtime, RuntimeStats};
use glory_render::memory::{Memory, Subtree};
use glory_render::profile::{Phase, Profile};
use glory_render::{
    Action, Descriptor, Error as RenderError, Event, EventKind, Input, Mutation, NodeId, NodeIds,
    Patch, Query, QueryToken, QueryValue, Render, RuntimeId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Access(AccessError),
    Render(RenderError),
    List(crate::list::Error),
    Gone,
    Faulted,
    SettleLimit { requested: usize, max: usize },
}
impl From<AccessError> for Error {
    fn from(error: AccessError) -> Self {
        Self::Access(error)
    }
}
impl From<RenderError> for Error {
    fn from(error: RenderError) -> Self {
        Self::Render(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T = ()> = std::result::Result<T, Error>;

/// Result of a bounded local observation turn, not display completion or an
/// assertion that asynchronous tasks/resources/animations have finished.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settle {
    pub passes: usize,
    pub handled: usize,
    pub pending: bool,
}
type Handler = Rc<RefCell<Box<dyn FnMut(Event)>>>;
struct Subscription {
    owner: InstanceId,
    callback: Handler,
}
type Reply = Box<dyn FnOnce(std::result::Result<QueryValue, RenderError>)>;

struct PendingQuery {
    owner: InstanceId,
    node: NodeId,
    query: Query,
    applied: u64,
    reply: Reply,
    cancelled: bool,
}
#[derive(Default)]
struct Owned {
    nodes: BTreeSet<NodeId>,
    subscriptions: BTreeSet<(NodeId, EventKind)>,
    queries: BTreeSet<QueryToken>,
}
impl Owned {
    fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.subscriptions.is_empty() && self.queries.is_empty()
    }
}

struct Present {
    tree: Memory,
    ids: NodeIds,
    owners: HashMap<InstanceId, Owned>,
    node_owner_count: usize,
    query_owner_count: usize,
    node_owners: HashMap<NodeId, InstanceId>,
    disposal_boundaries: BTreeSet<NodeId>,
    handlers: HashMap<NodeId, BTreeMap<EventKind, Subscription>>,
    listeners: Vec<(NodeId, EventKind, bool)>,
    commands: Vec<Mutation>,
    actions: Vec<Action>,
    queries: HashMap<QueryToken, PendingQuery>,
    node_queries: HashMap<NodeId, BTreeSet<QueryToken>>,
    next_query: u64,
}

impl Present {
    fn register_nodes(&mut self, owner: InstanceId, nodes: &[NodeId]) {
        if nodes.is_empty() {
            return;
        }
        for &node in nodes {
            self.node_owners.insert(node, owner);
        }
        let owned = self.owners.entry(owner).or_default();
        let first = owned.nodes.is_empty();
        owned.nodes.extend(nodes.iter().copied());
        self.node_owner_count += usize::from(first);
    }

    fn take_owner(&mut self, owner: InstanceId) -> Option<Owned> {
        let owned = self.owners.remove(&owner)?;
        self.node_owner_count -= usize::from(!owned.nodes.is_empty());
        self.query_owner_count -= usize::from(!owned.queries.is_empty());
        Some(owned)
    }

    fn unlink_subscription(&mut self, owner: InstanceId, node: NodeId, kind: EventKind) {
        if let Some(owned) = self.owners.get_mut(&owner) {
            owned.subscriptions.remove(&(node, kind));
            if owned.is_empty() {
                self.owners.remove(&owner);
            }
        }
    }

    /// Return the old capture so its destructor runs outside the Present borrow.
    fn register_subscription(
        &mut self,
        node: NodeId,
        kind: EventKind,
        subscription: Subscription,
    ) -> Option<Subscription> {
        let owner = subscription.owner;
        let previous = self
            .handlers
            .entry(node)
            .or_default()
            .insert(kind, subscription);
        if let Some(previous) = &previous {
            self.unlink_subscription(previous.owner, node, kind);
        } else {
            self.listeners.push((node, kind, true));
        }
        self.owners
            .entry(owner)
            .or_default()
            .subscriptions
            .insert((node, kind));
        previous
    }

    fn remove_subscription(&mut self, node: NodeId, kind: EventKind) -> Option<Subscription> {
        let subscription = self.handlers.get_mut(&node)?.remove(&kind)?;
        if self.handlers.get(&node).is_some_and(BTreeMap::is_empty) {
            self.handlers.remove(&node);
        }
        self.unlink_subscription(subscription.owner, node, kind);
        self.listeners.push((node, kind, false));
        Some(subscription)
    }

    fn register_query(&mut self, token: QueryToken, pending: PendingQuery) {
        self.node_queries
            .entry(pending.node)
            .or_default()
            .insert(token);
        let owned = self.owners.entry(pending.owner).or_default();
        let first = owned.queries.is_empty();
        owned.queries.insert(token);
        self.query_owner_count += usize::from(first);
        self.queries.insert(token, pending);
    }

    fn remove_query(&mut self, token: QueryToken) -> Option<PendingQuery> {
        let pending = self.queries.remove(&token)?;
        if let Some(tokens) = self.node_queries.get_mut(&pending.node) {
            tokens.remove(&token);
            if tokens.is_empty() {
                self.node_queries.remove(&pending.node);
            }
        }
        if let Some(owned) = self.owners.get_mut(&pending.owner) {
            owned.queries.remove(&token);
            if owned.queries.is_empty() {
                self.query_owner_count -= 1;
            }
            if owned.is_empty() {
                self.owners.remove(&pending.owner);
            }
        }
        Some(pending)
    }

    /// Memory supplies the actual retired identities. This only unlinks their
    /// presentation interests; it never walks the tree or drops user captures.
    fn retire_node(
        &mut self,
        node: NodeId,
        retiring_owner: InstanceId,
        discarded: &mut Vec<Handler>,
        cancelled: &mut Vec<QueryToken>,
    ) {
        self.ids.release(node);
        if !self.disposal_boundaries.is_empty() {
            self.disposal_boundaries.remove(&node);
        }
        let node_owner = self.node_owners.remove(&node);
        let handlers = self.handlers.remove(&node);
        // The retiring Owner's record may already be extracted. Occupied entry
        // avoids reserving insertion capacity when no record remains.
        if let Some(node_owner) = node_owner
            && node_owner != retiring_owner
            && let std::collections::hash_map::Entry::Occupied(mut entry) =
                self.owners.entry(node_owner)
        {
            let owned = entry.get_mut();
            owned.nodes.remove(&node);
            let empty_nodes = owned.nodes.is_empty();
            if let Some(handlers) = &handlers {
                for (kind, subscription) in handlers {
                    if subscription.owner == node_owner {
                        owned.subscriptions.remove(&(node, *kind));
                    }
                }
            }
            if owned.is_empty() {
                entry.remove();
            }
            self.node_owner_count -= usize::from(empty_nodes);
        }
        if let Some(handlers) = handlers {
            for (kind, subscription) in handlers {
                if Some(subscription.owner) != node_owner {
                    self.unlink_subscription(subscription.owner, node, kind);
                }
                discarded.push(subscription.callback);
            }
        }
        if let Some(tokens) = self.node_queries.remove(&node) {
            for token in tokens {
                if let Some(query) = self.queries.get_mut(&token) {
                    query.cancelled = true;
                    cancelled.push(token);
                }
            }
        }
    }
}

pub(crate) struct Session {
    command_buffer: crate::buffer::Pool<Mutation>,
    action_buffer: crate::buffer::Pool<Action>,
    listener_buffer: crate::buffer::Pool<(NodeId, EventKind, bool)>,
    input_buffer: crate::buffer::Pool<Input>,
    pub runtime: Runtime,
    present: RefCell<Present>,
    renderer: RefCell<Box<dyn Render>>,
    faulted: Cell<bool>,
    error: RefCell<Option<Error>>,
    style_profile: glory_style::Profile,
    applied: Cell<u64>,
    cancelled_queries: RefCell<Vec<QueryToken>>,
}

impl Session {
    fn new<R: Render + 'static>(
        runtime: Runtime,
        factory: impl FnOnce(RuntimeId) -> R,
    ) -> Rc<Self> {
        let id = runtime.id();
        let renderer: Box<dyn Render> = Box::new(factory(id));
        let style_profile = renderer.style_profile();
        Rc::new(Self {
            runtime,
            present: RefCell::new(Present {
                tree: Memory::new(id),
                ids: NodeIds::new(id),
                owners: HashMap::new(),
                node_owner_count: 0,
                query_owner_count: 0,
                node_owners: HashMap::new(),
                disposal_boundaries: BTreeSet::new(),
                handlers: HashMap::new(),
                listeners: Vec::new(),
                commands: Vec::new(),
                actions: Vec::new(),
                queries: HashMap::new(),
                node_queries: HashMap::new(),
                next_query: 1,
            }),
            renderer: RefCell::new(renderer),
            style_profile,
            applied: Cell::new(0),
            faulted: Cell::new(false),
            error: RefCell::new(None),
            cancelled_queries: RefCell::new(Vec::new()),
            command_buffer: Default::default(),
            action_buffer: Default::default(),
            listener_buffer: Default::default(),
            input_buffer: Default::default(),
        })
    }

    pub fn report(&self, error: Error) {
        self.error.borrow_mut().get_or_insert(error);
    }
    fn check_style(&self, style: &glory_style::Style) -> Result {
        self.style_profile
            .inline
            .check(style)
            .map_err(|requirement| {
                RenderError::UnsupportedStyle {
                    profile: self.style_profile.name,
                    version: self.style_profile.version,
                    requirement,
                }
                .into()
            })
    }
    pub fn patch(&self, id: NodeId, patch: Patch) -> Result {
        if self.faulted.get() {
            return Err(Error::Faulted);
        }
        let _span = self
            .runtime
            .profile()
            .map(|profile| profile.span(Phase::Patch));
        let mut present = self.present.borrow_mut();
        present.tree.node(id)?;
        if let Patch::Style(glory_render::Change::Set(style)) = &patch {
            style.validate().map_err(RenderError::InvalidProps)?;
            self.check_style(style)?;
        }
        self.renderer
            .borrow()
            .preflight(present.tree.node(id)?.descriptor.view(), Some(&patch))?;
        let mutation = Mutation::Patch { id, patch };
        present.tree.apply(std::slice::from_ref(&mutation))?;
        present.commands.push(mutation);
        Ok(())
    }

    pub(crate) fn input_value(&self, id: NodeId, value: String) -> Result {
        let revision = {
            let present = self.present.borrow();
            let glory_render::Props::Input(input) = &present.tree.node(id)?.descriptor.view().props
            else {
                return Err(RenderError::InvalidProps("value requires an input").into());
            };
            input
                .revision
                .checked_add(1)
                .ok_or(RenderError::InputRevisionExhausted)?
        };
        self.patch(
            id,
            Patch::Value {
                value: glory_render::Change::Set(value),
                revision,
            },
        )
    }

    pub(crate) fn rich_document(
        &self,
        id: NodeId,
        document: Rc<glory_render::RichDocument>,
    ) -> Result {
        document.validate()?;
        let revision = {
            let present = self.present.borrow();
            let glory_render::Props::RichEditor(editor) =
                present.tree.node(id)?.descriptor.view().props
            else {
                return Err(RenderError::Unsupported("rich document requires a RichEditor").into());
            };
            editor
                .revision
                .checked_add(1)
                .ok_or(RenderError::InputRevisionExhausted)?
        };
        self.patch(
            id,
            Patch::RichDocument {
                document: glory_render::Change::Set(document),
                revision,
            },
        )
    }

    pub(crate) fn select_selection(
        &self,
        id: NodeId,
        keys: Vec<glory_render::SemanticId>,
    ) -> Result {
        let revision = {
            let present = self.present.borrow();
            let glory_render::Props::Select(select) =
                present.tree.node(id)?.descriptor.view().props
            else {
                return Err(RenderError::InvalidProps("selection requires a select").into());
            };
            select
                .selection
                .revision
                .checked_add(1)
                .ok_or(RenderError::SelectRevisionExhausted)?
        };
        self.patch(
            id,
            Patch::SelectSelection {
                keys: glory_render::Change::Set(keys),
                revision,
            },
        )
    }

    pub(crate) fn image_source(&self, id: NodeId, source: String) -> Result {
        let revision = {
            let present = self.present.borrow();
            let glory_render::Props::Image(image) = present.tree.node(id)?.descriptor.view().props
            else {
                return Err(RenderError::InvalidProps("source requires an image").into());
            };
            image
                .revision
                .checked_add(1)
                .ok_or(RenderError::ImageRevisionExhausted)?
        };
        self.patch(
            id,
            Patch::Source {
                source: glory_render::Change::Set(source),
                revision,
            },
        )
    }

    fn place(&self, id: NodeId, parent: Option<NodeId>, before: Option<NodeId>) -> Result {
        let mut present = self.present.borrow_mut();
        let node = present.tree.node(id)?;
        if node.placed
            && !node.suspended
            && node.parent == parent
            && present.tree.next_sibling(id)? == before
        {
            return Ok(());
        }
        let mutation = if node.suspended {
            Mutation::Resume { id, parent, before }
        } else {
            Mutation::Place { id, parent, before }
        };
        present
            .tree
            .validate_placement(id, parent, before, node.suspended)?;
        self.renderer
            .borrow()
            .preflight_place(&present.tree, Subtree::Existing(id), parent)?;
        present.tree.apply(std::slice::from_ref(&mutation))?;
        present.commands.push(mutation);
        Ok(())
    }

    fn dispose_owner(&self, owner: InstanceId) {
        let mut discarded = Vec::new();
        let mut discarded_queries = Vec::new();
        let mut cancelled = Vec::new();
        {
            let mut present = self.present.borrow_mut();
            // One record covers all presentation interests of this Owner.
            // Ancestor cleanup may already have removed its nodes/listeners;
            // an empty Scope needs no further hash lookups or tree work.
            let Some(owned) = present.take_owner(owner) else {
                return;
            };
            {
                for token in owned.queries {
                    if let Some(query) = present.remove_query(token) {
                        discarded_queries.push(query);
                    }
                }
            }
            {
                for (node, kind) in owned.subscriptions {
                    if let Some(subscription) = present.remove_subscription(node, kind) {
                        discarded.push(subscription.callback);
                    }
                }
            }
            for mut id in owned.nodes {
                if !present.ids.contains(id) {
                    continue;
                }
                // Core retires the complete Owner subtree before running any
                // cleanup callbacks. Remove the highest already-retired
                // semantic ancestor once, rather than disposing every leaf
                // separately. A live parent (even under a different Scope) is
                // a hard boundary. Later cleanup callbacks still run in their
                // original order; released identities cannot be removed twice.
                while let Some(parent) = present.tree.node(id).expect("owned node").parent {
                    let Some(parent_owner) = present.node_owners.get(&parent) else {
                        break;
                    };
                    if parent_owner.is_alive() {
                        break;
                    }
                    // An explicit boundary must survive until its own Owner's
                    // cleanup callback can move live foreign-owned ranges out.
                    // Ordinary disposal keeps its empty-set fast path.
                    if *parent_owner != owner
                        && !present.disposal_boundaries.is_empty()
                        && present.disposal_boundaries.contains(&parent)
                    {
                        break;
                    }
                    id = parent;
                }
                let (_, retired) = present.tree.dispose(id).expect("owned semantic subtree");
                for child in retired {
                    present.retire_node(child, owner, &mut discarded, &mut cancelled);
                }
                present.commands.push(Mutation::Dispose { id });
            }
        }
        self.cancelled_queries.borrow_mut().extend(cancelled);
        drop(discarded);
        drop(discarded_queries);
    }

    fn commit(&self) -> Result {
        if self.faulted.get() {
            return Err(Error::Faulted);
        }
        let mut commands = self.command_buffer.take();
        std::mem::swap(&mut *commands, &mut self.present.borrow_mut().commands);
        if !commands.is_empty() {
            let _span = self
                .runtime
                .profile()
                .map(|profile| profile.span(Phase::Commit).work(commands.len() as u64));
            let receipt = match self.renderer.borrow_mut().apply(&commands) {
                Ok(receipt) => receipt,
                Err(error) => {
                    self.faulted.set(true);
                    return Err(error.into());
                }
            };
            let previous = self.applied.get();
            if receipt.applied <= previous {
                self.faulted.set(true);
                return Err(RenderError::InvalidReceipt {
                    previous,
                    applied: receipt.applied,
                }
                .into());
            }
            self.applied.set(receipt.applied);
        }
        let mut actions = self.action_buffer.take();
        std::mem::swap(&mut *actions, &mut self.present.borrow_mut().actions);
        let mut listeners = self.listener_buffer.take();
        std::mem::swap(&mut *listeners, &mut self.present.borrow_mut().listeners);
        for (node, kind, enabled) in listeners.drain(..) {
            if !self.present.borrow().ids.contains(node) {
                continue;
            }
            if let Err(error) = self.renderer.borrow_mut().listen(node, kind, enabled) {
                self.report(error.into());
            }
        }
        for action in actions.drain(..) {
            let present = self.present.borrow();
            if let Action::PickFiles {
                node,
                revision,
                request,
            } = action
            {
                if !present
                    .tree
                    .file_pending(node, request, revision)
                    .unwrap_or(false)
                {
                    continue;
                }
                if present.tree.validate_file_action(action).is_err() {
                    drop(present);
                    if let Some(files) = self.renderer.borrow().capabilities().files {
                        self.present.borrow_mut().tree.push_event(
                            node,
                            file_picker_failure(files.document, revision, request),
                        )?;
                        self.runtime.request_observe();
                    }
                    continue;
                }
            }
            if let Action::ClearFiles{node,revision} | Action::RemoveFile(glory_render::FileToken{node,revision,..})=action
                && !present.tree.node(node).is_ok_and(|node|matches!(node.descriptor.view().props,glory_render::Props::File(props) if props.revision==revision)) {continue;}
            if let Action::CopyText {
                node,
                revision,
                request,
            } = action
            {
                if !present
                    .tree
                    .clipboard_pending(node, request, revision)
                    .unwrap_or(false)
                {
                    continue;
                }
                if !present.tree.active(node)
                    || !present.tree.enabled(node)
                    || present.tree.inert(node)
                {
                    drop(present);
                    self.present.borrow_mut().tree.push_event(
                        node,
                        Event::Clipboard {
                            revision,
                            request,
                            outcome: glory_render::ClipboardOutcome::Failed,
                        },
                    )?;
                    self.runtime.request_observe();
                    continue;
                }
            }
            if let Action::SelectRich{node,revision,..}=action && !present.tree.node(node).is_ok_and(|node|matches!(node.descriptor.view().props,glory_render::Props::RichEditor(p) if p.revision==revision)){continue;}
            if let Action::SelectText {node,revision,..}=action
                && !present.tree.node(node).is_ok_and(|node|matches!(node.descriptor.view().props,glory_render::Props::Input(p) if p.revision==revision))
            { continue; }
            // Menu close and exit commit together. The retained panel may now
            // be hidden, but the host must still move focus past its trigger.
            if present.tree.node(action.node()).is_err()
                || (!present.tree.active(action.node())
                    && !matches!(action, Action::ExitMenu { .. }))
            {
                continue;
            }
            if matches!(
                action,
                Action::Focus(_)
                    | Action::SelectRich { .. }
                    | Action::RaiseLayer(_)
                    | Action::ScrollIntoView(_)
                    | Action::ExitMenu { .. }
            ) && present.tree.inert(action.node())
            {
                continue;
            }
            drop(present);
            let _span = self
                .runtime
                .profile()
                .map(|profile| profile.span(Phase::Action));
            let file_document = if matches!(action, Action::PickFiles { .. }) {
                self.renderer
                    .borrow()
                    .capabilities()
                    .files
                    .map(|files| files.document)
            } else {
                None
            };
            if let Err(error) = self.renderer.borrow_mut().action(action) {
                if let Action::PickFiles {
                    node,
                    revision,
                    request,
                } = action
                    && let Some(document) = file_document
                {
                    self.present
                        .borrow_mut()
                        .tree
                        .push_event(node, file_picker_failure(document, revision, request))?;
                    self.runtime.request_observe();
                    continue;
                }

                if let Action::CopyText {
                    node,
                    revision,
                    request,
                } = action
                {
                    self.present.borrow_mut().tree.push_event(
                        node,
                        Event::Clipboard {
                            revision,
                            request,
                            outcome: glory_render::ClipboardOutcome::Failed,
                        },
                    )?;
                    self.runtime.request_observe();
                    continue;
                }
                // Focus can become unavailable after queuing (CSS/layout,
                // disabling, host policy). That ordinary refusal does not
                // desynchronize the semantic tree. Focus is a best-effort
                // intent; Query::Focus observes the actual result. Retain later
                // intents; actual backend failures remain fatal.
                if matches!(
                    action,
                    Action::Focus(_)
                        | Action::RaiseLayer(_)
                        | Action::ScrollIntoView(_)
                        | Action::SelectText { .. }
                        | Action::SelectRich { .. }
                ) && matches!(error, RenderError::Unsupported(_))
                {
                    continue;
                }
                self.faulted.set(true);
                return Err(error.into());
            }
        }
        Ok(())
    }

    fn route(&self, node: NodeId, event: Event) -> Result<bool> {
        let default_form_action = match &event {
            Event::Activate => Some(false),
            Event::Keyboard {
                command: glory_render::KeyboardCommand::Submit,
                ..
            } => Some(true),
            _ => None,
        };
        // Suspension preserves the scope/observers. Terminal focus/composition
        // loss still releases pins/editing after its physical range detaches.
        // Disposal remains guarded by generational identity and live handlers.
        let terminal_loss = event.is_terminal_loss();
        let (handlers, observed_tree) = {
            let mut present = self.present.borrow_mut();
            if !present.ids.contains(node) || !present.tree.active(node) && !terminal_loss {
                return Ok(false);
            }
            let props = &present.tree.node(node)?.descriptor.view().props;
            if matches!(event, Event::Activate)
                && !present
                    .tree
                    .node(node)?
                    .descriptor
                    .view()
                    .activation_enabled()
            {
                return Ok(false);
            }
            if let Event::ValueChanged { revision, .. }
            | Event::Commit { revision, .. }
            | Event::Tags { revision, .. } = &event
                && let glory_render::Props::Input(input) = props
                && *revision < input.revision
            {
                return Ok(false);
            }
            if let Event::InputEdit(edit) = &event
                && let glory_render::Props::Input(input) = props
                && edit.revision() < input.revision
            {
                return Ok(false);
            }
            if let Event::Sheet { revision, .. } = &event
                && present
                    .tree
                    .node(node)?
                    .descriptor
                    .view()
                    .semantics
                    .as_ref()
                    .and_then(|s| s.layer.as_ref())
                    .and_then(|l| l.sheet.as_ref())
                    .is_some_and(|s| *revision < s.revision)
            {
                return Ok(false);
            }
            if let Event::SelectChanged { revision, .. } = &event
                && let glory_render::Props::Select(select) = props
                && *revision < select.selection.revision
            {
                return Ok(false);
            }
            if let Event::ImageStatus { revision, .. } = &event
                && let glory_render::Props::Image(image) = props
                && *revision < image.revision
            {
                return Ok(false);
            }
            if !present.tree.validate_event(node, &event)? {
                return Ok(false);
            }
            if let Event::Files(result) = &event {
                let files = self.renderer.borrow().capabilities().files.ok_or(
                    RenderError::InvalidEvent("file observation without a file consumer"),
                )?;
                if !present.tree.finish_files(node, files.document, result)? {
                    return Ok(false);
                }
            }
            if let Event::Clipboard {
                request, revision, ..
            } = event
                && !present.tree.finish_clipboard(node, request, revision)?
            {
                return Ok(false);
            }
            if !present.tree.enabled(node)
                && !event.is_terminal_loss()
                && matches!(
                    event,
                    Event::Activate
                        | Event::Submit { .. }
                        | Event::Reset
                        | Event::Keyboard { .. }
                        | Event::Palette { .. }
                        | Event::Typeahead { .. }
                        | Event::Menu { .. }
                        | Event::ContextMenu { .. }
                        | Event::NavigationPanel { .. }
                        | Event::GridNavigate { .. }
                        | Event::TreeNavigate { .. }
                        | Event::Sheet { .. }
                        | Event::Choice { .. }
                        | Event::Drag { .. }
                        | Event::Range { .. }
                        | Event::Spin { .. }
                        | Event::ValueChanged { .. }
                        | Event::InputEdit(_)
                        | Event::CheckedChanged(_)
                        | Event::SelectChanged { .. }
                        | Event::Commit { .. }
                )
            {
                return Ok(false);
            }
            let mut cursor = Some(node);
            let mut handler = None;
            let mut nested_control = false;
            while let Some(id) = cursor {
                let current = present.tree.node(id)?;
                // A real tree item can contain editable/action controls. Their
                // clicks keep native policy even without a local app handler;
                // labels and other passive content still activate their item.
                if matches!(event, Event::Activate) {
                    if id != node
                        && current.descriptor.view().role == Some(glory_render::Role::TreeItem)
                        && nested_control
                    {
                        break;
                    }
                    nested_control |= matches!(
                        current.descriptor.view().props,
                        glory_render::Props::Input(_)
                            | glory_render::Props::Button(_)
                            | glory_render::Props::Check(_)
                            | glory_render::Props::Link(_)
                            | glory_render::Props::Select(_)
                            | glory_render::Props::RichEditor(_)
                    );
                }
                if matches!(
                    event,
                    Event::Activate
                        | Event::Keyboard { .. }
                        | Event::Palette { .. }
                        | Event::Typeahead { .. }
                        | Event::Menu { .. }
                        | Event::NavigationPanel { .. }
                        | Event::Choice { .. }
                ) && !current.descriptor.view().props.enabled()
                {
                    return Ok(false);
                }
                if handler.is_none() {
                    handler = present
                        .handlers
                        .get(&id)
                        .and_then(|handlers| handlers.get(&event.kind()))
                        .map(|subscription| (id, subscription.callback.clone()));
                }
                if !matches!(
                    event,
                    Event::Activate
                        | Event::Navigate { .. }
                        | Event::Keyboard { .. }
                        | Event::Palette { .. }
                        | Event::Typeahead { .. }
                        | Event::Menu { .. }
                        | Event::NavigationPanel { .. }
                        | Event::Choice { .. }
                ) {
                    break;
                }
                cursor = current.parent;
            }
            let mut handlers = Vec::new();
            let focused = if let Event::FocusChanged(focused) = &event {
                Some(*focused)
            } else {
                None
            };
            let pointer = if let Event::Pointer {
                from,
                related,
                phase,
            } = &event
            {
                Some((*from, *related, *phase))
            } else {
                None
            };
            if let Some((id, handler)) = handler {
                handlers.push((id, event.kind(), handler, event));
            }
            if let Some(focused) = focused {
                // Observation is broadcast along the semantic ancestry. A
                // control's direct listener must not hide focus from an outer
                // virtual list, nor may physical Group anchors change it.
                let mut cursor = Some(node);
                while let Some(id) = cursor {
                    if let Some(subscription) = present
                        .handlers
                        .get(&id)
                        .and_then(|handlers| handlers.get(&EventKind::FocusWithin))
                    {
                        handlers.push((
                            id,
                            EventKind::FocusWithin,
                            subscription.callback.clone(),
                            Event::FocusWithin { node, focused },
                        ));
                    }
                    cursor = present.tree.node(id)?.parent;
                }
            }
            if let Some((from, related, phase)) = pointer {
                // A boundary ends at the first common physical ancestry.
                // Each chain is walked once, with no stored visit history.
                let mut shared = BTreeSet::new();
                let mut cursor = related;
                while let Some(id) = cursor {
                    shared.insert(id);
                    cursor = present.tree.node(id)?.parent;
                }
                let mut cursor = Some(from);
                while let Some(id) = cursor {
                    if shared.contains(&id) {
                        break;
                    }
                    if let Some(subscription) = present
                        .handlers
                        .get(&id)
                        .and_then(|handlers| handlers.get(&EventKind::PointerWithin))
                    {
                        handlers.push((
                            id,
                            EventKind::PointerWithin,
                            subscription.callback.clone(),
                            Event::PointerWithin {
                                from,
                                related,
                                phase,
                            },
                        ));
                    }
                    cursor = present.tree.node(id)?.parent;
                }
            }
            (handlers, present.tree.stats())
        };
        let handled = !handlers.is_empty();
        for (id, kind, handler, event) in handlers {
            let owner = {
                let present = self.present.borrow();
                if (present.tree.active(id) || terminal_loss)
                    && !(event.is_user_interaction()
                        && present.tree.stats() != observed_tree
                        && !present.tree.validate_event(id, &event).unwrap_or(false))
                {
                    present
                        .handlers
                        .get(&id)
                        .and_then(|handlers| handlers.get(&kind))
                        .filter(|subscription| Rc::ptr_eq(&subscription.callback, &handler))
                        .map(|subscription| subscription.owner)
                } else {
                    None
                }
            };
            if let Some(owner) = owner {
                // An earlier direct/ancestor callback may have removed or moved
                // the focus origin. Only re-walk ancestry after tree changes;
                // the normal focus path stays linear in logical depth.
                if matches!(
                    event,
                    Event::FocusWithin { .. } | Event::PointerWithin { .. }
                ) {
                    let present = self.present.borrow();
                    if present.tree.stats() != observed_tree
                        && !present.tree.validate_event(id, &event).unwrap_or(false)
                    {
                        continue;
                    }
                }
                match self.runtime.enter(owner, || handler.borrow_mut()(event)) {
                    Ok(()) | Err(AccessError::Disposed) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        let request=default_form_action.and_then(|keyboard| {
            let present=self.present.borrow();
            let source=present.tree.node(node).ok()?;
            if !present.tree.active(node) || !present.tree.enabled(node) { return None; }
            let action=if keyboard {
                if !matches!(&source.descriptor.view().props,glory_render::Props::Input(input) if !input.multiline) { return None; }
                glory_render::FormAction::Submit
            } else {
                source.descriptor.view().semantics.as_ref()?.form_action?
            };
            let mut cursor=source.parent;
            while let Some(id)=cursor {
                let current=present.tree.node(id).ok()?;
                if current.descriptor.view().field_part()==Some(glory_render::FieldPart::Form) {
                    return Some((id,match action {
                        glory_render::FormAction::Submit=>Event::Submit { submitter:(!keyboard).then_some(node) },
                        glory_render::FormAction::Reset=>Event::Reset,
                    }));
                }
                cursor=current.parent;
            }
            None
        });
        if let Some((form, event)) = request {
            return Ok(self.route(form, event)? || handled);
        }
        Ok(handled)
    }

    fn replies(&self) -> usize {
        let cancelled = std::mem::take(&mut *self.cancelled_queries.borrow_mut());
        let count = cancelled.len();
        for token in cancelled {
            let pending = self.present.borrow_mut().remove_query(token);
            if let Some(pending) = pending {
                let node = pending.node;
                self.reply(pending, Err(RenderError::NodeGone(node)));
            }
        }
        count
    }

    fn reply(&self, pending: PendingQuery, result: std::result::Result<QueryValue, RenderError>) {
        if let Err(error) = glory_core::untracked(|| {
            self.runtime
                .enter(pending.owner, || (pending.reply)(result))
        }) && error != AccessError::Disposed
        {
            self.report(error.into());
        }
    }

    fn flush(&self) -> Result {
        let mut settled = false;
        for _ in 0..1024 {
            self.runtime.flush()?;
            self.commit()?;
            if self.replies() == 0 {
                settled = true;
                break;
            }
        }
        if !settled {
            return Err(AccessError::RunawayEffect.into());
        }
        if let Some(error) = self.error.borrow_mut().take() {
            return Err(error);
        }
        Ok(())
    }

    fn request(&self, owner: InstanceId, query: Query, reply: Reply) -> Result<QueryToken> {
        self.runtime.enter(owner, || ())?;
        self.flush()?; // Mandatory query barrier; it may dispose the requester.
        self.runtime.enter(owner, || ())?;
        let token = {
            let mut present = self.present.borrow_mut();
            present.tree.node(query.node())?;
            let token = QueryToken(present.next_query);
            present.next_query = present
                .next_query
                .checked_add(1)
                .expect("query identity exhausted");
            token
        };
        self.renderer.borrow_mut().request(query, token)?;
        {
            let mut present = self.present.borrow_mut();
            present.register_query(
                token,
                PendingQuery {
                    owner,
                    node: query.node(),
                    query,
                    applied: self.applied.get(),
                    reply,
                    cancelled: false,
                },
            );
        }
        Ok(token)
    }
}

/// A UI context carries semantic parent identity, never a DOM/GPU object.
/// Core ownership remains separate; all presentation helpers work on any Render.
#[derive(Clone)]
pub struct Scope(Rc<ScopeState>);
struct ScopeState {
    core: glory_core::Scope,
    session: Weak<Session>,
    parent: Option<NodeId>,
    roots: RefCell<Vec<NodeId>>,
    children: RefCell<Vec<Mounted>>,
    resources: RefCell<Vec<Box<dyn Any>>>,
}
pub struct WeakScope(Weak<ScopeState>);
impl Clone for WeakScope {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl WeakScope {
    pub fn upgrade(&self) -> Option<Scope> {
        self.0
            .upgrade()
            .filter(|scope| scope.core.is_alive())
            .map(Scope)
    }
}

impl Scope {
    pub fn instantiate(&self, plan: &crate::Plan) -> Result<crate::PlanInstance> {
        let session = self.session()?;
        {
            let present = session.present.borrow();
            present
                .tree
                .validate_child(Subtree::Plan(&plan.semantic), self.0.parent)?;
            let renderer = session.renderer.borrow();
            for node in plan.semantic.nodes() {
                session.check_style(node.descriptor.view().style)?;
                renderer.preflight(node.descriptor.view(), None)?;
            }
            renderer.preflight_place(
                &present.tree,
                Subtree::Plan(&plan.semantic),
                self.0.parent,
            )?;
        }
        let ids = {
            let mut present = session.present.borrow_mut();
            let ids: Vec<_> = plan
                .semantic
                .nodes()
                .iter()
                .map(|_| present.ids.allocate())
                .collect();
            let mutation = Mutation::CreatePlan {
                plan: plan.semantic.clone(),
                ids: ids.clone(),
            };
            if let Err(error) = present.tree.apply(std::slice::from_ref(&mutation)) {
                for id in ids {
                    present.ids.release(id);
                }
                return Err(error.into());
            }
            let owner = self.id();
            present.register_nodes(owner, &ids);
            present.commands.push(mutation);
            ids
        };
        self.0.roots.borrow_mut().push(ids[0]);
        session.place(ids[0], self.0.parent, None)?;
        Ok(crate::PlanInstance {
            scope: self.downgrade(),
            plan: plan.semantic.clone(),
            ids,
        })
    }
    pub fn mount_with<W: Widget>(&self, factory: impl FnOnce() -> W) -> Result<Mounted> {
        self.mount_at(self.0.parent, factory)
    }
    fn mount_at<W: Widget>(
        &self,
        parent: Option<NodeId>,
        factory: impl FnOnce() -> W,
    ) -> Result<Mounted> {
        let _span = self
            .session()?
            .runtime
            .profile()
            .map(|profile| profile.span(Phase::Build));
        let scope = Self::new(self.0.core.child(), &self.session()?, parent)?;
        let widget = glory_core::untracked(|| {
            scope.owner().enter(|| {
                let mut widget = factory();
                widget.build(&scope)?;
                Ok::<_, Error>(widget)
            })
        })?;
        let roots = scope.roots();
        scope.retain(widget)?;
        Ok(Mounted { scope, roots })
    }
    pub(crate) fn fill(&self, node: NodeId, child: Box<dyn Widget>) -> Result {
        self.session()?.present.borrow().tree.node(node)?;
        let mounted = self.mount_at(Some(node), || child)?;
        // Content has a different semantic parent, so it is retained for
        // ownership but must not join this Scope's movable root range.
        self.retain(mounted)
    }
    fn new(core: glory_core::Scope, session: &Rc<Session>, parent: Option<NodeId>) -> Result<Self> {
        let scope = Self(Rc::new(ScopeState {
            core,
            session: Rc::downgrade(session),
            parent,
            roots: RefCell::new(Vec::new()),
            children: RefCell::new(Vec::new()),
            resources: RefCell::new(Vec::new()),
        }));
        let weak = Rc::downgrade(&scope.0);
        let session = Rc::downgrade(session);
        let id = scope.id();
        scope.on_cleanup(move || {
            if let Some(session) = session.upgrade() {
                session.dispose_owner(id);
            }
            if let Some(scope) = weak.upgrade() {
                let children = std::mem::take(&mut *scope.children.borrow_mut());
                let resources = std::mem::take(&mut *scope.resources.borrow_mut());
                scope.roots.borrow_mut().clear();
                drop(children);
                drop(resources);
            }
        })?;
        Ok(scope)
    }
    pub(crate) fn session(&self) -> Result<Rc<Session>> {
        if !self.0.core.is_alive() {
            return Err(Error::Gone);
        }
        let session = self.0.session.upgrade().ok_or(Error::Gone)?;
        if session.faulted.get() {
            return Err(Error::Faulted);
        }
        Ok(session)
    }
    pub(crate) fn weak(&self) -> WeakScope {
        WeakScope(Rc::downgrade(&self.0))
    }
    /// A callback may upgrade transiently without retaining its owning UI scope.
    pub fn downgrade(&self) -> WeakScope {
        self.weak()
    }
    /// Apply several writes as one observation boundary. Nested calls remain
    /// atomic and unwind releases the boundary. Host output still commits at
    /// the App's normal flush/event boundary.
    pub fn batch<R>(&self, callback: impl FnOnce() -> R) -> Result<R> {
        let session = self.session()?;
        Ok(session.runtime.batch(callback)?)
    }
    pub fn child(&self) -> Result<Self> {
        let session = self.session()?;
        Self::new(self.0.core.child(), &session, self.0.parent)
    }
    pub fn child_in(&self, parent: NodeId) -> Result<Self> {
        self.session()?.present.borrow().tree.node(parent)?;
        Self::new(self.0.core.child(), &self.session()?, Some(parent))
    }
    /// Whether this Scope's construction parent is the supplied live node or
    /// one of its staged descendants. Reads semantic ancestry only; hidden and
    /// suspended nodes remain valid, with no host query or commit barrier.
    pub fn is_within(&self, parent: NodeId) -> Result<bool> {
        let session = self.session()?;
        let present = session.present.borrow();
        present.tree.node(parent)?;
        let mut cursor = self.0.parent;
        while let Some(node) = cursor {
            let current = present.tree.node(node)?;
            if node == parent {
                return Ok(true);
            }
            cursor = current.parent;
        }
        Ok(false)
    }
    /// Whether the current staged semantic subtree rooted at `ancestor`
    /// contains `node`, including the ancestor itself. Both identities and
    /// this requesting Scope must still be live. Hidden, suspended and boxless
    /// nodes keep their logical ancestry; use `is_active` separately before a
    /// physical observation. No flush, host lookup or construction-parent
    /// assumption is involved, so explicit Place is visible immediately.
    pub fn contains_node(&self, ancestor: NodeId, node: NodeId) -> Result<bool> {
        let session = self.session()?;
        let present = session.present.borrow();
        present.tree.node(ancestor)?;
        present.tree.node(node)?;
        let mut cursor = Some(node);
        while let Some(current) = cursor {
            if current == ancestor {
                return Ok(true);
            }
            cursor = present.tree.node(current)?.parent;
        }
        Ok(false)
    }
    pub fn retain(&self, resource: impl Any) -> Result {
        self.session()?;
        self.0.resources.borrow_mut().push(Box::new(resource));
        Ok(())
    }
    pub fn create(&self, mut descriptor: Descriptor) -> Result<NodeId> {
        descriptor.normalize_input()?;
        descriptor.validate()?;
        let session = self.session()?;
        session.check_style(&descriptor.style)?;
        session
            .renderer
            .borrow()
            .preflight(descriptor.view(), None)?;
        let id = {
            let mut present = session.present.borrow_mut();
            present
                .tree
                .validate_child(Subtree::New(descriptor.view()), self.0.parent)?;
            session.renderer.borrow().preflight_place(
                &present.tree,
                Subtree::New(descriptor.view()),
                self.0.parent,
            )?;
            let id = present.ids.allocate();
            let mutation = Mutation::Create {
                id,
                node: Box::new(descriptor),
            };
            if let Err(error) = present.tree.apply(std::slice::from_ref(&mutation)) {
                present.ids.release(id);
                return Err(error.into());
            }
            present.register_nodes(self.id(), &[id]);
            present.commands.push(mutation);
            id
        };
        self.0.roots.borrow_mut().push(id);
        session.place(id, self.0.parent, None)?;
        Ok(id)
    }
    pub fn patch(&self, id: NodeId, patch: Patch) -> Result {
        self.session()?.patch(id, patch)
    }
    /// Move a live semantic range to an explicit parent, retaining its original
    /// Owner. Shared placement rules and adapter preflight run before changing
    /// the model or queue; suspended ranges resume through the same path.
    pub fn place(&self, node: NodeId, parent: Option<NodeId>, before: Option<NodeId>) -> Result {
        self.session()?.place(node, parent, before)
    }
    /// Read the accepted semantic link target/context without host I/O or a
    /// commit barrier. A rejected source update never changes this snapshot.
    pub fn link_request(&self, id: NodeId) -> Result<(String, glory_render::LinkOpen)> {
        let session = self.session()?;
        let present = session.present.borrow();
        let descriptor = present.tree.node(id)?.descriptor.view();
        let glory_render::Props::Link(link) = &descriptor.props else {
            return Err(RenderError::InvalidProps("link request requires a link").into());
        };
        Ok((
            link.target.clone(),
            descriptor
                .semantics
                .as_ref()
                .and_then(|s| s.link_open)
                .unwrap_or_default(),
        ))
    }
    pub fn action(&self, action: Action) -> Result {
        let session = self.session()?;
        action.validate_target(
            &session
                .present
                .borrow()
                .tree
                .node(action.node())?
                .descriptor,
        )?;
        if matches!(action, Action::SelectRich { .. })
            && (!session.present.borrow().tree.active(action.node())
                || !session.present.borrow().tree.enabled(action.node())
                || session.present.borrow().tree.inert(action.node()))
        {
            return Err(RenderError::Unsupported(
                "rich selection requires an active enabled non-inert editor",
            )
            .into());
        }
        if matches!(action, Action::CopyText { .. })
            && (!session.present.borrow().tree.enabled(action.node())
                || session.present.borrow().tree.inert(action.node())
                || !session.present.borrow().tree.active(action.node()))
        {
            return Err(RenderError::Unsupported(
                "clipboard source is inactive, disabled or inert",
            )
            .into());
        }
        if matches!(action, Action::RaiseLayer(_) | Action::ScrollIntoView(_))
            && (!session.present.borrow().tree.active(action.node())
                || session.present.borrow().tree.inert(action.node()))
        {
            return Err(RenderError::Unsupported("raise on an inactive layer").into());
        }
        if matches!(action, Action::Focus(_) | Action::RaiseLayer(_))
            && !session.present.borrow().tree.enabled(action.node())
        {
            return Err(RenderError::Unsupported("focus in disabled fieldset").into());
        }
        session.present.borrow().tree.validate_file_action(action)?;
        let capabilities = session.renderer.borrow().capabilities();
        match action {
            Action::PickFiles { .. } if !capabilities.files.is_some_and(|files| files.picker) => {
                return Err(RenderError::Unsupported("file picker").into());
            }
            Action::ClearFiles { .. } | Action::RemoveFile(_) if capabilities.files.is_none() => {
                return Err(RenderError::Unsupported("file access").into());
            }
            Action::CopyText { .. } if !capabilities.clipboard_write => {
                return Err(RenderError::Unsupported("text clipboard write").into());
            }
            Action::SelectRich { .. } | Action::SelectText { .. } if !capabilities.input => {
                return Err(RenderError::Unsupported("text selection").into());
            }
            Action::Focus(_) | Action::RaiseLayer(_) | Action::ExitMenu { .. }
                if !capabilities.focus =>
            {
                return Err(RenderError::Unsupported("focus").into());
            }
            Action::ScrollTo { .. } | Action::ScrollIntoView(_) | Action::RevealOption { .. }
                if !capabilities.scroll =>
            {
                return Err(RenderError::Unsupported("scroll").into());
            }
            _ => {}
        }
        let mut present = session.present.borrow_mut();
        present.tree.node(action.node())?;
        present.tree.admit_clipboard(action)?;
        present.tree.admit_file_action(action)?;
        present.actions.push(action);
        Ok(())
    }
    /// Validate a complete finite configuration before allocating a revision.
    /// Equal declarations preserve the current selection and request lifetime.
    pub fn set_file_configuration(
        &self,
        node: NodeId,
        configuration: glory_render::FileConfig,
    ) -> Result<u64> {
        configuration.validate()?;
        let session = self.session()?;
        let revision = {
            let present = session.present.borrow();
            let glory_render::Props::File(props) = present.tree.node(node)?.descriptor.view().props
            else {
                return Err(
                    RenderError::Unsupported("file configuration requires a File control").into(),
                );
            };
            if props.configuration == configuration {
                return Ok(props.revision);
            }
            props
                .revision
                .checked_add(1)
                .ok_or(RenderError::InvalidProps(
                    "file configuration revision exhausted",
                ))?
        };
        self.patch(
            node,
            Patch::File {
                configuration: glory_render::Change::Set(Box::new(configuration)),
                revision,
            },
        )?;
        Ok(revision)
    }
    pub fn clear_files(&self, node: NodeId) -> Result {
        let revision = {
            let session = self.session()?;
            let present = session.present.borrow();
            let glory_render::Props::File(props) = present.tree.node(node)?.descriptor.view().props
            else {
                return Err(RenderError::Unsupported("clear requires a File control").into());
            };
            props.revision
        };
        self.action(Action::ClearFiles { node, revision })
    }
    pub fn remove_file(&self, file: glory_render::FileToken) -> Result {
        self.action(Action::RemoveFile(file))
    }
    pub fn pick_files(&self, node: NodeId) -> Result<u64> {
        let session = self.session()?;
        let (revision, request) = {
            let present = session.present.borrow();
            let glory_render::Props::File(props) = present.tree.node(node)?.descriptor.view().props
            else {
                return Err(RenderError::Unsupported("file picker requires a File control").into());
            };
            (props.revision, present.tree.next_file_request(node)?)
        };
        self.action(Action::PickFiles {
            node,
            revision,
            request,
        })?;
        Ok(request)
    }
    /// Queue an explicit text write from this live Button's accepted source.
    /// The returned identity is strictly increasing for its NodeId lifetime.
    pub fn copy_text(&self, node: NodeId) -> Result<u64> {
        let session = self.session()?;
        let (revision, request) = {
            let present = session.present.borrow();
            let descriptor = present.tree.node(node)?.descriptor.view();
            let source = descriptor
                .semantics
                .as_ref()
                .and_then(|s| s.clipboard.as_ref())
                .ok_or(RenderError::Unsupported("button has no clipboard source"))?;
            (source.revision, present.tree.next_clipboard_request(node)?)
        };
        self.action(Action::CopyText {
            node,
            revision,
            request,
        })?;
        Ok(request)
    }
    /// Queue an exact UTF-16 selection using this Input's current program
    /// revision. Call after set_input_value when formatting changed the value.
    /// Surrogate interiors are invalid; a host may refuse subcluster positions.
    pub fn set_input_selection(
        &self,
        node: NodeId,
        selection: glory_render::TextSelection,
    ) -> Result {
        let session = self.session()?;
        let revision = {
            let present = session.present.borrow();
            let glory_render::Props::Input(input) =
                present.tree.node(node)?.descriptor.view().props
            else {
                return Err(RenderError::Unsupported("text selection requires an Input").into());
            };
            input.revision
        };
        self.action(Action::SelectText {
            node,
            revision,
            selection,
        })
    }
    /// Explicit program document write, even when equal, acknowledges or rejects
    /// a physical draft using a new revision from the live node.
    pub fn set_rich_document(
        &self,
        node: NodeId,
        document: Rc<glory_render::RichDocument>,
    ) -> Result<u64> {
        let session = self.session()?;
        session.rich_document(node, document)?;
        Ok(self.rich_editor_value(node)?.1)
    }
    /// Read staged accepted state, without a host query or flush.
    pub fn rich_editor_value(&self, node: NodeId) -> Result<(Rc<glory_render::RichDocument>, u64)> {
        let session = self.session()?;
        let present = session.present.borrow();
        let glory_render::Props::RichEditor(editor) =
            present.tree.node(node)?.descriptor.view().props
        else {
            return Err(RenderError::Unsupported("rich document requires a RichEditor").into());
        };
        Ok((editor.document.clone(), editor.revision))
    }
    pub fn rich_editor_editable(&self, node: NodeId) -> Result<bool> {
        let session = self.session()?;
        let present = session.present.borrow();
        let descriptor = present.tree.node(node)?.descriptor.view();
        if !matches!(descriptor.props, glory_render::Props::RichEditor(_)) {
            return Err(RenderError::Unsupported("rich edit requires a RichEditor").into());
        }
        Ok(present.tree.active(node)
            && present.tree.enabled(node)
            && !present.tree.inert(node)
            && !descriptor.readonly())
    }
    pub fn set_rich_selection(
        &self,
        node: NodeId,
        selection: glory_render::RichSelection,
    ) -> Result {
        let revision = self.rich_editor_value(node)?.1;
        self.action(Action::SelectRich {
            node,
            revision,
            selection,
        })
    }
    /// Flush before observing a host value. The reply executes under this
    /// Scope's Owner, independent of the target's Owner or the pump caller.
    /// Scope disposal drops its waiters without calling user code; a removed
    /// target completes a still-live request with NodeGone. No Scope is retained.
    pub fn request(
        &self,
        query: Query,
        reply: impl FnOnce(std::result::Result<QueryValue, RenderError>) + 'static,
    ) -> Result<QueryToken> {
        self.session()?.request(self.id(), query, Box::new(reply))
    }
    /// Revoke this Scope's pending reply without attempting to undo host work.
    /// Tokens are local to their App. Missing/completed tokens return false;
    /// another live Scope's token is rejected before either index is changed.
    pub fn cancel_request(&self, token: QueryToken) -> Result<bool> {
        let session = self.session()?;
        let pending = {
            let mut present = session.present.borrow_mut();
            match present.queries.get(&token) {
                None => return Ok(false),
                Some(query) if query.owner != self.id() => {
                    return Err(RenderError::InvalidProps(
                        "query cancellation requires its requesting Scope",
                    )
                    .into());
                }
                Some(_) => present.remove_query(token),
            }
        };
        let removed = pending.is_some();
        // Arbitrary captured destructors may write or reenter this registry.
        session
            .runtime
            .enter(self.id(), || glory_core::untracked(|| drop(pending)))?;
        Ok(removed)
    }
    pub fn capabilities(&self) -> Result<glory_render::Capabilities> {
        Ok(self.session()?.renderer.borrow().capabilities())
    }
    pub fn style_profile(&self) -> Result<glory_style::Profile> {
        Ok(self.session()?.style_profile)
    }
    pub fn is_active(&self, node: NodeId) -> Result<bool> {
        let session = self.session()?;
        let present = session.present.borrow();
        present.tree.node(node)?;
        Ok(present.tree.active(node))
    }
    /// Current semantic enabled ancestry, without a host query or flush. This
    /// is independent of placement/visibility, which `is_active` checks.
    pub fn is_enabled(&self, node: NodeId) -> Result<bool> {
        let session = self.session()?;
        let present = session.present.borrow();
        present.tree.node(node)?;
        Ok(present.tree.enabled(node))
    }
    /// Keep this owned node out of an earlier child's retired-ancestor
    /// coalescing. Its own Owner cleanup still disposes it normally. This lets
    /// that Owner move live foreign ranges before their semantic parent dies;
    /// it does not keep the Owner or its own descendants alive. The marker is
    /// reclaimed whenever the node is disposed, including ancestor disposal.
    pub fn defer_descendant_disposal(&self, node: NodeId) -> Result {
        let session = self.session()?;
        let mut present = session.present.borrow_mut();
        present.tree.node(node)?;
        if present.node_owners.get(&node) != Some(&self.id()) {
            return Err(RenderError::InvalidProps(
                "disposal boundary must belong to the registering Scope",
            )
            .into());
        }
        present.disposal_boundaries.insert(node);
        Ok(())
    }
    /// Current inherited inert state. This reads the logical live tree only.
    pub fn is_inert(&self, node: NodeId) -> Result<bool> {
        let session = self.session()?;
        let present = session.present.borrow();
        present.tree.node(node)?;
        Ok(present.tree.inert(node))
    }
    /// The immediate logical child of parent containing node. Retired focus
    /// targets are normal after removal; physical DOM ancestry is irrelevant.
    pub(crate) fn child_containing(&self, parent: NodeId, node: NodeId) -> Result<Option<NodeId>> {
        let session = self.session()?;
        let present = session.present.borrow();
        present.tree.node(parent)?;
        let mut cursor = node;
        while let Ok(current) = present.tree.node(cursor) {
            if current.parent == Some(parent) {
                return Ok(Some(cursor));
            }
            let Some(next) = current.parent else { break };
            cursor = next;
        }
        Ok(None)
    }
    pub fn on(
        &self,
        node: NodeId,
        kind: EventKind,
        handler: impl FnMut(Event) + 'static,
    ) -> Result {
        let session = self.session()?;
        let previous = {
            let mut present = session.present.borrow_mut();
            kind.validate_target(&present.tree.node(node)?.descriptor)?;
            present.register_subscription(
                node,
                kind,
                Subscription {
                    owner: self.id(),
                    callback: Rc::new(RefCell::new(Box::new(handler))),
                },
            )
        };
        drop(previous);
        Ok(())
    }
    fn roots(&self) -> Vec<NodeId> {
        let mut roots = self.0.roots.borrow().clone();
        for child in self.0.children.borrow().iter() {
            roots.extend(child.roots.iter().copied());
        }
        roots
    }
}
impl std::ops::Deref for Scope {
    type Target = glory_core::Scope;
    fn deref(&self) -> &Self::Target {
        &self.0.core
    }
}

/// A widget is built once; dynamic props/control flow install owned bindings.
pub trait Widget: 'static {
    #[doc(hidden)]
    fn static_plan(&mut self) -> Result<crate::Plan> {
        Err(RenderError::Unsupported("dynamic widget cannot be a static plan").into())
    }
    fn build(&mut self, scope: &Scope) -> Result;
    fn mount(self, parent: &Scope) -> Result<Mounted>
    where
        Self: Sized,
    {
        parent.mount_with(|| self)
    }
    fn show_in(self, parent: &Scope) -> Result
    where
        Self: Sized,
    {
        let mounted = self.mount(parent)?;
        parent.0.children.borrow_mut().push(mounted);
        Ok(())
    }
}

pub struct Mounted {
    scope: Scope,
    roots: Vec<NodeId>,
}
impl Mounted {
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }
    pub fn place_before(&self, before: Option<&Self>) -> Result {
        let session = self.scope.session()?;
        let before = before.and_then(|before| before.roots.first()).copied();
        for root in &self.roots {
            session.place(*root, self.scope.0.parent, before)?;
        }
        Ok(())
    }
    pub fn suspend(&self) -> Result {
        let session = self.scope.session()?;
        let mut present = session.present.borrow_mut();
        for root in &self.roots {
            let mutation = Mutation::Suspend { id: *root };
            present.tree.apply(std::slice::from_ref(&mutation))?;
            present.commands.push(mutation);
        }
        Ok(())
    }
}
impl Drop for Mounted {
    fn drop(&mut self) {
        self.scope.0.core.dispose();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UiStats {
    pub nodes: usize,
    pub node_slots: usize,
    pub owners: usize,
    pub handlers: usize,
    pub queries: usize,
    pub query_owners: usize,
    pub query_targets: usize,
    pub pending: usize,
    pub disposal_boundaries: usize,
}

pub struct App {
    root: Scope,
    session: Rc<Session>,
}
impl App {
    pub const DEFAULT_SETTLE_PASSES: usize = 8;
    pub const MAX_SETTLE_PASSES: usize = 32;

    /// Observe/flush before drawing, with a strict pass budget. Pending work
    /// yields to the host rather than becoming a runaway error. Adapters opt
    /// into additional local passes; browser RAF and external I/O are not spun.
    pub fn settle(&self, limit: usize) -> Result<Settle> {
        if limit == 0 || limit > Self::MAX_SETTLE_PASSES {
            return Err(Error::SettleLimit {
                requested: limit,
                max: Self::MAX_SETTLE_PASSES,
            });
        }
        let mut turn = Settle::default();
        for _ in 0..limit {
            turn.handled = turn.handled.saturating_add(self.pump()?);
            turn.passes += 1;
            turn.pending = self.session.renderer.borrow().needs_observe()
                || self.session.present.borrow().tree.needs_observe();
            if !turn.pending {
                break;
            }
        }
        Ok(turn)
    }
    /// Pending and idle-spare capacity; excludes batches currently being
    /// processed. Slot/byte counts describe Vec storage, not RSS or live data.
    pub fn buffer_stats(&self) -> crate::BufferStats {
        let p = self.session.present.borrow();
        let commands = p.commands.capacity() + self.session.command_buffer.capacity();
        let actions = p.actions.capacity() + self.session.action_buffer.capacity();
        let listeners = p.listeners.capacity() + self.session.listener_buffer.capacity();
        let inputs = self.session.input_buffer.capacity();
        crate::BufferStats {
            commands,
            actions,
            listeners,
            inputs,
            bytes: commands * size_of::<Mutation>()
                + actions * size_of::<Action>()
                + listeners * size_of::<(NodeId, EventKind, bool)>()
                + inputs * size_of::<Input>(),
        }
    }
    /// Explicit live graph diagnostic; never called by the host frame loop.
    pub fn graph_stats(&self) -> glory_core::GraphStats {
        self.session.runtime.graph_stats()
    }
    pub fn profile(&self) -> Option<Profile> {
        self.session.runtime.profile()
    }
    /// Attach one shared collector to runtime, UI and supported renderer phases.
    /// None stops future spans; snapshots already retained by callers survive.
    pub fn set_profile(&self, profile: Option<Profile>) {
        self.session.runtime.set_profile(profile.clone());
        let previous = self.session.renderer.borrow_mut().set_profile(profile);
        drop(previous);
    }
    /// Construct maps/resources inside the mounted widget's owner. Use this
    /// for repeated root mounting when input signals belong to the app scope.
    pub fn mount_with<W: Widget>(&self, factory: impl FnOnce() -> W) -> Result {
        let built = self.root.mount_with(factory);
        match built {
            Ok(mounted) => {
                self.root.0.children.borrow_mut().push(mounted);
                self.flush()
            }
            Err(error) => {
                self.flush()?;
                Err(error)
            }
        }
    }
    pub fn new<R: Render + 'static>(factory: impl FnOnce(RuntimeId) -> R) -> Self {
        Self::from_runtime(Runtime::new(), factory)
    }
    pub fn with_host<R: Render + 'static>(
        host: Rc<dyn Host>,
        factory: impl FnOnce(RuntimeId) -> R,
    ) -> Self {
        Self::from_runtime(Runtime::with_host(host), factory)
    }
    fn from_runtime<R: Render + 'static>(
        runtime: Runtime,
        factory: impl FnOnce(RuntimeId) -> R,
    ) -> Self {
        let session = Session::new(runtime, factory);
        let root = Scope::new(session.runtime.scope(), &session, None).expect("new UI scope");
        Self { root, session }
    }
    pub fn memory() -> Self {
        Self::new(Memory::new)
    }
    pub fn scope(&self) -> &Scope {
        &self.root
    }
    pub fn runtime(&self) -> &Runtime {
        &self.session.runtime
    }
    pub fn style_profile(&self) -> glory_style::Profile {
        self.session.style_profile
    }
    pub fn runtime_stats(&self) -> RuntimeStats {
        self.session.runtime.stats()
    }
    /// The semantic model may include staged changes. This is not a screen snapshot.
    pub fn tree(&self) -> Ref<'_, Memory> {
        Ref::map(self.session.present.borrow(), |present| &present.tree)
    }
    pub fn stats(&self) -> UiStats {
        let present = self.session.present.borrow();
        UiStats {
            nodes: present.tree.len(),
            node_slots: present.ids.capacity(),
            owners: present.node_owner_count,
            handlers: present.handlers.values().map(BTreeMap::len).sum(),
            queries: present.queries.len(),
            query_owners: present.query_owner_count,
            query_targets: present.node_queries.len(),
            pending: present.commands.len() + present.actions.len() + present.listeners.len(),
            disposal_boundaries: present.disposal_boundaries.len(),
        }
    }
    pub fn mount(&self, widget: impl Widget) -> Result {
        let built = widget.show_in(&self.root);
        self.flush()?;
        built
    }
    pub fn clear(&self) -> Result {
        let children = std::mem::take(&mut *self.root.0.children.borrow_mut());
        drop(children);
        self.flush()
    }
    pub fn flush(&self) -> Result {
        self.session.flush()
    }
    pub fn dispatch(&self, node: NodeId, event: Event) -> Result<bool> {
        if matches!(event, Event::Clipboard { .. } | Event::Files(_)) {
            return Err(RenderError::InvalidEvent(
                "clipboard and file completions require the host input channel",
            )
            .into());
        }
        let handled = self.session.route(node, event)?;
        self.flush()?;
        Ok(handled)
    }
    /// All events delivered in this host tick share one reactive/render flush.
    pub fn pump(&self) -> Result<usize> {
        let mut inputs = self.session.input_buffer.take();
        {
            let _span = self.profile().map(|profile| profile.span(Phase::Observe));
            self.session.renderer.borrow_mut().drain(&mut inputs);
            self.session.present.borrow_mut().tree.drain(&mut inputs);
        }
        let mut handled = 0;
        let mut event_error = None;
        for input in inputs.drain(..) {
            match input {
                Input::Event { node, event } => match self.session.route(node, event) {
                    Ok(delivered) => handled += usize::from(delivered),
                    Err(error) => {
                        event_error.get_or_insert(error);
                    }
                },
                Input::Response {
                    token,
                    applied,
                    result,
                } => {
                    let pending = self.session.present.borrow_mut().remove_query(token);
                    if let Some(pending) = pending {
                        let result = if pending.cancelled {
                            Err(RenderError::NodeGone(pending.node))
                        } else if applied != pending.applied {
                            Err(RenderError::QueryWatermark {
                                expected: pending.applied,
                                applied,
                            })
                        } else {
                            result.and_then(|value| {
                                pending.query.validate_response(&value)?;
                                // An earlier event in this already-drained tick
                                // may have cleared/replaced a selected grant.
                                // File bodies and preview addresses require the
                                // current token even after host completion.
                                if let Query::FileRead(glory_render::FileRead { file, .. })
                                | Query::FilePreview(file) = pending.query
                                {
                                    self.session.present.borrow().tree.selected_file(file)?;
                                }
                                Ok(value)
                            })
                        };
                        self.session.reply(pending, result);
                    }
                }
            }
        }
        self.flush()?;
        event_error.map_or(Ok(handled), Err)
    }
    /// Application-lifetime observation. Use Scope::request for widget/row
    /// waiters that must be released when their requesting Scope is disposed.
    pub fn request(
        &self,
        query: Query,
        reply: impl FnOnce(std::result::Result<QueryValue, RenderError>) + 'static,
    ) -> Result<QueryToken> {
        self.root.request(query, reply)
    }
    pub fn cancel_request(&self, token: QueryToken) -> Result<bool> {
        self.root.cancel_request(token)
    }
}
impl Drop for App {
    fn drop(&mut self) {
        self.root.0.core.dispose();
        let _ = self.session.commit();
        // Dropping outstanding callbacks releases future senders. Do not call
        // application code after the app lifetime has ended.
        self.session.cancelled_queries.borrow_mut().clear();
    }
}

fn file_picker_failure(
    document: glory_render::FileDocumentId,
    revision: u64,
    request: u64,
) -> Event {
    Event::Files(Box::new(glory_render::FileEvent {
        document,
        revision,
        request: Some(request),
        batch: 1,
        origin: glory_render::FileOrigin::Picker,
        outcome: glory_render::FileOutcome::Rejected(glory_render::FileRejection::Failed),
    }))
}

#[cfg(test)]
mod placement_tests {
    use super::*;

    #[test]
    fn retiring_subtree_emits_one_dispose_and_stops_at_a_live_semantic_parent() {
        let app = App::memory();
        let parent = app
            .scope()
            .create(Descriptor::new(glory_render::Props::View))
            .unwrap();
        let content = app.scope().child_in(parent).unwrap();
        let mounted = crate::view()
            .fill((
                crate::text().content("first"),
                crate::group().fill(crate::input().value("draft")),
            ))
            .mount(&content)
            .unwrap();
        let root = mounted.roots()[0];
        let instance = mounted.scope().id();
        app.flush().unwrap();
        assert!(instance.is_alive());
        drop(mounted);
        assert!(!instance.is_alive());
        assert!(content.id().is_alive());
        assert_eq!(app.stats().nodes, 1);
        assert!(app.tree().node(parent).unwrap().children.is_empty());
        assert_eq!(
            app.session.present.borrow().commands,
            [Mutation::Dispose { id: root }]
        );
        app.flush().unwrap();
        // Arena reuse must not make a retired generational instance live again.
        let replacement = crate::view().mount(&content).unwrap();
        assert!(replacement.scope().id().is_alive());
        assert!(!instance.is_alive());
        drop(replacement);
        app.flush().unwrap();
        assert_eq!(app.stats().nodes, 1);
    }

    /// A backend with an extra content restriction, exercising the public hook
    /// without pulling an HTML/platform dependency into the UI layer.
    struct Guard(Memory);
    impl Render for Guard {
        fn preflight_place(
            &self,
            tree: &Memory,
            child: Subtree<'_>,
            parent: Option<NodeId>,
        ) -> std::result::Result<(), RenderError> {
            if let Some(parent) = parent
                && tree.node(parent)?.descriptor.view().name.as_deref() == Some("restricted")
            {
                let descriptor = match child {
                    Subtree::Plan(plan) => plan.nodes()[0].descriptor.view(),
                    Subtree::New(descriptor) => descriptor,
                    Subtree::Existing(id) => tree.node(id)?.descriptor.view(),
                };
                if descriptor.name.as_deref() == Some("blocked") {
                    return Err(RenderError::Unsupported("guarded child"));
                }
            }
            Ok(())
        }
        fn capabilities(&self) -> glory_render::Capabilities {
            self.0.capabilities()
        }
        fn apply(
            &mut self,
            mutations: &[Mutation],
        ) -> std::result::Result<glory_render::Receipt, RenderError> {
            self.0.apply(mutations)
        }
        fn request(
            &mut self,
            query: Query,
            token: QueryToken,
        ) -> std::result::Result<(), RenderError> {
            self.0.request(query, token)
        }
        fn drain(&mut self, inputs: &mut Vec<Input>) {
            self.0.drain(inputs);
        }
    }

    #[test]
    fn refused_place_and_resume_use_staged_ancestors_and_preserve_model_queue_and_future_moves() {
        for resume in [false, true] {
            let app = App::new(|id| Guard(Memory::new(id)));
            let parent = app
                .scope()
                .create(Descriptor::new(glory_render::Props::View))
                .unwrap();
            let child = crate::view().name("blocked").mount(app.scope()).unwrap();
            let id = child.roots()[0];
            app.flush().unwrap();
            if resume {
                child.suspend().unwrap();
            }
            // The committed adapter still sees the original unrestricted parent.
            app.scope()
                .patch(
                    parent,
                    Patch::Name(glory_render::Change::Set(Some("restricted".into()))),
                )
                .unwrap();
            let before = (app.stats(), app.tree().stats());
            assert_eq!(
                app.session.place(id, Some(parent), None),
                Err(Error::Render(RenderError::Unsupported("guarded child")))
            );
            assert_eq!((app.stats(), app.tree().stats()), before);
            assert_eq!(app.tree().node(id).unwrap().parent, None);
            assert_eq!(app.tree().node(id).unwrap().suspended, resume);
            // The staged descriptor changes again, without a host/commit lookup.
            app.scope()
                .patch(parent, Patch::Name(glory_render::Change::Clear))
                .unwrap();
            app.session.place(id, Some(parent), None).unwrap();
            app.flush().unwrap();
            assert_eq!(app.tree().node(parent).unwrap().children, [id]);
            assert!(app.scope().is_active(id).unwrap());
            drop(child);
            app.flush().unwrap();
            assert!(app.tree().node(parent).unwrap().children.is_empty());
        }
    }
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod buffer_tests;
