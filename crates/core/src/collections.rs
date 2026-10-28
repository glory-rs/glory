//! Explicit structural edits select affected logical paths before modifying the
//! collection. Arbitrary `revise` remains conservative. No source/history scan.
use super::*;
use std::collections::{BTreeMap, HashMap};
use std::hash::{BuildHasher, Hash};

enum Plan<P, R> {
    Change(Selection, P),
    Noop(R),
}
enum Publish {
    Plain { _guard: crate::runtime::Write },
    Selected { _guard: Write },
}
fn mutate<C: 'static, P, R>(
    cage: Cage<C>,
    prepare: impl FnOnce(&C, Option<&EntryIndex>) -> Result<Plan<P, R>, AccessError>,
    edit: impl FnOnce(&mut C, P) -> R,
) -> Result<R, AccessError> {
    if EVALUATING.with_borrow(|stack| stack.last().is_some_and(|mode| *mode == Mode::Pure)) {
        return Err(AccessError::WriteDuringCompute);
    }
    let state = lookup(cage.handle.runtime)?;
    let (path, parent, index) = {
        let graph = state.borrow();
        let node = graph
            .nodes
            .get(cage.handle.key)
            .ok_or(AccessError::Disposed)?;
        if !matches!(node.compute, Compute::Source) {
            return Err(AccessError::TypeMismatch);
        }
        let parent = graph
            .fields
            .aliases
            .get(&cage.handle.key)
            .copied()
            .map_or(Parent::Root(cage.handle.key), Parent::Field);
        (
            graph.value_path(cage.handle.key, node)?,
            parent,
            graph.fields.indices.get(&parent).cloned(),
        )
    };
    let mut borrowed = path
        .value
        .try_borrow_mut()
        .map_err(|_| AccessError::Borrowed)?;
    let value = path
        .write(borrowed.as_mut())?
        .downcast_mut::<C>()
        .ok_or(AccessError::TypeMismatch)?;
    // User comparisons/hashing run here, outside graph/index borrows. They may
    // retire Owners, so revalidate the source before taking a publication guard.
    let (selection, plan) = match prepare(value, index.as_deref())? {
        Plan::Noop(result) => {
            if state.borrow().nodes.get(cage.handle.key).is_none() {
                return Err(AccessError::Disposed);
            }
            return Ok(result);
        }
        Plan::Change(selection, plan) => (selection, plan),
    };
    let publish = {
        let mut graph = state.borrow_mut();
        let source = graph
            .nodes
            .get(cage.handle.key)
            .ok_or(AccessError::Disposed)?;
        source
            .version
            .checked_add(1)
            .expect("source version exhausted");
        if matches!(parent, Parent::Root(_)) && !graph.fields.has_root(path.root) {
            Publish::Plain {
                _guard: crate::runtime::Write {
                    state: state.clone(),
                    key: path.root,
                },
            }
        } else {
            let target = graph.write_collection_target(path.root, parent, selection)?;
            Publish::Selected {
                _guard: Write {
                    state: state.clone(),
                    target: Some(target),
                },
            }
        }
    };
    let result = edit(value, plan);
    drop(borrowed);
    drop(publish);
    Ok(result)
}
fn selected<K: Eq + Hash + 'static>(
    index: Option<&EntryIndex>,
    key: &K,
) -> Result<Selection, AccessError> {
    Ok(Selection::Entry(
        index.map(|index| index.find(key)).transpose()?.flatten(),
    ))
}
pub(super) fn update_member<T: 'static, R>(
    source: Cage<Arena<T>>,
    id: u64,
    edit: impl FnOnce(&mut T) -> R,
) -> Result<R, AccessError> {
    let key = Key::unpack(id);
    mutate(
        source,
        |arena, _| {
            if arena.get(key).is_none() {
                return Err(AccessError::MissingField);
            }
            Ok(Plan::Change(Selection::Entry(Some(id)), ()))
        },
        |arena, ()| edit(arena.get_mut(key).expect("validated member")),
    )
}
pub(super) fn append_members<T: 'static, R>(
    source: Cage<Arena<T>>,
    edit: impl FnOnce(&mut Arena<T>) -> R,
) -> Result<R, AccessError> {
    mutate(
        source,
        |_, _| Ok(Plan::Change(Selection::Entry(None), ())),
        |arena, ()| edit(arena),
    )
}
pub(super) fn remove_member<T: 'static>(
    source: Cage<Arena<T>>,
    id: u64,
) -> Result<Option<T>, AccessError> {
    mutate(
        source,
        |arena, _| {
            Ok(if arena.get(Key::unpack(id)).is_none() {
                Plan::Noop(None)
            } else {
                Plan::Change(Selection::Entry(Some(id)), ())
            })
        },
        |arena, ()| arena.remove(Key::unpack(id)),
    )
}
fn items(start: usize, end: usize) -> Selection {
    Selection::Items { start, end }
}

impl<T: 'static> Cage<Vec<T>> {
    /// Append without notifying old positions. No element/collection Clone.
    /// `try_` reports reactive access/index/length errors; allocation and user
    /// destructor panics have the same behavior as ordinary Vec operations.
    ///
    /// ```
    /// let runtime = glory_core::Runtime::new();
    /// let owner = runtime.owner();
    /// let rows = owner.cage(Vec::<usize>::new());
    /// let first = rows.item(0);
    /// rows.push(7);
    /// rows.insert(0, 9);
    /// assert_eq!(first.get(), 9);
    /// assert_eq!(rows.remove(0), 9);
    /// assert_eq!(first.get(), 7);
    /// ```
    pub fn push(self, value: T) {
        self.try_push(value).expect("append to a live Vec Cage");
    }
    pub fn try_push(self, value: T) -> Result<(), AccessError> {
        mutate(
            self,
            |v, _| {
                Ok(Plan::Change(
                    items(
                        v.len(),
                        v.len().checked_add(1).ok_or(AccessError::Capacity)?,
                    ),
                    (),
                ))
            },
            |v, ()| v.push(value),
        )
    }
    pub fn pop(self) -> Option<T> {
        self.try_pop().expect("pop a live Vec Cage")
    }
    pub fn try_pop(self) -> Result<Option<T>, AccessError> {
        mutate(
            self,
            |v, _| {
                Ok(if v.is_empty() {
                    Plan::Noop(None)
                } else {
                    Plan::Change(items(v.len() - 1, v.len()), ())
                })
            },
            |v, ()| v.pop(),
        )
    }
    /// Insert at a position; only registered positions in [index, old_len]
    /// change. Positions are not stable entity identities; use map keys for that.
    pub fn insert(self, index: usize, value: T) {
        self.try_insert(index, value)
            .expect("insert within a live Vec Cage");
    }
    pub fn try_insert(self, index: usize, value: T) -> Result<(), AccessError> {
        mutate(
            self,
            |v, _| {
                if index > v.len() {
                    return Err(AccessError::OutOfBounds);
                }
                Ok(Plan::Change(
                    items(index, v.len().checked_add(1).ok_or(AccessError::Capacity)?),
                    (),
                ))
            },
            |v, ()| v.insert(index, value),
        )
    }
    pub fn remove(self, index: usize) -> T {
        self.try_remove(index)
            .expect("remove within a live Vec Cage")
    }
    pub fn try_remove(self, index: usize) -> Result<T, AccessError> {
        mutate(
            self,
            |v, _| {
                if index >= v.len() {
                    return Err(AccessError::OutOfBounds);
                }
                Ok(Plan::Change(items(index, v.len()), ()))
            },
            |v, ()| v.remove(index),
        )
    }
    pub fn truncate(self, len: usize) {
        self.try_truncate(len).expect("truncate a live Vec Cage");
    }
    pub fn try_truncate(self, len: usize) -> Result<(), AccessError> {
        mutate(
            self,
            |v, _| {
                Ok(if len >= v.len() {
                    Plan::Noop(())
                } else {
                    Plan::Change(items(len, v.len()), ())
                })
            },
            |v, ()| v.truncate(len),
        )
    }
    pub fn clear(self) {
        self.truncate(0);
    }
    pub fn try_clear(self) -> Result<(), AccessError> {
        self.try_truncate(0)
    }
}

// Keep identical edit/notification semantics for both standard map families.
// These are Rust API code generation, not a UI or expression DSL.
macro_rules! map_edits {
    ($map:ty, [$($generic:tt)*]) => {
        impl<$($generic)*> Cage<$map> {
            /// Edit one present key without constructing a persistent entry alias.
            /// Only that key's registered fields and map/ancestor readers publish.
            pub fn try_update<R>(self, key: &K, edit: impl FnOnce(&mut V) -> R) -> Result<R, AccessError> {
                mutate(self, |map, index| {
                    if !project(|| Ok(map.contains_key(key)))? { return Err(AccessError::MissingField); }
                    Ok(Plan::Change(selected(index,key)?, ()))
                }, |map, ()| edit(project(||Ok(map.get_mut(key))).expect("pure map lookup").expect("validated present key")))
            }
            /// Insert/replace one key. Publishes the map/ancestors and that key's
            /// live aliases/children; unrelated keys are not invalidated. Keys
            /// and values need not be Clone. Hash/Eq/Ord/hasher must be pure.
            ///
            /// ```
            /// let runtime = glory_core::Runtime::new();
            /// let owner = runtime.owner();
            /// let rows = owner.cage(std::collections::HashMap::<u64, String>::new());
            /// let title = rows.entry(7);
            /// assert_eq!(title.try_get(), Err(glory_core::AccessError::MissingField));
            /// rows.insert(7, String::from("ready"));
            /// assert_eq!(title.get(), "ready");
            /// assert_eq!(rows.remove(&7).as_deref(), Some("ready"));
            /// ```
            pub fn insert(self, key: K, value: V) -> Option<V> {
                self.try_insert(key,value).expect("insert into a live map Cage")
            }
            pub fn try_insert(self, key: K, value: V) -> Result<Option<V>, AccessError> {
                // Retain a duplicate incoming key until the mutable model borrow
                // and publication have ended, so its destructor can use runtime.
                let (old,duplicate)=mutate(self, |_, index| Ok(Plan::Change(selected(index,&key)?, key)), |map, key| {
                    if let Some(old)=project(|| Ok(map.get_mut(&key))).expect("pure map lookup") {
                        (Some(std::mem::replace(old,value)),Some(key))
                    } else {
                        (project(|| Ok(map.insert(key,value))).expect("pure map insertion"),None)
                    }
                })?;
                drop(duplicate);
                Ok(old)
            }
            /// Missing-key removal is a no-op (no versions, work or wake).
            pub fn remove(self, key: &K) -> Option<V> {
                self.try_remove(key).expect("remove from a live map Cage")
            }
            pub fn try_remove(self, key: &K) -> Result<Option<V>, AccessError> {
                let removed=mutate(self, |map, index| {
                    if !project(|| Ok(map.contains_key(key)))? { return Ok(Plan::Noop(None)); }
                    Ok(Plan::Change(selected(index,key)?, ()))
                }, |map, ()| project(|| Ok(map.remove_entry(key))).expect("pure map removal"))?;
                // A removed stored key drops outside graph, registry and model
                // borrows, rather than in the middle of std::Map::remove.
                Ok(removed.map(|(_,value)| value))
            }
        }
    };
}
map_edits!(BTreeMap<K,V>, [K: Ord + Hash + 'static, V: 'static]);
map_edits!(HashMap<K,V,S>, [K: Eq + Hash + 'static, V: 'static, S: BuildHasher + 'static]);
