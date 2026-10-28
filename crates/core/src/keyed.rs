//! Ordered entity storage. Values are stored once; order has its own channel.
use super::*;
use std::borrow::Cow;
use std::collections::HashSet;
use std::hash::Hash;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListError {
    Access(AccessError),
    DuplicateKey,
}
impl From<AccessError> for ListError {
    fn from(value: AccessError) -> Self {
        Self::Access(value)
    }
}
impl std::fmt::Display for ListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ListError {}

/// A membership identity, unchanged by movement or in-place value replacement.
/// Slots may be reused with a new generation; a complete ID is never reused.
#[derive(Clone, Debug)]
pub struct RowToken<K> {
    key: K,
    id: u64,
    source: (RuntimeId, Key),
}
impl<K> RowToken<K> {
    pub fn key(&self) -> &K {
        &self.key
    }
    pub fn id(&self) -> u64 {
        self.id
    }
}
type KeyFn<K, T> = Rc<dyn Fn(&T) -> K>;
struct Meta<K, T> {
    keys: HashMap<K, u64>,
    key: KeyFn<K, T>,
}

/// Owner-owned, thread-local collection of authoritative values and stable keys.
/// Keys must be pure and fixed for a member's lifetime. Editing its key field
/// does not rename a member; use remove/insert to establish a new identity.
pub struct KeyedList<K: 'static, T: 'static> {
    meta: Cage<Meta<K, T>>,
    order: Cage<Vec<RowToken<K>>>,
    values: Cage<Arena<T>>,
}
impl<K, T> Copy for KeyedList<K, T> {}
impl<K, T> Clone for KeyedList<K, T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// A non-owning entity handle; field aliases belong to their constructing Owner.
pub struct KeyedEntry<K: 'static, T: 'static> {
    token: RowToken<K>,
    values: Cage<Arena<T>>,
}
impl<K: Clone, T> Clone for KeyedEntry<K, T> {
    fn clone(&self) -> Self {
        Self {
            token: self.token.clone(),
            values: self.values,
        }
    }
}
impl<K, T> KeyedEntry<K, T> {
    pub fn key(&self) -> &K {
        &self.token.key
    }
    pub fn id(&self) -> u64 {
        self.token.id
    }
    pub fn try_with_untracked<R>(&self, read: impl FnOnce(&T) -> R) -> Result<R, AccessError> {
        self.values.try_with_untracked(|values| {
            values
                .get(Key::unpack(self.token.id))
                .map(read)
                .ok_or(AccessError::MissingField)
        })?
    }
    pub fn update<R>(&self, edit: impl FnOnce(&mut T) -> R) -> Result<R, AccessError> {
        fields::update_member(self.values, self.token.id, edit)
    }
    pub fn field<U: 'static>(
        &self,
        name: impl Into<Cow<'static, str>>,
        get: impl for<'a> Fn(&'a T) -> &'a U + 'static,
        get_mut: impl for<'a> Fn(&'a mut T) -> &'a mut U + 'static,
    ) -> Cage<U> {
        self.try_field(name, get, get_mut)
            .expect("project a field of a live keyed member")
    }
    pub fn try_field<U: 'static>(
        &self,
        name: impl Into<Cow<'static, str>>,
        get: impl for<'a> Fn(&'a T) -> &'a U + 'static,
        get_mut: impl for<'a> Fn(&'a mut T) -> &'a mut U + 'static,
    ) -> Result<Cage<U>, AccessError> {
        fields::keyed_field(self.values, self.token.id, name, get, get_mut)
    }
}

// Collection operations validate all roots before making related writes. Host
// wake never synchronously reenters; no observers run between these publications.
// Returned old values/keys are dropped after all model borrows and publications.
fn writable<T>(cage: Cage<T>) -> Result<(), AccessError> {
    if EVALUATING.with_borrow(|s| s.last() == Some(&Mode::Pure)) {
        return Err(AccessError::WriteDuringCompute);
    }
    let state = lookup(cage.handle.runtime)?;
    let value = state
        .borrow()
        .nodes
        .get(cage.handle.key)
        .ok_or(AccessError::Disposed)?
        .value
        .clone()
        .ok_or(AccessError::TypeMismatch)?;
    let _borrow = value.try_borrow_mut().map_err(|_| AccessError::Borrowed)?;
    Ok(())
}
impl Owner {
    pub fn keyed_list<K: Clone + Eq + Hash + 'static, T: 'static>(
        &self,
        initial: Vec<T>,
        key: impl Fn(&T) -> K + 'static,
    ) -> Result<KeyedList<K, T>, ListError> {
        let mut seen = HashSet::with_capacity(initial.len());
        for value in &initial {
            if !fields::project(|| Ok(seen.insert(key(value))))? {
                return Err(ListError::DuplicateKey);
            }
        }
        drop(seen);
        if !self.is_alive() {
            return Err(AccessError::Disposed.into());
        }
        let list = KeyedList {
            meta: self.cage(Meta {
                keys: HashMap::new(),
                key: Rc::new(key),
            }),
            order: self.cage(Vec::new()),
            values: self.cage(Arena::default()),
        };
        list.extend(initial)?;
        Ok(list)
    }
}
impl<K: Clone + Eq + Hash + 'static, T: 'static> KeyedList<K, T> {
    pub fn runtime(self) -> RuntimeId {
        self.order.runtime()
    }
    pub fn is_alive(self) -> bool {
        self.order.is_alive() && self.values.is_alive()
    }
    pub fn len_untracked(self) -> usize {
        self.order.with_untracked(Vec::len)
    }
    /// Allocated value slots follow peak concurrent membership, not visited keys.
    pub fn storage_slots_untracked(self) -> usize {
        self.values.with_untracked(Arena::capacity)
    }
    pub fn with_order<R>(self, read: impl FnOnce(&[RowToken<K>]) -> R) -> R {
        self.order.with(|v| read(v))
    }
    pub fn entry(self, token: &RowToken<K>) -> Result<KeyedEntry<K, T>, AccessError> {
        if token.source != (self.values.handle.runtime, self.values.handle.key) {
            return Err(AccessError::WrongRuntime);
        }
        Ok(KeyedEntry {
            token: token.clone(),
            values: self.values,
        })
    }
    pub fn at(self, index: usize) -> Result<KeyedEntry<K, T>, AccessError> {
        self.order.try_with_untracked(|order| {
            order
                .get(index)
                .map(|token| self.entry(token))
                .ok_or(AccessError::OutOfBounds)
        })??
    }
    pub fn update_at<R>(
        self,
        index: usize,
        edit: impl FnOnce(&mut T) -> R,
    ) -> Result<R, AccessError> {
        self.at(index)?.update(edit)
    }
    pub fn find(self, key: &K) -> Result<Option<KeyedEntry<K, T>>, AccessError> {
        self.meta.try_with_untracked(|m| {
            fields::project(|| {
                Ok(m.keys.get(key).map(|id| KeyedEntry {
                    token: RowToken {
                        key: key.clone(),
                        id: *id,
                        source: (self.values.handle.runtime, self.values.handle.key),
                    },
                    values: self.values,
                }))
            })
        })?
    }
    pub fn update_key<R>(self, key: &K, edit: impl FnOnce(&mut T) -> R) -> Result<R, AccessError> {
        self.find(key)?
            .ok_or(AccessError::MissingField)?
            .update(edit)
    }
    fn check(self) -> Result<(), AccessError> {
        writable(self.meta)?;
        writable(self.order)?;
        writable(self.values)
    }
    fn prepare(
        self,
        values: Vec<T>,
        replace: bool,
    ) -> Result<(Vec<(K, Option<u64>, T)>, KeyFn<K, T>), ListError> {
        let (key, old) = self
            .meta
            .try_with_untracked(|m| (m.key.clone(), m.keys.clone()))?;
        let mut seen = HashSet::with_capacity(values.len());
        let mut result = Vec::with_capacity(values.len());
        for value in values {
            let k = fields::project(|| Ok(key(&value)))?;
            if !fields::project(
                || Ok(seen.insert(k.clone()) && (replace || !old.contains_key(&k))),
            )? {
                return Err(ListError::DuplicateKey);
            }
            let previous = fields::project(|| Ok(old.get(&k).copied()))?;
            result.push((k, previous, value));
        }
        Ok((result, key))
    }
    pub fn replace(self, values: Vec<T>) -> Result<(), ListError> {
        let (incoming, key) = self.prepare(values, true)?;
        self.check()?;
        let retained: HashSet<_> = incoming.iter().filter_map(|(_, id, _)| *id).collect();
        let previous = self.order.try_get()?;
        let source = (self.values.handle.runtime, self.values.handle.key);
        let (order, keys, removed) = self.values.try_revise(|arena| {
            let mut removed = Vec::with_capacity(previous.len());
            for token in &previous {
                if !retained.contains(&token.id) {
                    removed.push(arena.remove(Key::unpack(token.id)).expect("indexed member"));
                }
            }
            let mut order = Vec::with_capacity(incoming.len());
            let mut keys = HashMap::with_capacity(incoming.len());
            for (k, previous, value) in incoming {
                let id = if let Some(id) = previous {
                    removed.push(std::mem::replace(
                        arena.get_mut(Key::unpack(id)).expect("retained member"),
                        value,
                    ));
                    id
                } else {
                    arena.insert(value).packed()
                };
                keys.insert(k.clone(), id);
                order.push(RowToken { key: k, id, source });
            }
            (order, keys, removed)
        })?;
        let changed =
            previous.len() != order.len() || previous.iter().zip(&order).any(|(a, b)| a.id != b.id);
        let old_meta = self
            .meta
            .try_revise(|m| std::mem::replace(m, Meta { key, keys }))?;
        let old_order = if changed {
            Some(self.order.try_revise(|v| std::mem::replace(v, order))?)
        } else {
            None
        };
        drop((previous, removed, old_meta, old_order));
        Ok(())
    }
    pub fn extend(self, values: Vec<T>) -> Result<(), ListError> {
        if values.is_empty() {
            self.check()?;
            return Ok(());
        }
        let (incoming, key) = self.prepare(values, false)?;
        self.check()?;
        let mut keys = self.meta.try_with_untracked(|m| m.keys.clone())?;
        let source = (self.values.handle.runtime, self.values.handle.key);
        let order = fields::append_members(self.values, |arena| {
            let mut order = Vec::with_capacity(incoming.len());
            for (k, _, value) in incoming {
                let id = arena.insert(value).packed();
                keys.insert(k.clone(), id);
                order.push(RowToken { key: k, id, source });
            }
            order
        })?;
        let old_meta = self
            .meta
            .try_revise(|m| std::mem::replace(m, Meta { key, keys }))?;
        self.order.try_revise(|v| v.extend(order))?;
        drop(old_meta);
        Ok(())
    }
    pub fn swap(self, a: usize, b: usize) -> Result<(), AccessError> {
        writable(self.order)?;
        let len = self.order.try_with_untracked(Vec::len)?;
        if a >= len || b >= len {
            return Err(AccessError::OutOfBounds);
        }
        if a != b {
            self.order.try_revise(|v| v.swap(a, b))?;
        }
        Ok(())
    }
    pub fn remove_key(self, key: &K) -> Result<Option<T>, AccessError> {
        self.check()?;
        let Some(id) = self.meta.try_with_untracked(|m| m.keys.get(key).copied())? else {
            return Ok(None);
        };
        let index = self
            .order
            .try_with_untracked(|v| v.iter().position(|t| t.id == id).expect("indexed member"))?;
        let old_key = self.meta.try_revise(|m| m.keys.remove_entry(key))?;
        let old_token = self.order.try_remove(index)?;
        let old_value = fields::remove_member(self.values, id)?;
        drop((old_key, old_token));
        Ok(old_value)
    }
    pub fn clear(self) -> Result<(), AccessError> {
        self.check()?;
        if self.order.try_with_untracked(Vec::is_empty)? {
            return Ok(());
        }
        let keys = self.meta.try_revise(|m| std::mem::take(&mut m.keys))?;
        let order = self.order.try_revise(std::mem::take)?;
        let values = self.values.try_revise(|arena| {
            order
                .iter()
                .map(|t| arena.remove(Key::unpack(t.id)).expect("indexed member"))
                .collect::<Vec<_>>()
        })?;
        drop((keys, order, values));
        Ok(())
    }
}
