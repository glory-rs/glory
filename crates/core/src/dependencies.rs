//! Empty/single dependencies need no tree allocation. Multi-source consumers
//! retain the original Key-sorted BTreeMap and its version semantics.
use super::Key;
use std::collections::{BTreeMap, btree_map};

#[derive(Clone, Default)]
pub(super) enum Dependencies {
    #[default]
    Empty,
    One(Key, u64),
    Many(BTreeMap<Key, u64>),
}
impl Dependencies {
    pub(super) fn new() -> Self {
        Self::Empty
    }
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::One(..) => 1,
            Self::Many(map) => map.len(),
        }
    }
    pub(super) fn insert(&mut self, key: Key, version: u64) -> Option<u64> {
        match self {
            Self::Empty => {
                *self = Self::One(key, version);
                None
            }
            Self::One(old, stamp) if *old == key => Some(std::mem::replace(stamp, version)),
            Self::One(old, stamp) => {
                let map = BTreeMap::from([(*old, *stamp), (key, version)]);
                *self = Self::Many(map);
                None
            }
            Self::Many(map) => map.insert(key, version),
        }
    }
    pub(super) fn contains_key(&self, key: &Key) -> bool {
        match self {
            Self::Empty => false,
            Self::One(old, _) => old == key,
            Self::Many(map) => map.contains_key(key),
        }
    }
    pub(super) fn keys(&self) -> impl Iterator<Item = &Key> {
        self.into_iter().map(|(key, _)| key)
    }
}

pub(super) enum Iter<'a> {
    Empty,
    One(Option<(&'a Key, &'a u64)>),
    Many(btree_map::Iter<'a, Key, u64>),
}
impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Key, &'a u64);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::One(entry) => entry.take(),
            Self::Many(iter) => iter.next(),
        }
    }
}
impl<'a> IntoIterator for &'a Dependencies {
    type Item = (&'a Key, &'a u64);
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        match self {
            Dependencies::Empty => Iter::Empty,
            Dependencies::One(key, stamp) => Iter::One(Some((key, stamp))),
            Dependencies::Many(map) => Iter::Many(map.iter()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_generations_cloning_and_order_match_the_tree_oracle() {
        let mut seed = 17_u64;
        for count in [0, 1, 2, 4, 32, 1024] {
            let mut actual = Dependencies::new();
            let mut oracle = BTreeMap::new();
            for version in 0..count {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let key = Key {
                    slot: (seed % 13) as u32,
                    generation: ((seed >> 32) % 3) as u32,
                };
                assert_eq!(actual.insert(key, version), oracle.insert(key, version));
                assert_eq!(actual.len(), oracle.len());
                assert!(actual.contains_key(&key));
                let cloned = actual.clone();
                assert_eq!(
                    cloned.keys().copied().collect::<Vec<_>>(),
                    oracle.keys().copied().collect::<Vec<_>>()
                );
                assert_eq!(
                    (&cloned).into_iter().collect::<Vec<_>>(),
                    oracle.iter().collect::<Vec<_>>()
                );
            }
        }
    }
}
