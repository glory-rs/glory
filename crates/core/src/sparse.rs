//! Internal numeric/schema maps. The common zero/one case needs no tree node.
//! These keys and values are runtime metadata, never application-owned values.
use std::{
    collections::{BTreeMap, btree_map},
    ops::{Bound, RangeBounds},
};
pub(super) enum SparseMap<K, V> {
    One(Option<(K, V)>),
    Many(BTreeMap<K, V>),
}
impl<K, V> Default for SparseMap<K, V> {
    fn default() -> Self {
        Self::One(None)
    }
}
impl<K: Ord, V> SparseMap<K, V> {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::One(one) => one.is_none(),
            Self::Many(map) => map.is_empty(),
        }
    }
    pub fn get(&self, key: &K) -> Option<&V> {
        match self {
            Self::One(one) => one.as_ref().filter(|(k, _)| k == key).map(|(_, v)| v),
            Self::Many(map) => map.get(key),
        }
    }
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self {
            Self::One(one) => {
                if let Some((old, v)) = one.as_mut()
                    && old == &key
                {
                    return Some(std::mem::replace(v, value));
                }
                if let Some(old) = one.take() {
                    let mut map = BTreeMap::new();
                    map.insert(old.0, old.1);
                    map.insert(key, value);
                    *self = Self::Many(map)
                } else {
                    *one = Some((key, value))
                }
                None
            }
            Self::Many(map) => map.insert(key, value),
        }
    }
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let result = match self {
            Self::One(one) => {
                if one.as_ref().is_some_and(|(k, _)| k == key) {
                    one.take().map(|(_, v)| v)
                } else {
                    None
                }
            }
            Self::Many(map) => map.remove(key),
        };
        if let Self::Many(map) = self
            && map.len() <= 1
        {
            let one = map.pop_first();
            *self = Self::One(one)
        }
        result
    }
    pub fn values(&self) -> impl Iterator<Item = &V> {
        let (one, many) = match self {
            Self::One(one) => (one.as_ref().map(|(_, v)| v), None),
            Self::Many(map) => (None, Some(map.values())),
        };
        one.into_iter().chain(many.into_iter().flatten())
    }
    pub fn into_values(self) -> impl Iterator<Item = V> {
        let (one, many) = match self {
            Self::One(one) => (one.map(|(_, v)| v), None),
            Self::Many(map) => (None, Some(map.into_values())),
        };
        one.into_iter().chain(many.into_iter().flatten())
    }
    pub fn range(&self, bounds: (Bound<K>, Bound<K>)) -> Range<'_, K, V> {
        match self {
            Self::One(one) => Range::One(
                one.as_ref()
                    .filter(|(key, _)| bounds.contains(key))
                    .map(|(k, v)| (k, v))
                    .into_iter(),
            ),
            Self::Many(map) => Range::Many(map.range(bounds)),
        }
    }
}
pub(super) enum Range<'a, K, V> {
    One(std::option::IntoIter<(&'a K, &'a V)>),
    Many(btree_map::Range<'a, K, V>),
}
impl<'a, K, V> Iterator for Range<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::One(one) => one.next(),
            Self::Many(many) => many.next(),
        }
    }
}
