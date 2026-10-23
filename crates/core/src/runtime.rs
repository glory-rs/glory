use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::time::Duration;

use crate::RuntimeId;
use crate::arena::{Arena, Key};
use glory_render::profile::{Phase, Profile};
#[path = "fields.rs"]
mod fields;

pub type LocalFuture = Pin<Box<dyn Future<Output = ()>>>;
type Cancel = Box<dyn FnOnce()>;
type Value = Rc<RefCell<Box<dyn Any>>>;
type Mapper = Rc<dyn Fn() -> Box<dyn Any>>;
type Equal = Rc<dyn Fn(&dyn Any, &dyn Any) -> bool>;
type Callback = Rc<RefCell<Box<dyn FnMut()>>>;
type Shared = Rc<RefCell<State>>;
const QUEUE_SLACK: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostError(pub String);
impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for HostError {}

/// One host owns the event loop. Neither method may synchronously reenter a
/// runtime; spawn queues a future without polling it before returning. Returned
/// cancellation must stop/drop that future. No built-in block_on fallback.
pub trait Host {
    fn spawn(&self, future: LocalFuture) -> Result<Cancel, HostError>;
    fn wake(&self);
    fn now(&self) -> Duration;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccessError {
    RuntimeGone,
    Disposed,
    WrongRuntime,
    Borrowed,
    TypeMismatch,
    Cycle,
    WriteDuringCompute,
    ReentrantFlush,
    RunawayEffect,
    NoHost,
    Host(HostError),
    InvalidField(&'static str),
    MissingField,
    OutOfBounds,
    Capacity,
}
impl std::fmt::Display for AccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for AccessError {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeStats {
    pub source_writes: u64,
    pub marked: u64,
    pub validations: u64,
    pub derived_runs: u64,
    pub binding_runs: u64,
    pub structural_runs: u64,
    pub effect_runs: u64,
    pub nodes: usize,
    pub node_slots: usize,
    pub owners: usize,
    pub owner_slots: usize,
    pub instances: usize,
    pub instance_slots: usize,
    pub tasks: usize,
    pub task_slots: usize,
    pub edges: usize,
    pub queued: usize,
    /// Physical FIFO entries, including bounded retired generational identities.
    pub queue_entries: usize,
    /// Allocated FIFO capacity; like arena slots it follows peak concurrent work.
    pub queue_capacity: usize,
    pub queue_compactions: u64,
    /// Identities inspected by nonempty compaction, excluding constant-time clear.
    pub queue_compaction_visits: u64,
    pub fields: usize,
    pub field_slots: usize,
    pub entry_keys: usize,
    pub entry_buckets: usize,
    pub field_target_builds: u64,
    pub field_target_hits: u64,
    pub field_target_visits: u64,
    pub field_target_caches: usize,
    pub field_target_keys: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Pure,
    Effect,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Clean,
    Check,
    Dirty,
    Computing,
}

#[derive(Clone)]
enum Compute {
    Source,
    Derived {
        mapper: Mapper,
        equal: Option<Equal>,
    },
    Binding(Callback),
    Structure(Callback),
    Effect(Callback),
}

struct Node {
    owner: Key,
    compute: Compute,
    value: Option<Value>,
    version: u64,
    status: Status,
    rerun: bool,
    // This node's current invalidation has reached its subscriber frontier.
    // A validation/new subscription rearms it before any refreshed consumer
    // could otherwise be hidden behind an already-pending ancestor.
    propagated: bool,
    last_flush: u64,
    flush_runs: u32,
    dependencies: BTreeMap<Key, u64>,
    subscribers: BTreeSet<Key>,
}

#[derive(Default)]
struct OwnerData {
    parent: Option<Key>,
    children: BTreeSet<Key>,
    nodes: BTreeSet<Key>,
    tasks: BTreeSet<Key>,
    cleanup: Vec<Cancel>,
    instance: Option<Key>,
}

struct Instance {
    owner: Key,
    parent: Option<Key>,
}
struct TaskState {
    owner: Key,
    cancel: Option<Cancel>,
}

struct State {
    root: Key,
    nodes: Arena<Node>,
    owners: Arena<OwnerData>,
    instances: Arena<Instance>,
    tasks: Arena<TaskState>,
    bindings: VecDeque<Key>,
    structures: VecDeque<Key>,
    effects: VecDeque<Key>,
    queued: BTreeSet<Key>,
    queue_retired: usize,
    flushing: bool,
    batch_depth: usize,
    flush_epoch: u64,
    host: Option<Rc<dyn Host>>,
    stats: RuntimeStats,
    profile: Option<Profile>,
    fields: fields::Fields,
}

struct Frame {
    runtime: RuntimeId,
    key: Key,
    dependencies: BTreeMap<Key, u64>,
}
thread_local! {
    // This is only a handle lookup and stack of active evaluations. Every graph,
    // queue, value, owner and scheduler flag lives in its own Runtime, not here.
    static RUNTIMES: RefCell<HashMap<RuntimeId, Weak<RefCell<State>>>> = RefCell::new(HashMap::new());
    static TRACKING: RefCell<Vec<Option<Frame>>> = const { RefCell::new(Vec::new()) };
    static EVALUATING: RefCell<Vec<Mode>> = const { RefCell::new(Vec::new()) };
    static CONSTRUCTING: RefCell<Vec<(RuntimeId, Key)>> = const { RefCell::new(Vec::new()) };
}

fn allow_creation() -> Result<(), AccessError> {
    if EVALUATING.with_borrow(|stack| stack.last() == Some(&Mode::Pure)) {
        Err(AccessError::WriteDuringCompute)
    } else {
        Ok(())
    }
}

fn enter_owner<R>(runtime: RuntimeId, owner: Key, f: impl FnOnce() -> R) -> R {
    struct Enter(usize);
    impl Drop for Enter {
        fn drop(&mut self) {
            CONSTRUCTING.with_borrow_mut(|stack| stack.truncate(self.0));
        }
    }
    let previous = CONSTRUCTING.with_borrow_mut(|stack| {
        let len = stack.len();
        stack.push((runtime, owner));
        len
    });
    let _guard = Enter(previous);
    f()
}

fn lookup(runtime: RuntimeId) -> Result<Shared, AccessError> {
    RUNTIMES
        .with_borrow(|runtimes| runtimes.get(&runtime).and_then(Weak::upgrade))
        .ok_or(AccessError::RuntimeGone)
}

fn check_reader(runtime: RuntimeId) -> Result<(), AccessError> {
    TRACKING.with_borrow(|stack| {
        if stack
            .last()
            .and_then(Option::as_ref)
            .is_some_and(|frame| frame.runtime != runtime)
        {
            Err(AccessError::WrongRuntime)
        } else {
            Ok(())
        }
    })
}

fn track(runtime: RuntimeId, key: Key, version: u64) -> Result<(), AccessError> {
    TRACKING.with_borrow_mut(|stack| {
        if let Some(Some(frame)) = stack.last_mut() {
            if frame.runtime != runtime {
                return Err(AccessError::WrongRuntime);
            }
            frame.dependencies.insert(key, version);
        }
        Ok(())
    })
}

struct TrackingPause(usize);
impl Drop for TrackingPause {
    fn drop(&mut self) {
        TRACKING.with_borrow_mut(|stack| stack.truncate(self.0));
    }
}

/// Suppress only reads in this closure. Derived computations still track their
/// own dependencies, and purity checks still apply inside untracked code.
pub fn untracked<R>(f: impl FnOnce() -> R) -> R {
    let previous = TRACKING.with_borrow_mut(|stack| {
        let len = stack.len();
        stack.push(None);
        len
    });
    let _guard = TrackingPause(previous);
    f()
}

impl State {
    fn new() -> Self {
        let mut owners = Arena::default();
        let root = owners.insert(OwnerData::default());
        Self {
            root,
            owners,
            nodes: Arena::default(),
            instances: Arena::default(),
            tasks: Arena::default(),
            bindings: VecDeque::new(),
            structures: VecDeque::new(),
            effects: VecDeque::new(),
            queued: BTreeSet::new(),
            queue_retired: 0,
            flushing: false,
            batch_depth: 0,
            flush_epoch: 0,
            host: None,
            stats: RuntimeStats::default(),
            profile: None,
            fields: fields::Fields::default(),
        }
    }

    fn enqueue(&mut self, key: Key) {
        let Some(node) = self.nodes.get(key) else {
            return;
        };
        let phase = match node.compute {
            Compute::Structure(_) => 0,
            Compute::Binding(_) => 1,
            Compute::Effect(_) => 2,
            _ => return,
        };
        if self.queued.insert(key) {
            match phase {
                0 => self.structures.push_back(key),
                1 => self.bindings.push_back(key),
                _ => self.effects.push_back(key),
            }
        }
    }

    fn queue_entries(&self) -> usize {
        self.structures.len() + self.bindings.len() + self.effects.len()
    }

    fn compact_queues(&mut self) {
        debug_assert_eq!(self.queue_entries(), self.queued.len() + self.queue_retired);
        if self.queue_retired == 0 {
            return;
        }
        let live = self.queued.len();
        if live == 0 {
            // Keys own no data. Clearing lengths needs no per-key destruction.
            self.structures.clear();
            self.bindings.clear();
            self.effects.clear();
        } else if self.queue_retired > live.saturating_add(QUEUE_SLACK) {
            // Only node retirement leaves tombstones, so generation-checked
            // arena lookup is sufficient. No callback, value or graph traversal.
            self.stats.queue_compaction_visits += self.queue_entries() as u64;
            self.structures.retain(|key| self.nodes.get(*key).is_some());
            self.bindings.retain(|key| self.nodes.get(*key).is_some());
            self.effects.retain(|key| self.nodes.get(*key).is_some());
        } else {
            return;
        }
        self.queue_retired = 0;
        self.stats.queue_compactions += 1;
    }

    fn next_observer(&mut self) -> Option<Key> {
        loop {
            let key = self
                .structures
                .pop_front()
                .or_else(|| self.bindings.pop_front())
                .or_else(|| self.effects.pop_front())?;
            if self.queued.remove(&key) {
                // Draining live work can also change the live/tombstone ratio.
                // Compact before callbacks so the bound holds during reentry.
                self.compact_queues();
                return Some(key);
            }
            self.queue_retired -= 1;
        }
    }

    fn invalidate(&mut self, first: impl IntoIterator<Item = Key>) {
        let mut pending: Vec<_> = first.into_iter().collect();
        while let Some(key) = pending.pop() {
            let Some(node) = self.nodes.get_mut(key) else {
                continue;
            };
            if node.propagated {
                continue;
            }
            node.propagated = true;
            self.stats.marked += 1;
            match node.status {
                Status::Clean => node.status = Status::Check,
                Status::Computing => node.rerun = true,
                _ => {}
            }
            pending.extend(node.subscribers.iter().copied());
            self.enqueue(key);
        }
    }

    fn publish(&mut self, key: Key) {
        let Some(node) = self.nodes.get_mut(key) else {
            return;
        };
        node.version = node
            .version
            .checked_add(1)
            .expect("source version exhausted");
        self.stats.source_writes += 1;
        if node.propagated {
            return;
        }
        node.propagated = true;
        let subscribers: Vec<_> = node.subscribers.iter().copied().collect();
        self.invalidate(subscribers);
    }

    fn insert_node(&mut self, owner: Key, node: Node) -> Result<Key, AccessError> {
        let owner = self.owners.get_mut(owner).ok_or(AccessError::Disposed)?;
        let key = self.nodes.insert(node);
        owner.nodes.insert(key);
        self.enqueue(key);
        Ok(key)
    }

    fn remove_node(&mut self, key: Key, cleanup: &mut Cleanup) {
        let Some(node) = self.nodes.remove(key) else {
            return;
        };
        if let Some(owner) = self.owners.get_mut(node.owner) {
            owner.nodes.remove(&key);
        }
        for source in node.dependencies.keys() {
            if let Some(source) = self.nodes.get_mut(*source) {
                source.subscribers.remove(&key);
            }
        }
        if matches!(
            &node.compute,
            Compute::Structure(_) | Compute::Binding(_) | Compute::Effect(_)
        ) && self.queued.remove(&key)
        {
            // Cancel membership immediately. A small retirement must not scan
            // unrelated ready work. Only numeric stale keys wait for compaction.
            self.queue_retired += 1;
            self.compact_queues();
        }
        self.invalidate(node.subscribers.iter().copied());
        let removal = self.fields.detach(key, node.owner);
        cleanup.fields.extend(removal.fields);
        cleanup.nodes.push(node);
        for alias in removal.aliases {
            self.remove_node(alias, cleanup);
        }
    }

    fn remove_task(&mut self, key: Key) -> Option<TaskState> {
        let task = self.tasks.remove(key)?;
        if let Some(owner) = self.owners.get_mut(task.owner) {
            owner.tasks.remove(&key);
        }
        Some(task)
    }

    fn dispose_owner(&mut self, key: Key) -> Cleanup {
        let mut cleanup = Cleanup::default();
        let Some(mut owner) = self.owners.remove(key) else {
            return cleanup;
        };
        // Most repeated disposals come from handles already retired by an
        // ancestor. Do not allocate a traversal stack for them or for leaves.
        let mut pending = Vec::new();
        let mut key = key;
        loop {
            if let Some(parent) = owner.parent.and_then(|parent| self.owners.get_mut(parent)) {
                parent.children.remove(&key);
            }
            pending.extend(owner.children);
            if let Some(instance) = owner.instance {
                self.instances.remove(instance);
            }
            if !owner.cleanup.is_empty() {
                cleanup.callbacks.push((key, owner.cleanup));
            }
            for task in owner.tasks {
                if let Some(task) = self.remove_task(task) {
                    cleanup.tasks.push(task);
                }
            }
            for node in owner.nodes {
                self.remove_node(node, &mut cleanup);
            }
            loop {
                let Some(next) = pending.pop() else {
                    return cleanup;
                };
                if let Some(next_owner) = self.owners.remove(next) {
                    key = next;
                    owner = next_owner;
                    break;
                }
            }
        }
    }
}

#[derive(Default)]
struct Cleanup {
    nodes: Vec<Node>,
    tasks: Vec<TaskState>,
    callbacks: Vec<(Key, Vec<Cancel>)>,
    fields: Vec<fields::FieldData>,
}
impl Cleanup {
    fn run(self, runtime: RuntimeId) {
        // All identities and subscriptions have already been invalidated.
        // Destructors and callbacks execute with no graph borrow held.
        for task in self.tasks {
            if let Some(cancel) = task.cancel {
                untracked(|| enter_owner(runtime, task.owner, cancel));
            }
        }
        for (owner, callbacks) in self.callbacks.into_iter().rev() {
            untracked(|| {
                enter_owner(runtime, owner, || {
                    for callback in callbacks.into_iter().rev() {
                        callback();
                    }
                });
            });
        }
        drop(self.nodes);
        drop(self.fields);
    }
}

struct Evaluation {
    state: Shared,
    key: Key,
    frame: Option<usize>,
    complete: bool,
}
impl Evaluation {
    fn new(state: &Shared, key: Key, mode: Mode) -> Self {
        EVALUATING.with_borrow_mut(|stack| stack.push(mode));
        Self {
            state: state.clone(),
            key,
            frame: None,
            complete: false,
        }
    }

    fn start_tracking(&mut self, runtime: RuntimeId) {
        self.frame = Some(TRACKING.with_borrow_mut(|stack| {
            let previous = stack.len();
            stack.push(Some(Frame {
                runtime,
                key: self.key,
                dependencies: BTreeMap::new(),
            }));
            previous
        }));
    }

    fn dependencies(&mut self) -> BTreeMap<Key, u64> {
        self.frame.take();
        TRACKING.with_borrow_mut(|stack| {
            let frame = stack.pop().flatten().expect("evaluation tracking frame");
            debug_assert_eq!(frame.key, self.key);
            frame.dependencies
        })
    }
}

impl Drop for Evaluation {
    fn drop(&mut self) {
        if let Some(previous) = self.frame {
            TRACKING.with_borrow_mut(|stack| stack.truncate(previous));
        }
        if !self.complete {
            let mut state = self.state.borrow_mut();
            if let Some(node) = state.nodes.get_mut(self.key) {
                node.status = Status::Dirty;
                node.propagated = false;
                state.enqueue(self.key);
            }
        }
        EVALUATING.with_borrow_mut(|stack| {
            stack.pop();
        });
    }
}

fn ensure(runtime: RuntimeId, state: &Shared, key: Key) -> Result<(), AccessError> {
    let (compute, old_status, old_dependencies, old_value, profile) = {
        let mut state = state.try_borrow_mut().map_err(|_| AccessError::Borrowed)?;
        let node = state.nodes.get_mut(key).ok_or(AccessError::Disposed)?;
        if node.status == Status::Computing {
            return Err(AccessError::Cycle);
        }
        if node.status == Status::Clean {
            return Ok(());
        }
        let job = (
            node.compute.clone(),
            node.status,
            node.dependencies.clone(),
            node.value.clone(),
        );
        node.status = Status::Computing;
        node.propagated = false;
        // Reads/validation can refresh only part of a pending graph. Rearm
        // upstream frontiers before executing callbacks, including effects
        // that write an existing dependency while this node is computing.
        for dependency in job.2.keys() {
            if let Some(source) = state.nodes.get_mut(*dependency) {
                source.propagated = false;
            }
        }
        state.stats.validations += 1;
        (job.0, job.1, job.2, job.3, state.profile.clone())
    };
    let _validation = profile.as_ref().map(|profile| {
        profile
            .span(Phase::Validate)
            .work(old_dependencies.len() as u64)
    });
    let mode = if matches!(compute, Compute::Effect(_) | Compute::Structure(_)) {
        Mode::Effect
    } else {
        Mode::Pure
    };
    let mut evaluation = Evaluation::new(state, key, mode);
    if old_status == Status::Check {
        let mut changed = false;
        for (dependency, version) in &old_dependencies {
            if state.borrow().nodes.get(*dependency).is_none() {
                changed = true;
                continue;
            }
            ensure(runtime, state, *dependency)?;
            changed |= state
                .borrow()
                .nodes
                .get(*dependency)
                .is_none_or(|source| source.version != *version);
        }
        if !changed {
            if let Some(node) = state.borrow_mut().nodes.get_mut(key) {
                node.status = Status::Clean;
            }
            evaluation.complete = true;
            return Ok(());
        }
    }

    evaluation.start_tracking(runtime);
    let output = {
        let phase = match &compute {
            Compute::Derived { .. } => Some(Phase::Derive),
            Compute::Binding(_) => Some(Phase::Bind),
            Compute::Structure(_) => Some(Phase::Structure),
            Compute::Effect(_) => Some(Phase::Effect),
            Compute::Source => None,
        };
        let _compute = profile
            .as_ref()
            .zip(phase)
            .map(|(profile, phase)| profile.span(phase));
        // Dependency validation (or a profile clock) can retire this observer.
        // Never execute a captured callback after its owning node disappeared.
        let owner = state.borrow().nodes.get(key).map(|node| node.owner);
        if let Some(owner) = owner {
            enter_owner(runtime, owner, || {
                Ok::<_, AccessError>(match &compute {
                    Compute::Derived { mapper, .. } => Some(mapper()),
                    Compute::Binding(callback)
                    | Compute::Structure(callback)
                    | Compute::Effect(callback) => {
                        callback
                            .try_borrow_mut()
                            .map_err(|_| AccessError::Borrowed)?();
                        None
                    }
                    Compute::Source => None,
                })
            })?
        } else {
            None
        }
    };
    let dependencies = evaluation.dependencies();
    let changed = if let (
        Some(output),
        Some(old),
        Compute::Derived {
            equal: Some(equal), ..
        },
    ) = (&output, &old_value, &compute)
    {
        untracked(|| !equal(old.borrow().as_ref(), output.as_ref()))
    } else {
        output.is_some()
    };

    let mut state = state.borrow_mut();
    if state.nodes.get(key).is_none() {
        evaluation.complete = true;
        return Ok(());
    }
    for dependency in old_dependencies.keys() {
        if !dependencies.contains_key(dependency)
            && let Some(source) = state.nodes.get_mut(*dependency)
        {
            source.subscribers.remove(&key);
        }
    }
    let mut needs_rerun = false;
    for (dependency, version) in &dependencies {
        if let Some(source) = state.nodes.get_mut(*dependency) {
            source.subscribers.insert(key);
            // A new or recomputed subscriber has now observed this version.
            // The next publication must reach it, even after unobserved writes.
            source.propagated = false;
            needs_rerun |= source.version != *version;
        } else {
            needs_rerun = true;
        }
    }
    let node = state.nodes.get_mut(key).expect("live evaluation");
    node.dependencies = dependencies;
    let discarded = if changed {
        node.version = node
            .version
            .checked_add(1)
            .expect("derived version exhausted");
        output.map(|output| node.value.replace(Rc::new(RefCell::new(output))))
    } else {
        None
    };
    needs_rerun |= node.rerun;
    node.rerun = false;
    node.status = if needs_rerun {
        Status::Check
    } else {
        Status::Clean
    };
    node.propagated = false;
    match compute {
        Compute::Derived { .. } => state.stats.derived_runs += 1,
        Compute::Binding(_) => state.stats.binding_runs += 1,
        Compute::Structure(_) => state.stats.structural_runs += 1,
        Compute::Effect(_) => state.stats.effect_runs += 1,
        Compute::Source => {}
    }
    if needs_rerun {
        state.enqueue(key);
    }
    evaluation.complete = true;
    drop(state);
    drop(discarded);
    Ok(())
}

/// Owns one graph and its scheduler. Runtime is intentionally not Clone.
pub struct Runtime {
    id: RuntimeId,
    state: Shared,
}
impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
impl Runtime {
    pub fn new() -> Self {
        let id = RuntimeId::new();
        let state = Rc::new(RefCell::new(State::new()));
        RUNTIMES.with_borrow_mut(|runtimes| runtimes.insert(id, Rc::downgrade(&state)));
        Self { id, state }
    }
    pub fn with_host(host: Rc<dyn Host>) -> Self {
        let runtime = Self::new();
        runtime.state.borrow_mut().host = Some(host);
        runtime
    }
    pub fn id(&self) -> RuntimeId {
        self.id
    }
    pub fn profile(&self) -> Option<Profile> {
        self.state.borrow().profile.clone()
    }
    pub fn set_profile(&self, profile: Option<Profile>) {
        let previous = std::mem::replace(&mut self.state.borrow_mut().profile, profile);
        drop(previous);
    }
    pub fn now(&self) -> Option<Duration> {
        self.state.borrow().host.as_ref().map(|host| host.now())
    }
    pub fn owner(&self) -> Owner {
        let root = self.state.borrow().root;
        make_owner(self.id, &self.state, root).expect("runtime root is live")
    }
    pub fn scope(&self) -> Scope {
        make_scope(self.owner(), None).expect("runtime is live")
    }

    /// Run a synchronous callback under a live instance's Owner. This does not
    /// retain the instance, borrow the graph during the callback, or flush work.
    /// Foreign/stale identities are rejected before calling user code; nested
    /// context is restored on return or panic. Future polls need their own entry.
    #[inline]
    pub fn enter<R>(&self, instance: InstanceId, f: impl FnOnce() -> R) -> Result<R, AccessError> {
        if instance.runtime != self.id {
            return Err(AccessError::WrongRuntime);
        }
        let owner = {
            let state = self.state.borrow();
            let owner = state
                .instances
                .get(instance.key)
                .ok_or(AccessError::Disposed)?
                .owner;
            state.owners.get(owner).ok_or(AccessError::Disposed)?;
            owner
        };
        Ok(enter_owner(self.id, owner, f))
    }

    /// Values enqueue work. Explicit flush/batch at an event/frame boundary
    /// prevents a series of writes from rendering intermediate states.
    pub fn batch<R>(&self, f: impl FnOnce() -> R) -> Result<R, AccessError> {
        self.state.borrow_mut().batch_depth += 1;
        struct Batch(Shared);
        impl Drop for Batch {
            fn drop(&mut self) {
                self.0.borrow_mut().batch_depth -= 1;
            }
        }
        let guard = Batch(self.state.clone());
        let value = f();
        drop(guard);
        let flush = {
            let state = self.state.borrow();
            state.batch_depth == 0 && !state.flushing
        };
        if flush {
            self.flush()?;
        }
        Ok(value)
    }

    pub fn flush(&self) -> Result<(), AccessError> {
        if self.state.borrow().batch_depth != 0 {
            return Ok(());
        }
        if self.state.borrow().flushing {
            return Err(AccessError::ReentrantFlush);
        }
        let _span = self.profile().map(|profile| profile.span(Phase::Flush));
        self.state.borrow_mut().flushing = true;
        let epoch = {
            let mut state = self.state.borrow_mut();
            state.flush_epoch = state
                .flush_epoch
                .checked_add(1)
                .expect("flush epoch exhausted");
            state.flush_epoch
        };
        struct Flush(Shared);
        impl Drop for Flush {
            fn drop(&mut self) {
                self.0.borrow_mut().flushing = false;
            }
        }
        let _guard = Flush(self.state.clone());
        loop {
            let key = self.state.borrow_mut().next_observer();
            let Some(key) = key else { break };
            {
                let mut state = self.state.borrow_mut();
                let Some(node) = state.nodes.get_mut(key) else {
                    continue;
                };
                if node.last_flush != epoch {
                    node.last_flush = epoch;
                    node.flush_runs = 0;
                }
                node.flush_runs += 1;
                // Bound repeated feedback of one observer, not the total number
                // of independent bindings in a large application/SSR document.
                if node.flush_runs > 1024 {
                    state.enqueue(key);
                    return Err(AccessError::RunawayEffect);
                }
            }
            ensure(self.id, &self.state, key)?;
        }
        Ok(())
    }

    pub fn stats(&self) -> RuntimeStats {
        let state = self.state.borrow();
        let (entry_keys, entry_buckets) = state.fields.entry_stats();
        let (field_target_caches, field_target_keys) = state.fields.target_stats();
        RuntimeStats {
            nodes: state.nodes.len(),
            node_slots: state.nodes.capacity(),
            owners: state.owners.len(),
            owner_slots: state.owners.capacity(),
            instances: state.instances.len(),
            instance_slots: state.instances.capacity(),
            tasks: state.tasks.len(),
            task_slots: state.tasks.capacity(),
            edges: state
                .nodes
                .values()
                .map(|node| node.dependencies.len())
                .sum(),
            queued: state.queued.len(),
            queue_entries: state.queue_entries(),
            queue_capacity: state.structures.capacity()
                + state.bindings.capacity()
                + state.effects.capacity(),
            fields: state.fields.len(),
            field_slots: state.fields.capacity(),
            entry_keys,
            entry_buckets,
            field_target_caches,
            field_target_keys,
            ..state.stats
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        RUNTIMES.with_borrow_mut(|runtimes| runtimes.remove(&self.id));
        let root = self.state.borrow().root;
        let cleanup = self.state.borrow_mut().dispose_owner(root);
        cleanup.run(self.id);
    }
}

pub struct Owner {
    runtime: RuntimeId,
    key: Key,
    local: PhantomData<Rc<()>>,
}
/// Non-owning identity for internal controllers. It cannot prolong an Owner or
/// its Runtime, and every operation validates its generation.
#[derive(Clone, Copy)]
pub(crate) struct OwnerHandle {
    runtime: RuntimeId,
    key: Key,
}
impl Owner {
    pub(crate) fn handle(&self) -> OwnerHandle {
        OwnerHandle {
            runtime: self.runtime,
            key: self.key,
        }
    }
    /// Derived maps created by this component/factory belong to this owner,
    /// even when their source is application-wide. The context never owns data.
    pub fn enter<R>(&self, f: impl FnOnce() -> R) -> R {
        assert!(self.is_alive(), "enter live owner");
        enter_owner(self.runtime, self.key, f)
    }
    pub fn runtime(&self) -> RuntimeId {
        self.runtime
    }
    pub fn is_alive(&self) -> bool {
        lookup(self.runtime).is_ok_and(|state| state.borrow().owners.get(self.key).is_some())
    }
    pub fn child(&self) -> Self {
        make_owner(
            self.runtime,
            &lookup(self.runtime).expect("live runtime"),
            self.key,
        )
        .expect("live owner")
    }
    pub fn dispose(&self) {
        if let Ok(state) = lookup(self.runtime) {
            let cleanup = state.borrow_mut().dispose_owner(self.key);
            cleanup.run(self.runtime);
        }
    }
    /// Run untracked after this Owner and its descendants have been invalidated.
    /// Implicit map/path construction remains in the retired Owner and fails;
    /// explicit entry into another live Owner is allowed. Callbacks run without
    /// graph borrows, in reverse registration order (children before parents).
    /// A panic unwinds normally; there is no catch/retry of remaining callbacks.
    pub fn on_cleanup(&self, cleanup: impl FnOnce() + 'static) -> Result<(), AccessError> {
        let state = lookup(self.runtime)?;
        state
            .borrow_mut()
            .owners
            .get_mut(self.key)
            .ok_or(AccessError::Disposed)?
            .cleanup
            .push(Box::new(cleanup));
        Ok(())
    }

    pub fn cage<T: 'static>(&self, value: T) -> Cage<T> {
        allow_creation().expect("state creation is not allowed in a pure computation");
        let node = Node {
            owner: self.key,
            compute: Compute::Source,
            value: Some(Rc::new(RefCell::new(Box::new(value)))),
            version: 1,
            status: Status::Clean,
            rerun: false,
            propagated: false,
            last_flush: 0,
            flush_runs: 0,
            dependencies: BTreeMap::new(),
            subscribers: BTreeSet::new(),
        };
        let key = lookup(self.runtime)
            .expect("live runtime")
            .borrow_mut()
            .insert_node(self.key, node)
            .expect("live owner");
        Cage {
            handle: Handle::new(self.runtime, key),
        }
    }

    pub fn bond<T: 'static>(&self, mapper: impl Fn() -> T + 'static) -> Bond<T> {
        make_bond(self.runtime, self.key, mapper).expect("live owner")
    }

    pub fn binding(&self, callback: impl FnMut() + 'static) -> BindingId {
        self.observer(callback, 1)
    }
    /// Effects run after currently queued bindings. Writes enqueue another pass;
    /// reads still form direct dependencies, and runaway loops fail explicitly.
    pub fn effect(&self, callback: impl FnMut() + 'static) -> BindingId {
        self.observer(callback, 2)
    }
    /// Structural controllers update owned row state before prop bindings run.
    /// Use this for tree control flow; ordinary computed props remain pure.
    pub fn structural(&self, callback: impl FnMut() + 'static) -> BindingId {
        self.observer(callback, 0)
    }
    fn observer(&self, callback: impl FnMut() + 'static, phase: u8) -> BindingId {
        allow_creation().expect("observer creation is not allowed in a pure computation");
        let callback = Rc::new(RefCell::new(Box::new(callback) as Box<dyn FnMut()>));
        let compute = match phase {
            0 => Compute::Structure(callback),
            1 => Compute::Binding(callback),
            _ => Compute::Effect(callback),
        };
        let node = Node {
            owner: self.key,
            compute,
            value: None,
            version: 0,
            status: Status::Dirty,
            rerun: false,
            propagated: false,
            last_flush: 0,
            flush_runs: 0,
            dependencies: BTreeMap::new(),
            subscribers: BTreeSet::new(),
        };
        let key = lookup(self.runtime)
            .expect("live runtime")
            .borrow_mut()
            .insert_node(self.key, node)
            .expect("live owner");
        BindingId {
            runtime: self.runtime,
            key,
            local: PhantomData,
        }
    }

    pub fn task(&self) -> Task {
        self.handle().task()
    }

    pub fn spawn(
        &self,
        future: impl Future<Output = ()> + 'static,
    ) -> Result<TaskToken, AccessError> {
        self.handle().spawn(future)
    }
}
impl OwnerHandle {
    fn task(self) -> Task {
        allow_creation().expect("task creation is not allowed in a pure computation");
        let state = lookup(self.runtime).expect("live runtime");
        let mut state = state.borrow_mut();
        assert!(state.owners.get(self.key).is_some(), "live owner");
        let key = state.tasks.insert(TaskState {
            owner: self.key,
            cancel: None,
        });
        state
            .owners
            .get_mut(self.key)
            .expect("live owner")
            .tasks
            .insert(key);
        Task {
            token: TaskToken {
                runtime: self.runtime,
                key,
                local: PhantomData,
            },
            detached: false,
        }
    }

    pub(crate) fn spawn(
        self,
        future: impl Future<Output = ()> + 'static,
    ) -> Result<TaskToken, AccessError> {
        allow_creation()?;
        let state = lookup(self.runtime)?;
        if state.borrow().owners.get(self.key).is_none() {
            return Err(AccessError::Disposed);
        }
        let host = state.borrow().host.clone().ok_or(AccessError::NoHost)?;
        let task = self.task();
        let token = task.token();
        struct Completion(TaskToken);
        impl Drop for Completion {
            fn drop(&mut self) {
                finish_task(self.0, false);
            }
        }
        let cancel = host
            .spawn(Box::pin(async move {
                let _completion = Completion(token);
                if token.is_current() {
                    let mut future = Box::pin(future);
                    std::future::poll_fn(move |context| {
                        let Ok(state) = lookup(token.runtime) else {
                            return std::task::Poll::Ready(());
                        };
                        let owner = state.borrow().tasks.get(token.key).map(|task| task.owner);
                        let Some(owner) = owner else {
                            return std::task::Poll::Ready(());
                        };
                        let profile = state.borrow().profile.clone();
                        let _span = profile.map(|profile| profile.span(Phase::Task));
                        enter_owner(token.runtime, owner, || future.as_mut().poll(context))
                    })
                    .await;
                }
            }))
            .map_err(AccessError::Host)?;
        let mut cancel = Some(cancel);
        if let Some(task) = state.borrow_mut().tasks.get_mut(token.key) {
            task.cancel = cancel.take();
        }
        if let Some(cancel) = cancel {
            cancel();
        }
        Ok(task.detach())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.dispose();
    }
}
impl std::fmt::Debug for Owner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Owner")
            .field("runtime", &self.runtime)
            .field("key", &self.key)
            .finish()
    }
}

fn make_owner(runtime: RuntimeId, state: &Shared, parent: Key) -> Result<Owner, AccessError> {
    let mut state = state.borrow_mut();
    if state.owners.get(parent).is_none() {
        return Err(AccessError::Disposed);
    }
    let key = state.owners.insert(OwnerData {
        parent: Some(parent),
        ..OwnerData::default()
    });
    state
        .owners
        .get_mut(parent)
        .expect("live parent")
        .children
        .insert(key);
    Ok(Owner {
        runtime,
        key,
        local: PhantomData,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InstanceId {
    runtime: RuntimeId,
    key: Key,
}
impl InstanceId {
    /// Whether this generational instance and its Owner are still live.
    /// Does not enter construction context or invoke application code.
    pub fn is_alive(self) -> bool {
        lookup(self.runtime).is_ok_and(|state| {
            let state = state.borrow();
            state
                .instances
                .get(self.key)
                .is_some_and(|instance| state.owners.get(instance.owner).is_some())
        })
    }
    pub fn runtime(self) -> RuntimeId {
        self.runtime
    }
    pub fn slot(self) -> u32 {
        self.key.slot
    }
    pub fn generation(self) -> u32 {
        self.key.generation
    }
}

#[derive(Debug)]
pub struct Scope {
    owner: Owner,
    id: InstanceId,
}
impl Scope {
    pub fn id(&self) -> InstanceId {
        self.id
    }
    pub fn owner(&self) -> &Owner {
        &self.owner
    }
    pub fn child(&self) -> Self {
        make_scope(self.owner.child(), Some(self.id.key)).expect("live scope")
    }
    pub fn parent(&self) -> Option<InstanceId> {
        let state = lookup(self.id.runtime).ok()?;
        let state = state.borrow();
        let parent = state.instances.get(self.id.key)?.parent?;
        Some(InstanceId {
            runtime: self.id.runtime,
            key: parent,
        })
    }
    pub fn is_alive(&self) -> bool {
        lookup(self.id.runtime).is_ok_and(|state| {
            state
                .borrow()
                .instances
                .get(self.id.key)
                .is_some_and(|instance| instance.owner == self.owner.key)
        })
    }
}
impl std::ops::Deref for Scope {
    type Target = Owner;
    fn deref(&self) -> &Owner {
        &self.owner
    }
}
fn make_scope(owner: Owner, parent: Option<Key>) -> Result<Scope, AccessError> {
    let state = lookup(owner.runtime)?;
    let mut state = state.borrow_mut();
    if state.owners.get(owner.key).is_none() {
        return Err(AccessError::Disposed);
    }
    if let Some(parent) = parent
        && state.instances.get(parent).is_none()
    {
        return Err(AccessError::Disposed);
    }
    let key = state.instances.insert(Instance {
        owner: owner.key,
        parent,
    });
    state
        .owners
        .get_mut(owner.key)
        .expect("live owner")
        .instance = Some(key);
    Ok(Scope {
        id: InstanceId {
            runtime: owner.runtime,
            key,
        },
        owner,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BindingId {
    runtime: RuntimeId,
    key: Key,
    local: PhantomData<Rc<()>>,
}
impl BindingId {
    pub fn dispose(self) {
        if let Ok(state) = lookup(self.runtime) {
            let mut cleanup = Cleanup::default();
            state.borrow_mut().remove_node(self.key, &mut cleanup);
            cleanup.run(self.runtime);
        }
    }
    pub fn is_alive(self) -> bool {
        lookup(self.runtime).is_ok_and(|state| state.borrow().nodes.get(self.key).is_some())
    }
}

struct Handle<T: 'static> {
    runtime: RuntimeId,
    key: Key,
    local: PhantomData<Rc<T>>,
}
impl<T> Copy for Handle<T> {}
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Handle<T> {
    fn new(runtime: RuntimeId, key: Key) -> Self {
        Self {
            runtime,
            key,
            local: PhantomData,
        }
    }
    fn with<R>(self, tracked: bool, f: impl FnOnce(&T) -> R) -> Result<R, AccessError> {
        fields::allow_read()?;
        if tracked {
            check_reader(self.runtime)?;
        }
        let state = lookup(self.runtime)?;
        ensure(self.runtime, &state, self.key)?;
        let (path, version) = {
            let state = state.borrow();
            let node = state.nodes.get(self.key).ok_or(AccessError::Disposed)?;
            (state.value_path(self.key, node)?, node.version)
        };
        if tracked {
            track(self.runtime, self.key, version)?;
        }
        let value = path.value.try_borrow().map_err(|_| AccessError::Borrowed)?;
        let value = path.read(value.as_ref())?;
        Ok(f(value.downcast_ref().ok_or(AccessError::TypeMismatch)?))
    }
}

/// Copy, thread-local reactive identity. `get` returns an owned clone; use
/// `with` for large collections so indexed reads never clone the whole data set.
///
/// ```compile_fail
/// let runtime = glory_core::Runtime::new();
/// let owner = runtime.owner();
/// let value = owner.cage(1);
/// std::thread::spawn(move || value.get()); // Handles cannot cross UI threads.
/// ```
pub struct Cage<T: 'static> {
    handle: Handle<T>,
}
pub struct Bond<T: 'static> {
    handle: Handle<T>,
}
macro_rules! read_handle {
    ($name:ident) => {
        impl<T> Copy for $name<T> {}
        impl<T> Clone for $name<T> {
            fn clone(&self) -> Self {
                *self
            }
        }
        impl<T> std::fmt::Debug for $name<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("runtime", &self.handle.runtime)
                    .field("key", &self.handle.key)
                    .finish()
            }
        }
        impl<T> $name<T> {
            pub fn runtime(self) -> RuntimeId {
                self.handle.runtime
            }
            pub fn try_with<R>(self, f: impl FnOnce(&T) -> R) -> Result<R, AccessError> {
                self.handle.with(true, f)
            }
            pub fn with<R>(self, f: impl FnOnce(&T) -> R) -> R {
                self.try_with(f).expect("read live reactive value")
            }
            pub fn try_with_untracked<R>(self, f: impl FnOnce(&T) -> R) -> Result<R, AccessError> {
                self.handle.with(false, f)
            }
            pub fn with_untracked<R>(self, f: impl FnOnce(&T) -> R) -> R {
                self.try_with_untracked(f)
                    .expect("read live reactive value")
            }
            pub fn try_get(self) -> Result<T, AccessError>
            where
                T: Clone,
            {
                self.try_with(Clone::clone)
            }
            pub fn get(self) -> T
            where
                T: Clone,
            {
                self.with(Clone::clone)
            }
            pub fn get_untracked(self) -> T
            where
                T: Clone,
            {
                self.with_untracked(Clone::clone)
            }
            pub fn is_alive(self) -> bool {
                lookup(self.handle.runtime)
                    .is_ok_and(|state| state.borrow().nodes.get(self.handle.key).is_some())
            }
        }
    };
}
read_handle!(Cage);
read_handle!(Bond);

struct Write {
    state: Shared,
    key: Key,
}
impl Drop for Write {
    fn drop(&mut self) {
        let host = {
            let mut state = self.state.borrow_mut();
            state.publish(self.key);
            state.host.clone()
        };
        if let Some(host) = host {
            host.wake();
        }
    }
}
impl<T> Cage<T> {
    pub fn try_revise<R>(self, f: impl FnOnce(&mut T) -> R) -> Result<R, AccessError> {
        if EVALUATING.with_borrow(|stack| stack.last().is_some_and(|mode| *mode == Mode::Pure)) {
            return Err(AccessError::WriteDuringCompute);
        }
        let state = lookup(self.handle.runtime)?;
        let value = {
            let state = state.borrow();
            let node = state
                .nodes
                .get(self.handle.key)
                .ok_or(AccessError::Disposed)?;
            if !matches!(node.compute, Compute::Source) {
                return Err(AccessError::TypeMismatch);
            }
            node.version
                .checked_add(1)
                .expect("source version exhausted");
            if state.fields.has_root(self.handle.key) {
                None
            } else {
                node.value.clone()
            }
        };
        let Some(value) = value else {
            return fields::revise(state, self.handle.key, f);
        };
        let mut borrowed = value.try_borrow_mut().map_err(|_| AccessError::Borrowed)?;
        let value = borrowed.downcast_mut().ok_or(AccessError::TypeMismatch)?;
        let write = Write {
            state,
            key: self.handle.key,
        };
        let result = f(value);
        drop(borrowed);
        drop(write);
        Ok(result)
    }
    pub fn revise<R>(self, f: impl FnOnce(&mut T) -> R) -> R {
        self.try_revise(f)
            .expect("revise live reactive value outside pure computation")
    }
    pub fn map<U: 'static>(self, mapper: impl Fn(&T) -> U + 'static) -> Bond<U> {
        let state = lookup(self.handle.runtime).expect("live runtime");
        let source_owner = state
            .borrow()
            .nodes
            .get(self.handle.key)
            .expect("live source")
            .owner;
        let owner = CONSTRUCTING.with_borrow(|stack| {
            stack
                .last()
                .filter(|(runtime, _)| *runtime == self.handle.runtime)
                .map(|(_, owner)| *owner)
                .unwrap_or(source_owner)
        });
        make_bond(self.handle.runtime, owner, move || self.with(&mapper)).expect("live owner")
    }
}

fn make_bond<T: 'static>(
    runtime: RuntimeId,
    owner: Key,
    mapper: impl Fn() -> T + 'static,
) -> Result<Bond<T>, AccessError> {
    allow_creation()?;
    let compute = Compute::Derived {
        mapper: Rc::new(move || Box::new(mapper())),
        equal: None,
    };
    let node = Node {
        owner,
        compute,
        value: None,
        version: 0,
        status: Status::Dirty,
        rerun: false,
        propagated: false,
        last_flush: 0,
        flush_runs: 0,
        dependencies: BTreeMap::new(),
        subscribers: BTreeSet::new(),
    };
    let key = lookup(runtime)?.borrow_mut().insert_node(owner, node)?;
    Ok(Bond {
        handle: Handle::new(runtime, key),
    })
}
impl<T> Bond<T> {
    pub fn with_eq(self, equal: impl Fn(&T, &T) -> bool + 'static) -> Self {
        let state = lookup(self.handle.runtime).expect("live runtime");
        let mut state = state.borrow_mut();
        let node = state.nodes.get_mut(self.handle.key).expect("live bond");
        let Compute::Derived {
            equal: comparator, ..
        } = &mut node.compute
        else {
            unreachable!()
        };
        let previous = comparator.replace(Rc::new(move |a, b| {
            equal(
                a.downcast_ref().expect("bond type"),
                b.downcast_ref().expect("bond type"),
            )
        }));
        drop(state);
        drop(previous);
        self
    }
    pub fn with_partial_eq(self) -> Self
    where
        T: PartialEq,
    {
        self.with_eq(PartialEq::eq)
    }
}

#[derive(Clone, Debug)]
pub enum Lotus<T: 'static> {
    Value(T),
    Cage(Cage<T>),
    Bond(Bond<T>),
}
impl<T> Lotus<T> {
    pub fn try_with<R>(&self, f: impl FnOnce(&T) -> R) -> Result<R, AccessError> {
        match self {
            Self::Value(value) => Ok(f(value)),
            Self::Cage(value) => value.try_with(f),
            Self::Bond(value) => value.try_with(f),
        }
    }
    pub fn try_get(&self) -> Result<T, AccessError>
    where
        T: Clone,
    {
        self.try_with(Clone::clone)
    }
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        match self {
            Self::Value(value) => f(value),
            Self::Cage(value) => value.with(f),
            Self::Bond(value) => value.with(f),
        }
    }
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(Clone::clone)
    }
    pub fn is_reactive(&self) -> bool {
        !matches!(self, Self::Value(_))
    }
}
impl<T> From<T> for Lotus<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}
impl<T> From<Cage<T>> for Lotus<T> {
    fn from(value: Cage<T>) -> Self {
        Self::Cage(value)
    }
}
impl<T> From<Bond<T>> for Lotus<T> {
    fn from(value: Bond<T>) -> Self {
        Self::Bond(value)
    }
}
impl From<&str> for Lotus<String> {
    fn from(value: &str) -> Self {
        Self::Value(value.to_owned())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TaskToken {
    runtime: RuntimeId,
    key: Key,
    local: PhantomData<Rc<()>>,
}
impl TaskToken {
    pub fn is_current(self) -> bool {
        lookup(self.runtime).is_ok_and(|state| state.borrow().tasks.get(self.key).is_some())
    }
    pub fn commit<R>(self, f: impl FnOnce() -> R) -> Result<R, AccessError> {
        let state = lookup(self.runtime)?;
        let owner = state
            .borrow()
            .tasks
            .get(self.key)
            .ok_or(AccessError::Disposed)?
            .owner;
        Ok(enter_owner(self.runtime, owner, f))
    }
    pub fn cancel(self) {
        finish_task(self, true);
    }
}

pub struct Task {
    token: TaskToken,
    detached: bool,
}
impl Task {
    pub fn token(&self) -> TaskToken {
        self.token
    }
    pub fn on_cancel(&self, cancel: impl FnOnce() + 'static) -> Result<(), AccessError> {
        let state = lookup(self.token.runtime)?;
        let mut state = state.borrow_mut();
        let task = state
            .tasks
            .get_mut(self.token.key)
            .ok_or(AccessError::Disposed)?;
        let previous = task.cancel.take();
        task.cancel = Some(Box::new(move || {
            if let Some(previous) = previous {
                previous();
            }
            cancel();
        }));
        Ok(())
    }
    pub fn detach(mut self) -> TaskToken {
        self.detached = true;
        self.token
    }
    pub fn complete(mut self) {
        finish_task(self.token, false);
        self.detached = true;
    }
}
impl Drop for Task {
    fn drop(&mut self) {
        if !self.detached {
            finish_task(self.token, true);
        }
    }
}
fn finish_task(token: TaskToken, cancel: bool) {
    if let Ok(state) = lookup(token.runtime) {
        let task = state.borrow_mut().remove_task(token.key);
        if cancel
            && let Some(task) = task
            && let Some(cancel) = task.cancel
        {
            untracked(|| enter_owner(token.runtime, task.owner, cancel));
        }
    }
}
