//! Collection selection never retains a reference or address into the data.
use super::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{BuildHasher, Hash, Hasher};

#[derive(Default)]
pub(super) struct EntryIndex {
    next: Cell<u64>,
    buckets: RefCell<BTreeMap<u64, Vec<(u64, Weak<EntryKey>)>>>,
}
pub(super) struct EntryKey {
    id: u64,
    hash: u64,
    value: Rc<dyn Any>,
    index: Weak<EntryIndex>,
}
impl Drop for EntryKey {
    fn drop(&mut self) {
        if let Some(index) = self.index.upgrade() {
            let mut buckets = index.buckets.borrow_mut();
            if let Some(bucket) = buckets.get_mut(&self.hash) {
                bucket.retain(|(id, _)| *id != self.id);
                if bucket.is_empty() {
                    buckets.remove(&self.hash);
                }
            }
        }
        // The user key drops after the numeric registry borrow has ended.
    }
}
impl EntryIndex {
    pub fn stats(&self) -> (usize, usize) {
        let buckets = self.buckets.borrow();
        (buckets.values().map(Vec::len).sum(), buckets.len())
    }
    fn candidates(&self, hash: u64) -> (Option<Rc<EntryKey>>, Vec<Rc<EntryKey>>) {
        let buckets = self.buckets.borrow();
        let Some(bucket) = buckets.get(&hash) else {
            return (None, Vec::new());
        };
        (
            bucket.first().and_then(|(_, key)| key.upgrade()),
            bucket
                .iter()
                .skip(1)
                .filter_map(|(_, key)| key.upgrade())
                .collect(),
        )
    }
    /// Only current live paths are searched. Snapshot weak candidates before
    /// invoking user Hash/Eq so disposal/destructors never run under the index.
    pub(super) fn find<K: Eq + Hash + 'static>(&self, key: &K) -> Result<Option<u64>, AccessError> {
        let hash = project(|| {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            Ok(hasher.finish())
        })?;
        let (first, rest) = self.candidates(hash);
        for candidate in first.into_iter().chain(rest) {
            if project(|| {
                Ok(candidate
                    .value
                    .downcast_ref::<K>()
                    .is_some_and(|old| old == key))
            })? {
                return Ok(Some(candidate.id));
            }
        }
        Ok(None)
    }
    fn intern<K: Eq + Hash + 'static>(
        self: &Rc<Self>,
        key: K,
    ) -> Result<Rc<EntryKey>, AccessError> {
        let hash = project(|| {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            Ok(hasher.finish())
        })?;
        let (first, rest) = self.candidates(hash);
        // User Hash/Eq run outside BOTH graph and registry borrows. Hash
        // collisions compare full typed keys, never alias unrelated entries.
        for candidate in first.into_iter().chain(rest) {
            if project(|| {
                Ok(candidate
                    .value
                    .downcast_ref::<K>()
                    .is_some_and(|old| old == &key))
            })? {
                return Ok(candidate);
            }
        }
        let id = self.next.get();
        self.next.set(
            id.checked_add(1)
                .expect("collection path identity exhausted"),
        );
        let key = Rc::new(EntryKey {
            id,
            hash,
            value: Rc::new(key),
            index: Rc::downgrade(self),
        });
        self.buckets
            .borrow_mut()
            .entry(hash)
            .or_default()
            .push((id, Rc::downgrade(&key)));
        Ok(key)
    }
}

trait Collection: 'static {
    type Key: 'static;
    type Value: 'static;
    fn read(&self, key: &Self::Key) -> Option<&Self::Value>;
    fn write(&mut self, key: &Self::Key) -> Option<&mut Self::Value>;
}
impl<T: 'static> Collection for Vec<T> {
    type Key = usize;
    type Value = T;
    fn read(&self, key: &usize) -> Option<&T> {
        self.get(*key)
    }
    fn write(&mut self, key: &usize) -> Option<&mut T> {
        self.get_mut(*key)
    }
}
impl<K: Ord + 'static, V: 'static> Collection for BTreeMap<K, V> {
    type Key = K;
    type Value = V;
    fn read(&self, key: &K) -> Option<&V> {
        self.get(key)
    }
    fn write(&mut self, key: &K) -> Option<&mut V> {
        self.get_mut(key)
    }
}
impl<K: Eq + Hash + 'static, V: 'static, S: BuildHasher + 'static> Collection for HashMap<K, V, S> {
    type Key = K;
    type Value = V;
    fn read(&self, key: &K) -> Option<&V> {
        self.get(key)
    }
    fn write(&mut self, key: &K) -> Option<&mut V> {
        self.get_mut(key)
    }
}
struct Select<C: Collection> {
    parent: Option<Rc<dyn Projection>>,
    key: Rc<dyn Any>,
    types: PhantomData<fn() -> C>,
}
impl<C: Collection> Projection for Select<C> {
    fn read<'a>(&self, value: &'a dyn Any) -> Result<&'a dyn Any, AccessError> {
        let value = if let Some(parent) = &self.parent {
            parent.read(value)?
        } else {
            value
        };
        let value = value.downcast_ref::<C>().ok_or(AccessError::TypeMismatch)?;
        let key = self.key.downcast_ref().ok_or(AccessError::TypeMismatch)?;
        value
            .read(key)
            .map(|v| v as &dyn Any)
            .ok_or(AccessError::MissingField)
    }
    fn write<'a>(&self, value: &'a mut dyn Any) -> Result<&'a mut dyn Any, AccessError> {
        let value = if let Some(parent) = &self.parent {
            parent.write(value)?
        } else {
            value
        };
        let value = value.downcast_mut::<C>().ok_or(AccessError::TypeMismatch)?;
        let key = self.key.downcast_ref().ok_or(AccessError::TypeMismatch)?;
        value
            .write(key)
            .map(|v| v as &mut dyn Any)
            .ok_or(AccessError::MissingField)
    }
}
fn keyed<C: Collection>(
    runtime: RuntimeId,
    source: Key,
    key: C::Key,
) -> Result<Cage<C::Value>, AccessError>
where
    C::Key: Eq + Hash,
{
    let context = Context::new(runtime, source)?;
    let index = context
        .state
        .borrow()
        .fields
        .indices
        .get(&context.parent)
        .cloned()
        .unwrap_or_default();
    let key = index.intern(key)?;
    let parent = context.path.projection.clone();
    let value = key.value.clone();
    context.register(
        Segment::Entry(key.id),
        None,
        move || {
            Rc::new(Select::<C> {
                parent,
                key: value,
                types: PhantomData,
            }) as Rc<dyn Projection>
        },
        Some((key, index)),
    )
}

impl<T: 'static> Cage<Vec<T>> {
    /// Track a Vec position, including a currently missing one, without copying
    /// its element. Insert/remove/reorder retarget this position. For stable
    /// business identity, keep order separately and use a keyed map's `entry`.
    /// `try_with`/`try_revise` return MissingField while the position is absent.
    pub fn item(self, index: usize) -> Cage<T> {
        self.try_item(index)
            .expect("create a collection position on a live Cage")
    }
    pub fn try_item(self, index: usize) -> Result<Cage<T>, AccessError> {
        let context = Context::new(self.handle.runtime, self.handle.key)?;
        let parent = context.path.projection.clone();
        context.register(
            Segment::Item(index),
            None,
            move || {
                Rc::new(Select::<Vec<T>> {
                    parent,
                    key: Rc::new(index),
                    types: PhantomData,
                }) as Rc<dyn Projection>
            },
            None,
        )
    }
}
impl<K: Ord + Hash + 'static, V: 'static> Cage<BTreeMap<K, V>> {
    /// Track a map key without cloning the map or its value. The same path
    /// observes MissingField after removal and the replacement after reinsert;
    /// use a generation in business keys when replacement means a new entity.
    /// Keys need not be Clone; Hash/Eq/Ord must be pure. Arbitrary `revise`
    /// notifies all live entries; `insert`/`remove` and entry writes leave
    /// unrelated keys alone.
    ///
    /// ```
    /// use std::collections::BTreeMap;
    /// let runtime = glory_core::Runtime::new();
    /// let owner = runtime.owner();
    /// let rows = owner.cage(BTreeMap::from([(7, String::from("row"))]));
    /// let row = rows.entry(7);
    /// row.revise(|value| value.push('!'));
    /// assert_eq!(row.get(), "row!");
    /// rows.revise(|rows| { rows.remove(&7); });
    /// assert_eq!(row.try_get(), Err(glory_core::AccessError::MissingField));
    /// ```
    pub fn entry(self, key: K) -> Cage<V> {
        self.try_entry(key)
            .expect("create a map key on a live Cage")
    }
    pub fn try_entry(self, key: K) -> Result<Cage<V>, AccessError> {
        keyed::<BTreeMap<K, V>>(self.handle.runtime, self.handle.key, key)
    }
}
impl<K: Eq + Hash + 'static, V: 'static, S: BuildHasher + 'static> Cage<HashMap<K, V, S>> {
    /// HashMap counterpart of BTreeMap's `entry`; preserves the map's hasher.
    /// Key hashing and comparison must be pure; no value Clone bound is needed.
    pub fn entry(self, key: K) -> Cage<V> {
        self.try_entry(key)
            .expect("create a map key on a live Cage")
    }
    pub fn try_entry(self, key: K) -> Result<Cage<V>, AccessError> {
        keyed::<HashMap<K, V, S>>(self.handle.runtime, self.handle.key, key)
    }
}
