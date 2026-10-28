//! Owned aliases of model paths. The data remains in the original Cage.
use super::*;
use std::any::TypeId;
use std::borrow::Cow;
use std::cell::Cell;
use std::mem::size_of;
#[path = "sparse.rs"]
mod sparse;
use sparse::SparseMap;
#[path = "entries.rs"]
mod entries;
use entries::{EntryIndex, EntryKey};
#[path = "collections.rs"]
mod collections;

thread_local! { static PROJECTING: Cell<bool> = const { Cell::new(false) }; }
pub(super) fn allow_read() -> Result<(), AccessError> {
    if PROJECTING.get() {
        Err(AccessError::InvalidField(
            "reactive reads inside a field accessor",
        ))
    } else {
        Ok(())
    }
}
pub(super) fn project<R>(f: impl FnOnce() -> Result<R, AccessError>) -> Result<R, AccessError> {
    struct Guard {
        previous: bool,
        mode: usize,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            PROJECTING.set(self.previous);
            EVALUATING.with_borrow_mut(|stack| stack.truncate(self.mode));
        }
    }
    let previous = PROJECTING.replace(true);
    let mode = EVALUATING.with_borrow_mut(|stack| {
        let n = stack.len();
        stack.push(Mode::Pure);
        n
    });
    let _guard = Guard { previous, mode };
    f()
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FieldId(Key);
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Parent {
    Root(Key),
    Field(FieldId),
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Segment {
    Field(Cow<'static, str>),
    Item(usize),
    Entry(u64),
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Layout {
    offset: usize,
    size: usize,
}
trait Projection {
    fn read<'a>(&self, value: &'a dyn Any) -> Result<&'a dyn Any, AccessError>;
    fn write<'a>(&self, value: &'a mut dyn Any) -> Result<&'a mut dyn Any, AccessError>;
}
struct Map<T, U, G, M> {
    parent: Option<Rc<dyn Projection>>,
    get: G,
    get_mut: M,
    offset: usize,
    types: PhantomData<fn(T) -> U>,
}
impl<T: 'static, U: 'static, G, M> Projection for Map<T, U, G, M>
where
    G: for<'a> Fn(&'a T) -> &'a U,
    M: for<'a> Fn(&'a mut T) -> &'a mut U,
{
    fn read<'a>(&self, value: &'a dyn Any) -> Result<&'a dyn Any, AccessError> {
        let value = if let Some(parent) = &self.parent {
            parent.read(value)?
        } else {
            value
        };
        let value = value.downcast_ref::<T>().ok_or(AccessError::TypeMismatch)?;
        let selected = (self.get)(value);
        if offset(value, selected)? != self.offset {
            return Err(AccessError::InvalidField(
                "field accessor changed its location",
            ));
        }
        Ok(selected)
    }
    fn write<'a>(&self, value: &'a mut dyn Any) -> Result<&'a mut dyn Any, AccessError> {
        let value = if let Some(parent) = &self.parent {
            parent.write(value)?
        } else {
            value
        };
        let value = value.downcast_mut::<T>().ok_or(AccessError::TypeMismatch)?;
        let start = std::ptr::from_ref(value) as usize;
        let selected = (self.get_mut)(value);
        let actual = (std::ptr::from_ref(selected) as usize).checked_sub(start);
        if actual != Some(self.offset) {
            return Err(AccessError::InvalidField(
                "mutable accessor selects another field",
            ));
        }
        Ok(selected)
    }
}
fn offset<T, U>(parent: &T, child: &U) -> Result<usize, AccessError> {
    let relative = (std::ptr::from_ref(child) as usize)
        .checked_sub(std::ptr::from_ref(parent) as usize)
        .ok_or(AccessError::InvalidField(
            "field is outside its inline parent",
        ))?;
    if relative
        .checked_add(size_of::<U>())
        .is_none_or(|end| end > size_of::<T>())
    {
        return Err(AccessError::InvalidField(
            "field is outside its inline parent",
        ));
    }
    Ok(relative)
}
pub(super) struct FieldData {
    parent: Parent,
    root: Key,
    segment: Segment,
    output: TypeId,
    layout: Option<Layout>,
    depth: usize,
    projection: Rc<dyn Projection>,
    children: SparseMap<Segment, FieldId>,
    // Exactly one alias node per live Owner; owner generations prevent reuse.
    aliases: SparseMap<Key, Key>,
    _entry: Option<Rc<EntryKey>>,
}
#[derive(Default)]
pub(super) struct Fields {
    roots: BTreeMap<Key, SparseMap<Segment, FieldId>>,
    slots: Arena<FieldData>,
    aliases: BTreeMap<Key, FieldId>,
    indices: BTreeMap<Parent, Rc<EntryIndex>>,
    targets: BTreeMap<Key, Box<CachedTarget>>,
}
#[derive(Default)]
pub(super) struct Removal {
    pub aliases: Vec<Key>,
    pub fields: Vec<FieldData>,
}
pub(super) struct ValuePath {
    pub value: Value,
    root: Key,
    projection: Option<Rc<dyn Projection>>,
}
impl ValuePath {
    pub fn read<'a>(&self, value: &'a dyn Any) -> Result<&'a dyn Any, AccessError> {
        self.projection
            .as_ref()
            .map_or(Ok(value), |p| project(|| p.read(value)))
    }
    pub fn write<'a>(&self, value: &'a mut dyn Any) -> Result<&'a mut dyn Any, AccessError> {
        if let Some(p) = &self.projection {
            project(|| p.write(value))
        } else {
            Ok(value)
        }
    }
}
struct WriteTarget {
    root: Key,
    at: Target,
    nodes: Vec<Key>,
    pending: Vec<FieldId>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Root,
    Field(FieldId),
    Collection,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct CollectionTarget {
    parent: Parent,
    selection: Selection,
}
/// A logical set of direct collection children, never business keys/values or
/// addresses. The one per-root target buffer is reused for these writes too.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Selection {
    Entry(Option<u64>),
    Items { start: usize, end: usize },
}
#[derive(Default)]
struct CachedTarget {
    buffer: Option<WriteTarget>,
    keys: usize,
    // Keep the numeric selector out of ordinary field write arguments/guards.
    // Reuse one cold allocation per root, never one record per visited key.
    collection: Option<Box<CollectionTarget>>,
}
impl Fields {
    pub fn target_stats(&self) -> (usize, usize) {
        (
            self.targets.len(),
            self.targets.values().map(|target| target.keys).sum(),
        )
    }
    fn invalidate_target(&mut self, root: Key) {
        // Removing this slot also revokes an in-flight writer's return permit.
        // New paths cannot register during its mutable root borrow, so a
        // removed slot cannot be recreated until that write has finished.
        self.targets.remove(&root);
    }
    pub fn entry_stats(&self) -> (usize, usize) {
        self.indices
            .values()
            .map(|index| index.stats())
            .fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1))
    }
    pub fn has_root(&self, key: Key) -> bool {
        self.roots.contains_key(&key)
    }
    pub fn len(&self) -> usize {
        self.slots.len()
    }
    pub fn capacity(&self) -> usize {
        self.slots.capacity()
    }
    fn children(&self, parent: Parent) -> Option<&SparseMap<Segment, FieldId>> {
        match parent {
            Parent::Root(root) => self.roots.get(&root),
            Parent::Field(id) => self.slots.get(id.0).map(|f| &f.children),
        }
    }
    fn selected(
        &self,
        parent: Parent,
        selection: Selection,
    ) -> Option<sparse::Range<'_, Segment, FieldId>> {
        use std::ops::Bound::{Excluded, Included};
        let bounds = match selection {
            Selection::Entry(Some(id)) => {
                (Included(Segment::Entry(id)), Included(Segment::Entry(id)))
            }
            Selection::Entry(None) => return None,
            Selection::Items { start, end } => {
                (Included(Segment::Item(start)), Excluded(Segment::Item(end)))
            }
        };
        self.children(parent).map(|children| children.range(bounds))
    }
    fn unlink(&mut self, parent: Parent, segment: &Segment) {
        let empty = match parent {
            Parent::Root(root) => self.roots.get_mut(&root).is_some_and(|children| {
                children.remove(segment);
                children.is_empty()
            }),
            Parent::Field(id) => self.slots.get_mut(id.0).is_some_and(|field| {
                field.children.remove(segment);
                field.children.is_empty()
            }),
        };
        if empty {
            self.indices.remove(&parent);
            if let Parent::Root(root) = parent {
                self.roots.remove(&root);
            }
        }
    }
    pub fn detach(&mut self, node: Key, owner: Key) -> Removal {
        let mut removal = Removal::default();
        if let Some(mut id) = self.aliases.remove(&node) {
            if let Some(field) = self.slots.get(id.0) {
                self.invalidate_target(field.root);
            }
            if let Some(field) = self.slots.get_mut(id.0) {
                field.aliases.remove(&owner);
            }
            while let Some(field) = self.slots.get(id.0) {
                if !field.aliases.is_empty() || !field.children.is_empty() {
                    break;
                }
                let field = self.slots.remove(id.0).expect("live field");
                self.indices.remove(&Parent::Field(id));
                self.unlink(field.parent, &field.segment);
                match field.parent {
                    Parent::Root(_) => {
                        removal.fields.push(field);
                        break;
                    }
                    Parent::Field(parent) => {
                        removal.fields.push(field);
                        id = parent;
                    }
                }
            }
        }
        self.indices.remove(&Parent::Root(node));
        if let Some(roots) = self.roots.remove(&node) {
            self.invalidate_target(node);
            let mut pending: Vec<_> = roots.into_values().collect();
            while let Some(id) = pending.pop() {
                if let Some(field) = self.slots.remove(id.0) {
                    self.indices.remove(&Parent::Field(id));
                    pending.extend(field.children.values().copied());
                    for alias in field.aliases.values() {
                        self.aliases.remove(alias);
                        removal.aliases.push(*alias);
                    }
                    removal.fields.push(field);
                }
            }
        }
        removal
    }
}
impl State {
    pub(super) fn value_path(&self, key: Key, node: &Node) -> Result<ValuePath, AccessError> {
        if let Some(value) = &node.value {
            return Ok(ValuePath {
                value: value.clone(),
                root: key,
                projection: None,
            });
        }
        let id = *self
            .fields
            .aliases
            .get(&key)
            .ok_or(AccessError::TypeMismatch)?;
        self.field_path(id)
    }
    fn field_path(&self, id: FieldId) -> Result<ValuePath, AccessError> {
        let field = self.fields.slots.get(id.0).ok_or(AccessError::Disposed)?;
        let value = self
            .nodes
            .get(field.root)
            .and_then(|n| n.value.clone())
            .ok_or(AccessError::Disposed)?;
        Ok(ValuePath {
            value,
            root: field.root,
            projection: Some(field.projection.clone()),
        })
    }
    pub(super) fn advance_source(&mut self, key: Key) {
        let Some(node) = self.nodes.get_mut(key) else {
            return;
        };
        node.version = node
            .version
            .checked_add(1)
            .expect("source version exhausted");
        if node.propagated {
            return;
        }
        node.propagated = true;
        let subscribers: Vec<_> = node.subscribers.iter().copied().collect();
        self.invalidate(subscribers);
    }
    fn write_target(&mut self, root: Key, at: Target) -> Result<WriteTarget, AccessError> {
        debug_assert!(!matches!(at, Target::Collection));
        let cached = self
            .fields
            .targets
            .get(&root)
            .and_then(|slot| slot.buffer.as_ref());
        let hit = cached.is_some_and(|target| target.at == at);
        // Precheck before taking/modifying a buffer. Exhausted versions cannot
        // leave a changed cache, run business code or publish a partial write.
        let mut check = |key| {
            if let Some(node) = self.nodes.get(key) {
                node.version
                    .checked_add(1)
                    .expect("source version exhausted");
            }
        };
        if let Some(target) = cached.filter(|_| hit) {
            for key in &target.nodes {
                check(*key);
            }
        } else {
            check(root);
            if let Target::Field(id) = at {
                let field = self.fields.slots.get(id.0).ok_or(AccessError::Disposed)?;
                let mut parent = field.parent;
                while let Parent::Field(id) = parent {
                    let field = self.fields.slots.get(id.0).ok_or(AccessError::Disposed)?;
                    for key in field.aliases.values() {
                        check(*key);
                    }
                    parent = field.parent;
                }
                self.visit_target(id, &mut check)?;
            } else {
                let children = self
                    .fields
                    .roots
                    .get(&root)
                    .ok_or(AccessError::InvalidField("source has no registered field"))?;
                for id in children.values() {
                    self.visit_target(*id, &mut check)?;
                }
            }
        }
        let mut target = self
            .fields
            .targets
            .get_mut(&root)
            .and_then(|slot| slot.buffer.take())
            .unwrap_or_else(|| WriteTarget {
                root,
                at,
                nodes: Vec::new(),
                pending: Vec::new(),
            });
        if hit {
            self.stats.field_target_hits += 1;
        } else {
            self.collect_target(root, at, &mut target);
            self.stats.field_target_builds += 1;
            self.stats.field_target_visits += target.nodes.len() as u64;
        }
        // The empty slot is the writer's return permit. Alias/schema changes
        // remove it. Only counts and cold numeric metadata remain in the slot.
        self.fields.targets.entry(root).or_default().keys = target.nodes.len();
        Ok(target)
    }
    fn write_collection_target(
        &mut self,
        root: Key,
        parent: Parent,
        selection: Selection,
    ) -> Result<WriteTarget, AccessError> {
        let selector = CollectionTarget { parent, selection };
        let slot = self.fields.targets.get(&root);
        let cached = slot.and_then(|slot| slot.buffer.as_ref());
        let hit = cached.is_some_and(|target| target.at == Target::Collection)
            && slot.and_then(|slot| slot.collection.as_deref()) == Some(&selector);
        // As with ordinary writes, validation cannot change cache/counters or
        // take the buffer until every affected version has been checked.
        let mut check = |key| {
            if let Some(node) = self.nodes.get(key) {
                node.version
                    .checked_add(1)
                    .expect("source version exhausted");
            }
        };
        if let Some(target) = cached.filter(|_| hit) {
            for key in &target.nodes {
                check(*key);
            }
        } else {
            check(root);
            let mut ancestor = parent;
            while let Parent::Field(id) = ancestor {
                let field = self.fields.slots.get(id.0).ok_or(AccessError::Disposed)?;
                for key in field.aliases.values() {
                    check(*key);
                }
                ancestor = field.parent;
            }
            for (_, id) in self
                .fields
                .selected(parent, selection)
                .into_iter()
                .flatten()
            {
                self.visit_target(*id, &mut check)?;
            }
        }
        let mut target = self
            .fields
            .targets
            .get_mut(&root)
            .and_then(|slot| slot.buffer.take())
            .unwrap_or_else(|| WriteTarget {
                root,
                at: Target::Collection,
                nodes: Vec::new(),
                pending: Vec::new(),
            });
        if hit {
            self.stats.field_target_hits += 1;
        } else {
            target.root = root;
            target.at = Target::Collection;
            target.nodes.clear();
            target.pending.clear();
            target.nodes.push(root);
            let mut ancestor = parent;
            while let Parent::Field(id) = ancestor {
                let field = self.fields.slots.get(id.0).expect("prechecked ancestor");
                target.nodes.extend(field.aliases.values().copied());
                ancestor = field.parent;
            }
            target.pending.extend(
                self.fields
                    .selected(parent, selection)
                    .into_iter()
                    .flatten()
                    .map(|(_, id)| *id),
            );
            while let Some(id) = target.pending.pop() {
                let field = self.fields.slots.get(id.0).expect("prechecked descendant");
                target.nodes.extend(field.aliases.values().copied());
                target.pending.extend(field.children.values().copied());
            }
            self.stats.field_target_builds += 1;
            self.stats.field_target_visits += target.nodes.len() as u64;
        }
        let slot = self.fields.targets.entry(root).or_default();
        slot.keys = target.nodes.len();
        if !hit {
            if let Some(previous) = &mut slot.collection {
                **previous = selector;
            } else {
                slot.collection = Some(Box::new(selector));
            }
        }
        Ok(target)
    }
    fn visit_target(&self, id: FieldId, visit: &mut impl FnMut(Key)) -> Result<(), AccessError> {
        let field = self.fields.slots.get(id.0).ok_or(AccessError::Disposed)?;
        for key in field.aliases.values() {
            visit(*key);
        }
        // Path depth is capped at 128; validation needs no heap allocation.
        for child in field.children.values() {
            self.visit_target(*child, visit)?;
        }
        Ok(())
    }
    fn collect_target(&self, root: Key, at: Target, target: &mut WriteTarget) {
        target.root = root;
        target.at = at;
        target.nodes.clear();
        target.pending.clear();
        target.nodes.push(root);
        if let Target::Field(id) = at {
            let field = self.fields.slots.get(id.0).expect("prechecked field");
            let mut parent = field.parent;
            while let Parent::Field(id) = parent {
                let field = self.fields.slots.get(id.0).expect("prechecked ancestor");
                target.nodes.extend(field.aliases.values().copied());
                parent = field.parent;
            }
            target.pending.push(id);
        } else {
            target.pending.extend(
                self.fields
                    .roots
                    .get(&root)
                    .expect("prechecked root")
                    .values()
                    .copied(),
            );
        }
        while let Some(id) = target.pending.pop() {
            let field = self.fields.slots.get(id.0).expect("prechecked descendant");
            target.nodes.extend(field.aliases.values().copied());
            target.pending.extend(field.children.values().copied());
        }
    }
}

struct Write {
    state: Shared,
    target: Option<WriteTarget>,
}
impl Drop for Write {
    fn drop(&mut self) {
        let target = self.target.take().expect("one write guard");
        let host = {
            let mut state = self.state.borrow_mut();
            if state.nodes.get(target.root).is_some() {
                state.stats.source_writes += 1;
                for key in &target.nodes {
                    state.advance_source(*key);
                }
            }
            let host = state.host.clone();
            if let Some(slot) = state.fields.targets.get_mut(&target.root) {
                slot.buffer = Some(target);
            }
            host
        };
        if let Some(host) = host {
            host.wake();
        }
    }
}
// Keep projection/alias work out of the ordinary Cage write path. A field
// cannot be registered while that root value is mutably borrowed.
pub(super) fn revise<T: 'static, R>(
    state: Shared,
    key: Key,
    f: impl FnOnce(&mut T) -> R,
) -> Result<R, AccessError> {
    let (path, at) = {
        let state = state.borrow();
        let node = state.nodes.get(key).ok_or(AccessError::Disposed)?;
        let (path, at) = if node.value.is_some() {
            (state.value_path(key, node)?, Target::Root)
        } else {
            let id = *state
                .fields
                .aliases
                .get(&key)
                .ok_or(AccessError::TypeMismatch)?;
            (state.field_path(id)?, Target::Field(id))
        };
        (path, at)
    };
    let mut borrowed = path
        .value
        .try_borrow_mut()
        .map_err(|_| AccessError::Borrowed)?;
    let value = path
        .write(borrowed.as_mut())?
        .downcast_mut()
        .ok_or(AccessError::TypeMismatch)?;
    let target = state.borrow_mut().write_target(path.root, at)?;
    let write = Write {
        state,
        target: Some(target),
    };
    let result = f(value);
    drop(borrowed);
    drop(write);
    Ok(result)
}

impl<T> Cage<T> {
    /// Borrow and track a named inline field without cloning its parent model.
    /// Accessors must be pure, select the same fixed subobject, and have no
    /// reactive reads. Different sibling names must denote disjoint storage;
    /// overlapping views use a common parent field then nested fields instead.
    /// Heap elements/Deref projections and moving enum layouts are rejected.
    /// Use `item`/`entry` for the supported Vec and keyed-map projections.
    /// Aliases belong to the constructing Owner (or this Cage's owner outside
    /// construction); the last alias/child releases the canonical field path.
    /// Field writes notify whole-model and ancestor readers, plus this field's
    /// aliases and descendants. Whole-model writes conservatively notify every
    /// registered field. No value equality is implied by either operation.
    ///
    /// ```
    /// let runtime = glory_core::Runtime::new();
    /// let owner = runtime.owner();
    /// let model = owner.cage((String::from("hello"), 0usize));
    /// let title = model.field("title", |m| &m.0, |m| &mut m.0);
    /// title.revise(|s| s.push('!'));
    /// assert_eq!(model.with(|m| m.0.clone()), "hello!");
    /// ```
    pub fn field<U: 'static>(
        self,
        name: impl Into<Cow<'static, str>>,
        get: impl for<'a> Fn(&'a T) -> &'a U + 'static,
        get_mut: impl for<'a> Fn(&'a mut T) -> &'a mut U + 'static,
    ) -> Cage<U> {
        self.try_field(name, get, get_mut)
            .expect("create a pure, disjoint inline field on a live Cage")
    }
    pub fn try_field<U: 'static>(
        self,
        name: impl Into<Cow<'static, str>>,
        get: impl for<'a> Fn(&'a T) -> &'a U + 'static,
        get_mut: impl for<'a> Fn(&'a mut T) -> &'a mut U + 'static,
    ) -> Result<Cage<U>, AccessError> {
        allow_creation()?;
        let name = name.into();
        if name.is_empty() {
            return Err(AccessError::InvalidField("empty field name"));
        }
        let context = Context::new(self.handle.runtime, self.handle.key)?;
        inline(context, name, get, get_mut)
    }
}

fn inline<T: 'static, U: 'static>(
    context: Context,
    name: Cow<'static, str>,
    get: impl for<'a> Fn(&'a T) -> &'a U + 'static,
    get_mut: impl for<'a> Fn(&'a mut T) -> &'a mut U + 'static,
) -> Result<Cage<U>, AccessError> {
    if name.is_empty() {
        return Err(AccessError::InvalidField("empty field name"));
    }
    let selected_offset = {
        let borrowed = context
            .path
            .value
            .try_borrow()
            .map_err(|_| AccessError::Borrowed)?;
        project(|| {
            let parent = context
                .path
                .read(borrowed.as_ref())?
                .downcast_ref::<T>()
                .ok_or(AccessError::TypeMismatch)?;
            offset(parent, get(parent))
        })?
    };
    let parent = context.path.projection.clone();
    context.register(
        Segment::Field(name),
        Some(Layout {
            offset: selected_offset,
            size: size_of::<U>(),
        }),
        move || {
            Rc::new(Map::<T, U, _, _> {
                parent,
                get,
                get_mut,
                offset: selected_offset,
                types: PhantomData,
            }) as Rc<dyn Projection>
        },
        None,
    )
}
struct PathGuard {
    state: Shared,
    id: FieldId,
}
impl Drop for PathGuard {
    fn drop(&mut self) {
        let removed = {
            let mut graph = self.state.borrow_mut();
            let mut removed = Vec::new();
            let mut id = self.id;
            while let Some(field) = graph.fields.slots.get(id.0) {
                if !field.aliases.is_empty() || !field.children.is_empty() {
                    break;
                }
                let field = graph.fields.slots.remove(id.0).expect("live path");
                graph.fields.invalidate_target(field.root);
                graph.fields.indices.remove(&Parent::Field(id));
                graph.fields.unlink(field.parent, &field.segment);
                let parent = field.parent;
                removed.push(field);
                match parent {
                    Parent::Field(next) => id = next,
                    Parent::Root(_) => break,
                }
            }
            removed
        };
        drop(removed);
    }
}
pub(super) fn keyed_field<T: 'static, U: 'static>(
    source: Cage<Arena<T>>,
    key: u64,
    name: impl Into<Cow<'static, str>>,
    get: impl for<'a> Fn(&'a T) -> &'a U + 'static,
    get_mut: impl for<'a> Fn(&'a mut T) -> &'a mut U + 'static,
) -> Result<Cage<U>, AccessError> {
    let (context, _guard) = entries::keyed_context(source, key)?;
    inline(context, name.into(), get, get_mut)
}
pub(super) fn update_member<T: 'static, R>(
    source: Cage<Arena<T>>,
    id: u64,
    edit: impl FnOnce(&mut T) -> R,
) -> Result<R, AccessError> {
    collections::update_member(source, id, edit)
}
pub(super) fn append_members<T: 'static, R>(
    source: Cage<Arena<T>>,
    edit: impl FnOnce(&mut Arena<T>) -> R,
) -> Result<R, AccessError> {
    collections::append_members(source, edit)
}
pub(super) fn remove_member<T: 'static>(
    source: Cage<Arena<T>>,
    id: u64,
) -> Result<Option<T>, AccessError> {
    collections::remove_member(source, id)
}

struct Context {
    state: Shared,
    runtime: RuntimeId,
    source: Key,
    owner: Key,
    parent: Parent,
    depth: usize,
    path: ValuePath,
}
impl Context {
    fn new(runtime: RuntimeId, key: Key) -> Result<Self, AccessError> {
        allow_creation()?;
        let state = lookup(runtime)?;
        let (path, source_owner, parent, depth) = {
            let graph = state.borrow();
            let source = graph.nodes.get(key).ok_or(AccessError::Disposed)?;
            let field = graph.fields.aliases.get(&key).copied();
            let depth = field
                .and_then(|id| graph.fields.slots.get(id.0))
                .map_or(1, |f| f.depth + 1);
            (
                graph.value_path(key, source)?,
                source.owner,
                field.map_or(Parent::Root(key), Parent::Field),
                depth,
            )
        };
        if depth > 128 {
            return Err(AccessError::InvalidField("field nesting exceeds 128"));
        }
        // A write snapshots its existing aliases before invoking business code.
        // Do not add new paths while that root is mutably borrowed.
        drop(path.value.try_borrow().map_err(|_| AccessError::Borrowed)?);
        let owner = CONSTRUCTING.with_borrow(|stack| {
            stack
                .last()
                .filter(|(r, _)| *r == runtime)
                .map(|(_, owner)| *owner)
                .unwrap_or(source_owner)
        });
        Ok(Self {
            state,
            runtime,
            source: key,
            owner,
            parent,
            depth,
            path,
        })
    }
    fn register<U: 'static>(
        self,
        segment: Segment,
        layout: Option<Layout>,
        projection: impl FnOnce() -> Rc<dyn Projection>,
        entry: Option<(Rc<EntryKey>, Rc<EntryIndex>)>,
    ) -> Result<Cage<U>, AccessError> {
        let id = self.canonical::<U>(segment, layout, projection, entry)?;
        self.alias(id)
    }
    fn canonical<U: 'static>(
        &self,
        segment: Segment,
        layout: Option<Layout>,
        projection: impl FnOnce() -> Rc<dyn Projection>,
        entry: Option<(Rc<EntryKey>, Rc<EntryIndex>)>,
    ) -> Result<FieldId, AccessError> {
        let mut graph = self.state.borrow_mut();
        let (owner, parent) = (self.owner, self.parent);
        if graph.owners.get(owner).is_none() || graph.nodes.get(self.source).is_none() {
            return Err(AccessError::Disposed);
        }
        if let Some((_, index)) = &entry
            && graph
                .fields
                .indices
                .get(&parent)
                .is_some_and(|existing| !Rc::ptr_eq(existing, index))
        {
            return Err(AccessError::InvalidField(
                "collection path changed during key comparison",
            ));
        }
        let existing = graph
            .fields
            .children(parent)
            .and_then(|children| children.get(&segment))
            .copied();
        let id = if let Some(id) = existing {
            let field = graph.fields.slots.get(id.0).ok_or(AccessError::Disposed)?;
            if field.output != TypeId::of::<U>() {
                return Err(AccessError::TypeMismatch);
            }
            if field.layout != layout {
                return Err(AccessError::InvalidField("same name selects another field"));
            }
            id
        } else {
            if let Some(siblings) = graph.fields.children(parent) {
                // Accepted siblings are uniformly inline or collection paths.
                // Dynamic keys/positions are disjoint by identity; registering
                // one must not scan even the other live collection keys.
                if let Some(first) = siblings.values().next() {
                    let first = graph
                        .fields
                        .slots
                        .get(first.0)
                        .ok_or(AccessError::Disposed)?;
                    if first.layout.is_some() != layout.is_some() {
                        return Err(AccessError::InvalidField(
                            "inline and collection siblings overlap; use a common parent",
                        ));
                    }
                }
                if let Some(b) = layout {
                    for id in siblings.values() {
                        let a = graph
                            .fields
                            .slots
                            .get(id.0)
                            .ok_or(AccessError::Disposed)?
                            .layout
                            .expect("inline siblings");
                        if a.size != 0
                            && b.size != 0
                            && b.offset < a.offset + a.size
                            && a.offset < b.offset + b.size
                        {
                            return Err(AccessError::InvalidField("overlapping sibling fields"));
                        }
                    }
                }
            }
            let id = FieldId(graph.fields.slots.insert(FieldData {
                parent,
                root: self.path.root,
                segment: segment.clone(),
                output: TypeId::of::<U>(),
                layout,
                depth: self.depth,
                // Private factories only allocate/capture validated accessors;
                // they never invoke user getters, hashers or destructors here.
                projection: projection(),
                children: SparseMap::default(),
                aliases: SparseMap::default(),
                _entry: entry.as_ref().map(|(key, _)| key.clone()),
            }));
            if let Some((_, index)) = &entry {
                graph
                    .fields
                    .indices
                    .entry(parent)
                    .or_insert_with(|| index.clone());
            }
            match parent {
                Parent::Root(root) => {
                    graph
                        .fields
                        .roots
                        .entry(root)
                        .or_default()
                        .insert(segment, id);
                }
                Parent::Field(parent) => {
                    graph
                        .fields
                        .slots
                        .get_mut(parent.0)
                        .expect("live parent")
                        .children
                        .insert(segment, id);
                }
            }
            id
        };
        Ok(id)
    }
    fn alias<U: 'static>(&self, id: FieldId) -> Result<Cage<U>, AccessError> {
        let mut graph = self.state.borrow_mut();
        let owner = self.owner;
        if graph.owners.get(owner).is_none() || graph.nodes.get(self.source).is_none() {
            return Err(AccessError::Disposed);
        }
        if let Some(key) = graph
            .fields
            .slots
            .get(id.0)
            .ok_or(AccessError::Disposed)?
            .aliases
            .get(&owner)
        {
            return Ok(Cage {
                handle: Handle::new(self.runtime, *key),
            });
        }
        let node = Node {
            owner,
            compute: Compute::Source,
            value: None,
            version: 1,
            status: Status::Clean,
            rerun: false,
            propagated: false,
            queued: false,
            last_flush: 0,
            flush_runs: 0,
            dependencies: Dependencies::new(),
            subscribers: BTreeSet::new(),
        };
        let key = graph.insert_node(owner, node)?;
        graph.fields.aliases.insert(key, id);
        graph
            .fields
            .slots
            .get_mut(id.0)
            .expect("live field")
            .aliases
            .insert(owner, key);
        graph.fields.invalidate_target(self.path.root);
        drop(graph);
        Ok(Cage {
            handle: Handle::new(self.runtime, key),
        })
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    #[test]
    fn collection_targets_precheck_selected_versions_and_ignore_unchanged_positions() {
        let runtime = Runtime::new();
        let owner = runtime.owner();
        let rows = owner.cage(vec![0usize]);
        let first = rows.item(0);
        let last = rows.item(1);
        runtime
            .state
            .borrow_mut()
            .nodes
            .get_mut(first.handle.key)
            .unwrap()
            .version = u64::MAX;
        rows.push(1); // The exhausted, unchanged position must not be visited.
        assert_eq!(rows.with(Vec::len), 2);
        runtime
            .state
            .borrow_mut()
            .nodes
            .get_mut(last.handle.key)
            .unwrap()
            .version = u64::MAX;
        let before = runtime.stats();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rows.pop())).is_err());
        assert_eq!(rows.with(Vec::len), 2);
        assert_eq!(runtime.stats(), before);
        runtime
            .state
            .borrow_mut()
            .nodes
            .get_mut(last.handle.key)
            .unwrap()
            .version = 1;
        rows.pop();
        runtime
            .state
            .borrow_mut()
            .nodes
            .get_mut(rows.handle.key)
            .unwrap()
            .version = u64::MAX;
        let before = runtime.stats();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rows.push(2))).is_err());
        assert_eq!(rows.with(Vec::len), 1);
        assert_eq!(runtime.stats(), before);
    }
    #[test]
    fn cached_targets_still_precheck_every_source_version() {
        let runtime = Runtime::new();
        let owner = runtime.owner();
        let source = owner.cage((0usize,));
        let field = source.field("value", |m| &m.0, |m| &mut m.0);
        field.revise(|v| *v = 1);
        runtime
            .state
            .borrow_mut()
            .nodes
            .get_mut(source.handle.key)
            .unwrap()
            .version = u64::MAX;
        let before = runtime.stats();
        let called = Cell::new(false);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| field.revise(|v| {
                called.set(true);
                *v = 2;
            })))
            .is_err()
        );
        assert!(!called.get());
        assert_eq!(field.get(), 1);
        assert_eq!(runtime.stats(), before);
        runtime
            .state
            .borrow_mut()
            .nodes
            .get_mut(source.handle.key)
            .unwrap()
            .version = 1;
        field.revise(|v| *v = 2);
        assert_eq!(
            runtime.stats().field_target_hits,
            before.field_target_hits + 1
        );
    }
    #[test]
    fn borrowed_buffers_return_only_while_their_root_slot_survives() {
        let runtime = Runtime::new();
        let owner = runtime.owner();
        let source = owner.cage((0usize,));
        let field = source.field("value", |m| &m.0, |m| &mut m.0);
        let row = runtime.owner();
        let second = row.enter(|| source.field("value", |m| &m.0, |m| &mut m.0));
        let other = owner.cage((0usize,));
        second.revise(|v| {
            assert!(
                runtime.state.borrow().fields.targets[&source.handle.key]
                    .buffer
                    .is_none()
            );
            other
                .field("other", |m| &m.0, |m| &mut m.0)
                .revise(|n| *n = 1);
            assert!(
                runtime.state.borrow().fields.targets[&source.handle.key]
                    .buffer
                    .is_none()
            );
            *v = 1;
        });
        assert_eq!(runtime.stats().field_target_caches, 2);
        second.revise(|v| {
            drop(row);
            *v = 2;
        });
        assert_eq!(field.get(), 2);
        assert!(
            !runtime
                .state
                .borrow()
                .fields
                .targets
                .contains_key(&source.handle.key)
        );
        assert_eq!(runtime.stats().field_target_caches, 1);
        field.revise(|v| *v = 3);
        assert_eq!(runtime.stats().field_target_caches, 2);
    }
    #[test]
    fn alternating_selections_reuse_buffers_with_space_bounded_by_live_schema() {
        let runtime = Runtime::new();
        let owner = runtime.owner();
        let source = owner.cage([0usize; 64]);
        let mut owners: Vec<_> = (0..64).map(|_| Some(runtime.owner())).collect();
        let fields: Vec<_> = (0..64)
            .map(|i| {
                owners[i]
                    .as_ref()
                    .unwrap()
                    .enter(|| source.field(format!("{i}"), move |m| &m[i], move |m| &mut m[i]))
            })
            .collect();
        source.revise(|m| m.fill(0));
        let buffers = || {
            let state = runtime.state.borrow();
            let buffer = state.fields.targets[&source.handle.key]
                .buffer
                .as_ref()
                .unwrap();
            assert!(buffer.pending.is_empty());
            assert!(buffer.nodes.capacity() <= 2 * (state.fields.aliases.len() + 1));
            assert!(buffer.pending.capacity() <= 2 * state.fields.len());
            (buffer.nodes.as_ptr(), buffer.pending.as_ptr())
        };
        let initial = buffers();
        for i in 0..1000 {
            fields[i % 64].revise(|v| *v += 1);
            assert_eq!(buffers(), initial);
            assert_eq!(
                (
                    runtime.stats().field_target_caches,
                    runtime.stats().field_target_keys
                ),
                (1, 2)
            );
        }
        assert_eq!(source.with(|m| m.iter().sum::<usize>()), 1000);
        drop(owners[0].take());
        assert_eq!(
            (
                runtime.stats().field_target_caches,
                runtime.stats().field_target_keys
            ),
            (0, 0)
        );
    }
}
