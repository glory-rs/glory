use std::cell::{Ref, RefMut};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::hash::Hash;
use std::rc::Rc;

use super::{Bond, Cage};
use crate::reflow::cage::{CageAccessError, CageMutateError};

type Getter<Root, Value> = Rc<dyn for<'a> Fn(&'a Root) -> &'a Value>;
type GetterMut<Root, Value> = Rc<dyn for<'a> Fn(&'a mut Root) -> &'a mut Value>;

/// A read/write lens into a field or nested member of a root [`Cage`].
///
/// `CageLens` keeps one root reactive cell and projects borrows through
/// closures, so applications can update nested state without embedding
/// `Cage` values in every field.
pub struct CageLens<Root, Value>
where
    Root: fmt::Debug + 'static,
    Value: fmt::Debug + 'static,
{
    root: Cage<Root>,
    get: Getter<Root, Value>,
    get_mut: GetterMut<Root, Value>,
}

impl<Root, Value> Clone for CageLens<Root, Value>
where
    Root: fmt::Debug + 'static,
    Value: fmt::Debug + 'static,
{
    fn clone(&self) -> Self {
        Self {
            root: self.root,
            get: self.get.clone(),
            get_mut: self.get_mut.clone(),
        }
    }
}

impl<Root, Value> fmt::Debug for CageLens<Root, Value>
where
    Root: fmt::Debug + 'static,
    Value: fmt::Debug + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CageLens").field("root", &self.root).finish_non_exhaustive()
    }
}

impl<Root, Value> CageLens<Root, Value>
where
    Root: fmt::Debug + 'static,
    Value: fmt::Debug + 'static,
{
    pub fn new(
        root: Cage<Root>,
        get: impl for<'a> Fn(&'a Root) -> &'a Value + 'static,
        get_mut: impl for<'a> Fn(&'a mut Root) -> &'a mut Value + 'static,
    ) -> Self {
        Self {
            root,
            get: Rc::new(get),
            get_mut: Rc::new(get_mut),
        }
    }

    pub fn root(&self) -> Cage<Root> {
        self.root
    }

    pub fn get(&self) -> Ref<'_, Value> {
        Ref::map(self.root.get(), |root| (self.get)(root))
    }

    pub fn try_get(&self) -> Result<Ref<'_, Value>, CageAccessError> {
        self.root.try_get().map(|root| Ref::map(root, |root| (self.get)(root)))
    }

    pub fn get_untracked(&self) -> Ref<'_, Value> {
        Ref::map(self.root.get_untracked(), |root| (self.get)(root))
    }

    pub fn try_get_untracked(&self) -> Result<Ref<'_, Value>, CageAccessError> {
        self.root.try_get_untracked().map(|root| Ref::map(root, |root| (self.get)(root)))
    }

    pub fn revise<F, R>(&self, update: F) -> R
    where
        F: FnOnce(RefMut<'_, Value>) -> R,
    {
        self.root.revise(|root| update(RefMut::map(root, |root| (self.get_mut)(root))))
    }

    pub fn try_revise<F, R>(&self, update: F) -> Result<R, CageMutateError>
    where
        F: FnOnce(RefMut<'_, Value>) -> R,
    {
        self.root.try_revise(|root| update(RefMut::map(root, |root| (self.get_mut)(root))))
    }

    pub fn set(&self, value: Value) -> Value {
        self.revise(|mut current| std::mem::replace(&mut *current, value))
    }

    pub fn lens<Child>(
        &self,
        child_get: impl for<'a> Fn(&'a Value) -> &'a Child + 'static,
        child_get_mut: impl for<'a> Fn(&'a mut Value) -> &'a mut Child + 'static,
    ) -> CageLens<Root, Child>
    where
        Child: fmt::Debug + 'static,
    {
        let parent_get = self.get.clone();
        let parent_get_mut = self.get_mut.clone();
        CageLens::new(
            self.root,
            move |root| child_get(parent_get(root)),
            move |root| child_get_mut(parent_get_mut(root)),
        )
    }

    pub fn map<M, G>(&self, mapper: M) -> Bond<G>
    where
        M: Fn(Ref<'_, Value>) -> G + Clone + 'static,
        G: fmt::Debug + 'static,
    {
        let this = self.clone();
        Bond::new(move || mapper(this.get()))
    }
}

/// Root-level convenience for creating a [`CageLens`].
pub trait StoreExt<Root>
where
    Root: fmt::Debug + 'static,
{
    fn lens<Value>(
        &self,
        get: impl for<'a> Fn(&'a Root) -> &'a Value + 'static,
        get_mut: impl for<'a> Fn(&'a mut Root) -> &'a mut Value + 'static,
    ) -> CageLens<Root, Value>
    where
        Value: fmt::Debug + 'static;
}

impl<Root> StoreExt<Root> for Cage<Root>
where
    Root: fmt::Debug + 'static,
{
    fn lens<Value>(
        &self,
        get: impl for<'a> Fn(&'a Root) -> &'a Value + 'static,
        get_mut: impl for<'a> Fn(&'a mut Root) -> &'a mut Value + 'static,
    ) -> CageLens<Root, Value>
    where
        Value: fmt::Debug + 'static,
    {
        CageLens::new(*self, get, get_mut)
    }
}

/// Convenience collection operations for `Cage<Vec<T>>`.
///
/// These helpers keep the existing `Cage` API intact while making nested list
/// state updates less repetitive in TodoMVC-style apps.
pub trait VecStoreExt<T>
where
    T: fmt::Debug + 'static,
{
    fn push_item(&self, item: T);
    fn update_item_by(&self, predicate: impl FnMut(&T) -> bool, update: impl FnOnce(&mut T)) -> bool;
    fn remove_items_by(&self, predicate: impl FnMut(&T) -> bool) -> usize;
    fn clear_items(&self);
}

impl<T> VecStoreExt<T> for Cage<Vec<T>>
where
    T: fmt::Debug + 'static,
{
    fn push_item(&self, item: T) {
        self.revise(|mut items| items.push(item));
    }

    fn update_item_by(&self, mut predicate: impl FnMut(&T) -> bool, update: impl FnOnce(&mut T)) -> bool {
        self.revise(|mut items| {
            let Some(item) = items.iter_mut().find(|item| predicate(item)) else {
                return false;
            };
            update(item);
            true
        })
    }

    fn remove_items_by(&self, mut predicate: impl FnMut(&T) -> bool) -> usize {
        self.revise(|mut items| {
            let before = items.len();
            items.retain(|item| !predicate(item));
            before - items.len()
        })
    }

    fn clear_items(&self) {
        self.revise(|mut items| items.clear());
    }
}

impl<Root, T> VecStoreExt<T> for CageLens<Root, Vec<T>>
where
    Root: fmt::Debug + 'static,
    T: fmt::Debug + 'static,
{
    fn push_item(&self, item: T) {
        self.revise(|mut items| items.push(item));
    }

    fn update_item_by(&self, mut predicate: impl FnMut(&T) -> bool, update: impl FnOnce(&mut T)) -> bool {
        self.revise(|mut items| {
            let Some(item) = items.iter_mut().find(|item| predicate(item)) else {
                return false;
            };
            update(item);
            true
        })
    }

    fn remove_items_by(&self, mut predicate: impl FnMut(&T) -> bool) -> usize {
        self.revise(|mut items| {
            let before = items.len();
            items.retain(|item| !predicate(item));
            before - items.len()
        })
    }

    fn clear_items(&self) {
        self.revise(|mut items| items.clear());
    }
}

/// Convenience operations for `Cage<Option<T>>` and option lenses.
pub trait OptionStoreExt<T>
where
    T: fmt::Debug + 'static,
{
    fn set_some(&self, value: T);
    fn update_some(&self, update: impl FnOnce(&mut T)) -> bool;
    fn take_value(&self) -> Option<T>;
    fn clear_value(&self);
}

impl<T> OptionStoreExt<T> for Cage<Option<T>>
where
    T: fmt::Debug + 'static,
{
    fn set_some(&self, value: T) {
        self.revise(|mut current| *current = Some(value));
    }

    fn update_some(&self, update: impl FnOnce(&mut T)) -> bool {
        self.revise(|mut current| {
            let Some(value) = current.as_mut() else {
                return false;
            };
            update(value);
            true
        })
    }

    fn take_value(&self) -> Option<T> {
        self.revise(|mut current| current.take())
    }

    fn clear_value(&self) {
        self.revise(|mut current| *current = None);
    }
}

impl<Root, T> OptionStoreExt<T> for CageLens<Root, Option<T>>
where
    Root: fmt::Debug + 'static,
    T: fmt::Debug + 'static,
{
    fn set_some(&self, value: T) {
        self.revise(|mut current| *current = Some(value));
    }

    fn update_some(&self, update: impl FnOnce(&mut T)) -> bool {
        self.revise(|mut current| {
            let Some(value) = current.as_mut() else {
                return false;
            };
            update(value);
            true
        })
    }

    fn take_value(&self) -> Option<T> {
        self.revise(|mut current| current.take())
    }

    fn clear_value(&self) {
        self.revise(|mut current| *current = None);
    }
}

/// Convenience operations for `HashMap` state.
pub trait HashMapStoreExt<K, V>
where
    K: fmt::Debug + Eq + Hash + 'static,
    V: fmt::Debug + 'static,
{
    fn insert_entry(&self, key: K, value: V) -> Option<V>;
    fn update_entry(&self, key: &K, update: impl FnOnce(&mut V)) -> bool;
    fn remove_entry(&self, key: &K) -> Option<V>;
    fn clear_entries(&self);
}

impl<K, V> HashMapStoreExt<K, V> for Cage<HashMap<K, V>>
where
    K: fmt::Debug + Eq + Hash + 'static,
    V: fmt::Debug + 'static,
{
    fn insert_entry(&self, key: K, value: V) -> Option<V> {
        self.revise(|mut map| map.insert(key, value))
    }

    fn update_entry(&self, key: &K, update: impl FnOnce(&mut V)) -> bool {
        self.revise(|mut map| {
            let Some(value) = map.get_mut(key) else {
                return false;
            };
            update(value);
            true
        })
    }

    fn remove_entry(&self, key: &K) -> Option<V> {
        self.revise(|mut map| map.remove(key))
    }

    fn clear_entries(&self) {
        self.revise(|mut map| map.clear());
    }
}

impl<Root, K, V> HashMapStoreExt<K, V> for CageLens<Root, HashMap<K, V>>
where
    Root: fmt::Debug + 'static,
    K: fmt::Debug + Eq + Hash + 'static,
    V: fmt::Debug + 'static,
{
    fn insert_entry(&self, key: K, value: V) -> Option<V> {
        self.revise(|mut map| map.insert(key, value))
    }

    fn update_entry(&self, key: &K, update: impl FnOnce(&mut V)) -> bool {
        self.revise(|mut map| {
            let Some(value) = map.get_mut(key) else {
                return false;
            };
            update(value);
            true
        })
    }

    fn remove_entry(&self, key: &K) -> Option<V> {
        self.revise(|mut map| map.remove(key))
    }

    fn clear_entries(&self) {
        self.revise(|mut map| map.clear());
    }
}

/// Convenience operations for `BTreeMap` state.
pub trait BTreeMapStoreExt<K, V>
where
    K: fmt::Debug + Ord + 'static,
    V: fmt::Debug + 'static,
{
    fn insert_entry(&self, key: K, value: V) -> Option<V>;
    fn update_entry(&self, key: &K, update: impl FnOnce(&mut V)) -> bool;
    fn remove_entry(&self, key: &K) -> Option<V>;
    fn clear_entries(&self);
}

impl<K, V> BTreeMapStoreExt<K, V> for Cage<BTreeMap<K, V>>
where
    K: fmt::Debug + Ord + 'static,
    V: fmt::Debug + 'static,
{
    fn insert_entry(&self, key: K, value: V) -> Option<V> {
        self.revise(|mut map| map.insert(key, value))
    }

    fn update_entry(&self, key: &K, update: impl FnOnce(&mut V)) -> bool {
        self.revise(|mut map| {
            let Some(value) = map.get_mut(key) else {
                return false;
            };
            update(value);
            true
        })
    }

    fn remove_entry(&self, key: &K) -> Option<V> {
        self.revise(|mut map| map.remove(key))
    }

    fn clear_entries(&self) {
        self.revise(|mut map| map.clear());
    }
}

impl<Root, K, V> BTreeMapStoreExt<K, V> for CageLens<Root, BTreeMap<K, V>>
where
    Root: fmt::Debug + 'static,
    K: fmt::Debug + Ord + 'static,
    V: fmt::Debug + 'static,
{
    fn insert_entry(&self, key: K, value: V) -> Option<V> {
        self.revise(|mut map| map.insert(key, value))
    }

    fn update_entry(&self, key: &K, update: impl FnOnce(&mut V)) -> bool {
        self.revise(|mut map| {
            let Some(value) = map.get_mut(key) else {
                return false;
            };
            update(value);
            true
        })
    }

    fn remove_entry(&self, key: &K) -> Option<V> {
        self.revise(|mut map| map.remove(key))
    }

    fn clear_entries(&self) {
        self.revise(|mut map| map.clear());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_store_ext_updates_nested_items() {
        let items = Cage::new(vec![("a", 1), ("b", 2)]);

        items.push_item(("c", 3));
        assert_eq!(&*items.get_untracked(), &[("a", 1), ("b", 2), ("c", 3)]);

        assert!(items.update_item_by(|(key, _)| *key == "b", |(_, value)| *value = 20));
        assert_eq!(&*items.get_untracked(), &[("a", 1), ("b", 20), ("c", 3)]);

        assert!(!items.update_item_by(|(key, _)| *key == "missing", |(_, value)| *value = 0));
        assert_eq!(items.remove_items_by(|(_, value)| *value >= 3), 2);
        assert_eq!(&*items.get_untracked(), &[("a", 1)]);

        items.clear_items();
        assert!(items.get_untracked().is_empty());
    }

    #[derive(Debug, PartialEq)]
    struct Profile {
        name: String,
        visits: usize,
    }

    #[derive(Debug, PartialEq)]
    struct AppState {
        profile: Profile,
        todos: Vec<(&'static str, bool)>,
        flags: HashMap<String, bool>,
        sorted: BTreeMap<String, usize>,
        draft: Option<String>,
    }

    #[test]
    fn cage_lens_updates_arbitrary_nested_fields() {
        let state = Cage::new(AppState {
            profile: Profile {
                name: "Ada".into(),
                visits: 1,
            },
            todos: vec![("write", false)],
            flags: HashMap::new(),
            sorted: BTreeMap::new(),
            draft: Some("first".into()),
        });

        let profile = state.lens(|state| &state.profile, |state| &mut state.profile);
        let name = profile.lens(|profile| &profile.name, |profile| &mut profile.name);
        assert_eq!(&*name.get_untracked(), "Ada");
        assert_eq!(name.set("Grace".into()), "Ada");
        profile.revise(|mut profile| profile.visits += 1);

        let todos = state.lens(|state| &state.todos, |state| &mut state.todos);
        todos.push_item(("ship", false));
        assert!(todos.update_item_by(|(title, _)| *title == "ship", |(_, done)| *done = true));

        let flags = state.lens(|state| &state.flags, |state| &mut state.flags);
        flags.insert_entry("dirty".into(), false);
        assert!(flags.update_entry(&"dirty".into(), |value| *value = true));

        let sorted = state.lens(|state| &state.sorted, |state| &mut state.sorted);
        sorted.insert_entry("a".into(), 1);
        sorted.insert_entry("b".into(), 2);
        assert_eq!(sorted.remove_entry(&"a".into()), Some(1));

        let draft = state.lens(|state| &state.draft, |state| &mut state.draft);
        assert!(draft.update_some(|value| value.push_str(" draft")));
        assert_eq!(draft.take_value().as_deref(), Some("first draft"));
        assert!(!draft.update_some(|value| value.push_str("missing")));

        let state = state.get_untracked();
        assert_eq!(state.profile.name, "Grace");
        assert_eq!(state.profile.visits, 2);
        assert_eq!(state.todos, vec![("write", false), ("ship", true)]);
        assert_eq!(state.flags.get("dirty"), Some(&true));
        assert_eq!(state.sorted.keys().collect::<Vec<_>>(), vec![&"b".to_owned()]);
        assert_eq!(state.draft, None);
    }

    #[test]
    fn cage_lens_maps_to_bond() {
        let state = Cage::new(Profile {
            name: "Ada".into(),
            visits: 1,
        });
        let visits = state.lens(|profile| &profile.visits, |profile| &mut profile.visits);
        let doubled = visits.map(|visits| *visits * 2).with_partial_eq();
        assert_eq!(*doubled.get_untracked(), 2);

        visits.revise(|mut visits| *visits = 4);
        assert_eq!(*doubled.get_untracked(), 8);
    }
}
