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
struct Present {
    tree: Memory,
    ids: NodeIds,
    owners: HashMap<InstanceId, BTreeSet<NodeId>>,
    node_owners: HashMap<NodeId, InstanceId>,
    handlers: HashMap<NodeId, BTreeMap<EventKind, Subscription>>,
    subscriptions: HashMap<InstanceId, BTreeSet<(NodeId, EventKind)>>,
    listeners: Vec<(NodeId, EventKind, bool)>,
    commands: Vec<Mutation>,
    actions: Vec<Action>,
    queries: HashMap<QueryToken, PendingQuery>,
    node_queries: HashMap<NodeId, BTreeSet<QueryToken>>,
    owner_queries: HashMap<InstanceId, BTreeSet<QueryToken>>,
    next_query: u64,
}

impl Present {
    fn remove_query(&mut self, token: QueryToken) -> Option<PendingQuery> {
        let pending = self.queries.remove(&token)?;
        if let Some(tokens) = self.node_queries.get_mut(&pending.node) {
            tokens.remove(&token);
            if tokens.is_empty() {
                self.node_queries.remove(&pending.node);
            }
        }
        if let Some(tokens) = self.owner_queries.get_mut(&pending.owner) {
            tokens.remove(&token);
            if tokens.is_empty() {
                self.owner_queries.remove(&pending.owner);
            }
        }
        Some(pending)
    }
}

pub(crate) struct Session {
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
                node_owners: HashMap::new(),
                handlers: HashMap::new(),
                subscriptions: HashMap::new(),
                listeners: Vec::new(),
                commands: Vec::new(),
                actions: Vec::new(),
                queries: HashMap::new(),
                node_queries: HashMap::new(),
                owner_queries: HashMap::new(),
                next_query: 1,
            }),
            renderer: RefCell::new(renderer),
            style_profile,
            applied: Cell::new(0),
            faulted: Cell::new(false),
            error: RefCell::new(None),
            cancelled_queries: RefCell::new(Vec::new()),
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
            .preflight(&present.tree.node(id)?.descriptor, Some(&patch))?;
        let mutation = Mutation::Patch { id, patch };
        present.tree.apply(std::slice::from_ref(&mutation))?;
        present.commands.push(mutation);
        Ok(())
    }

    pub(crate) fn input_value(&self, id: NodeId, value: String) -> Result {
        let revision = {
            let present = self.present.borrow();
            let glory_render::Props::Input(input) = &present.tree.node(id)?.descriptor.props else {
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

    fn place(&self, id: NodeId, parent: Option<NodeId>, before: Option<NodeId>) -> Result {
        let mut present = self.present.borrow_mut();
        let node = present.tree.node(id)?;
        if node.placed && !node.suspended && node.parent == parent {
            let siblings = if let Some(parent) = parent {
                &present.tree.node(parent)?.children
            } else {
                present.tree.roots()
            };
            if let Some(index) = siblings.iter().position(|child| *child == id)
                && siblings.get(index + 1).copied() == before
            {
                return Ok(());
            }
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
            if let Some(tokens) = present.owner_queries.remove(&owner) {
                for token in tokens {
                    if let Some(query) = present.remove_query(token) {
                        discarded_queries.push(query);
                    }
                }
            }
            if let Some(subscriptions) = present.subscriptions.remove(&owner) {
                for (node, kind) in subscriptions {
                    if let Some(subscription) = present
                        .handlers
                        .get_mut(&node)
                        .and_then(|handlers| handlers.remove(&kind))
                    {
                        discarded.push(subscription.callback);
                        present.listeners.push((node, kind, false));
                    }
                    if present.handlers.get(&node).is_some_and(BTreeMap::is_empty) {
                        present.handlers.remove(&node);
                    }
                }
            }
            let roots = present.owners.remove(&owner).unwrap_or_default();
            for id in roots {
                if !present.ids.contains(id) {
                    continue;
                }
                let mut all = Vec::new();
                let mut pending = vec![id];
                while let Some(id) = pending.pop() {
                    if let Ok(node) = present.tree.node(id) {
                        pending.extend(node.children.iter().copied());
                        all.push(id);
                    }
                }
                for child in all {
                    present.ids.release(child);
                    if let Some(owner) = present.node_owners.remove(&child)
                        && let Some(nodes) = present.owners.get_mut(&owner)
                    {
                        nodes.remove(&child);
                        if nodes.is_empty() {
                            present.owners.remove(&owner);
                        }
                    }
                    if let Some(handlers) = present.handlers.remove(&child) {
                        for (kind, subscription) in handlers {
                            if let Some(subscriptions) =
                                present.subscriptions.get_mut(&subscription.owner)
                            {
                                subscriptions.remove(&(child, kind));
                                if subscriptions.is_empty() {
                                    present.subscriptions.remove(&subscription.owner);
                                }
                            }
                            discarded.push(subscription.callback);
                        }
                    }
                    if let Some(tokens) = present.node_queries.remove(&child) {
                        for token in tokens {
                            if let Some(query) = present.queries.get_mut(&token) {
                                query.cancelled = true;
                                cancelled.push(token);
                            }
                        }
                    }
                }
                let mutation = Mutation::Dispose { id };
                present
                    .tree
                    .apply(std::slice::from_ref(&mutation))
                    .expect("owned semantic subtree");
                present.commands.push(mutation);
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
        let commands = std::mem::take(&mut self.present.borrow_mut().commands);
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
        let actions = std::mem::take(&mut self.present.borrow_mut().actions);
        let listeners = std::mem::take(&mut self.present.borrow_mut().listeners);
        for (node, kind, enabled) in listeners {
            if !self.present.borrow().ids.contains(node) {
                continue;
            }
            if let Err(error) = self.renderer.borrow_mut().listen(node, kind, enabled) {
                self.report(error.into());
            }
        }
        for action in actions {
            if !self.present.borrow().tree.active(action.node()) {
                continue;
            }
            let _span = self
                .runtime
                .profile()
                .map(|profile| profile.span(Phase::Action));
            if let Err(error) = self.renderer.borrow_mut().action(action) {
                // Focus can become unavailable after queuing (CSS/layout,
                // disabling, host policy). That ordinary refusal does not
                // desynchronize the semantic tree. Focus is a best-effort
                // intent; Query::Focus observes the actual result. Retain later
                // intents; actual backend failures remain fatal.
                if matches!(action, Action::Focus(_))
                    && matches!(error, RenderError::Unsupported(_))
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
        // Suspension preserves the scope/observers. Terminal focus/composition
        // loss still releases pins/editing after its physical range detaches.
        // Disposal remains guarded by generational identity and live handlers.
        let terminal_loss = event.is_terminal_loss();
        let (handlers, observed_tree) = {
            let present = self.present.borrow();
            if !present.ids.contains(node) || !present.tree.active(node) && !terminal_loss {
                return Ok(false);
            }
            let props = &present.tree.node(node)?.descriptor.props;
            if let Event::ValueChanged { revision, .. } | Event::Commit { revision, .. } = &event
                && let glory_render::Props::Input(input) = props
                && *revision < input.revision
            {
                return Ok(false);
            }
            if !present.tree.validate_event(node, &event)? {
                return Ok(false);
            }
            if !props.enabled()
                && matches!(
                    event,
                    Event::Activate
                        | Event::Keyboard { .. }
                        | Event::Drag { .. }
                        | Event::ValueChanged { .. }
                        | Event::CheckedChanged(_)
                        | Event::Commit { .. }
                )
            {
                return Ok(false);
            }
            let mut cursor = Some(node);
            let mut handler = None;
            while let Some(id) = cursor {
                let current = present.tree.node(id)?;
                if matches!(event, Event::Activate | Event::Keyboard { .. })
                    && !current.descriptor.props.enabled()
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
                    Event::Activate | Event::Navigate { .. } | Event::Keyboard { .. }
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
            (handlers, present.tree.stats())
        };
        let handled = !handlers.is_empty();
        for (id, kind, handler, event) in handlers {
            let owner = {
                let present = self.present.borrow();
                if present.tree.active(id) || terminal_loss {
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
                if matches!(event, Event::FocusWithin { .. }) {
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
            present
                .node_queries
                .entry(query.node())
                .or_default()
                .insert(token);
            present
                .owner_queries
                .entry(owner)
                .or_default()
                .insert(token);
            present.queries.insert(
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
    pub fn mount_with<W: Widget>(&self, factory: impl FnOnce() -> W) -> Result<Mounted> {
        let _span = self
            .session()?
            .runtime
            .profile()
            .map(|profile| profile.span(Phase::Build));
        let scope = self.child()?;
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
    pub fn child(&self) -> Result<Self> {
        let session = self.session()?;
        Self::new(self.0.core.child(), &session, self.0.parent)
    }
    pub fn child_in(&self, parent: NodeId) -> Result<Self> {
        self.session()?.present.borrow().tree.node(parent)?;
        Self::new(self.0.core.child(), &self.session()?, Some(parent))
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
        session.renderer.borrow().preflight(&descriptor, None)?;
        let id = {
            let mut present = session.present.borrow_mut();
            present
                .tree
                .validate_child(Subtree::New(&descriptor), self.0.parent)?;
            session.renderer.borrow().preflight_place(
                &present.tree,
                Subtree::New(&descriptor),
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
            present.node_owners.insert(id, self.id());
            present.owners.entry(self.id()).or_default().insert(id);
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
        let capabilities = session.renderer.borrow().capabilities();
        match action {
            Action::Focus(_) if !capabilities.focus => {
                return Err(RenderError::Unsupported("focus").into());
            }
            Action::ScrollTo { .. } if !capabilities.scroll => {
                return Err(RenderError::Unsupported("scroll").into());
            }
            _ => {}
        }
        let mut present = session.present.borrow_mut();
        present.tree.node(action.node())?;
        present.actions.push(action);
        Ok(())
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
            let previous = present.handlers.entry(node).or_default().insert(
                kind,
                Subscription {
                    owner: self.id(),
                    callback: Rc::new(RefCell::new(Box::new(handler))),
                },
            );
            if let Some(previous) = &previous
                && let Some(subscriptions) = present.subscriptions.get_mut(&previous.owner)
            {
                subscriptions.remove(&(node, kind));
                if subscriptions.is_empty() {
                    present.subscriptions.remove(&previous.owner);
                }
            }
            present
                .subscriptions
                .entry(self.id())
                .or_default()
                .insert((node, kind));
            if previous.is_none() {
                present.listeners.push((node, kind, true));
            }
            previous
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

/// A component is built once; dynamic props/control flow install owned bindings.
pub trait Widget: 'static {
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
            turn.pending = self.session.renderer.borrow().needs_observe();
            if !turn.pending {
                break;
            }
        }
        Ok(turn)
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
    /// Construct maps/resources inside the mounted component's owner. Use this
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
            owners: present.owners.len(),
            handlers: present.handlers.values().map(BTreeMap::len).sum(),
            queries: present.queries.len(),
            query_owners: present.owner_queries.len(),
            query_targets: present.node_queries.len(),
            pending: present.commands.len() + present.actions.len() + present.listeners.len(),
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
        let handled = self.session.route(node, event)?;
        self.flush()?;
        Ok(handled)
    }
    /// All events delivered in this host tick share one reactive/render flush.
    pub fn pump(&self) -> Result<usize> {
        let mut inputs = Vec::new();
        {
            let _span = self.profile().map(|profile| profile.span(Phase::Observe));
            self.session.renderer.borrow_mut().drain(&mut inputs);
        }
        let mut handled = 0;
        let mut event_error = None;
        for input in inputs {
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
    /// Application-lifetime observation. Use Scope::request for component/row
    /// waiters that must be released when their requesting Scope is disposed.
    pub fn request(
        &self,
        query: Query,
        reply: impl FnOnce(std::result::Result<QueryValue, RenderError>) + 'static,
    ) -> Result<QueryToken> {
        self.root.request(query, reply)
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

#[cfg(test)]
mod placement_tests {
    use super::*;

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
                && tree.node(parent)?.descriptor.name.as_deref() == Some("restricted")
            {
                let descriptor = match child {
                    Subtree::New(descriptor) => descriptor,
                    Subtree::Existing(id) => &tree.node(id)?.descriptor,
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
