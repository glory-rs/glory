//! Most projected sources have one consumer. Keep that edge inline while
//! preserving Key order for fan-out and the existing invalidation schedule.
use super::Key;
use std::collections::{BTreeSet, btree_set};

#[derive(Default)]
pub(super) enum Subscribers {
    #[default]
    Empty,
    One(Key),
    Many(BTreeSet<Key>),
}
impl Subscribers {
    pub(super) fn new() -> Self {
        Self::Empty
    }
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::One(_) => 1,
            Self::Many(keys) => keys.len(),
        }
    }
    pub(super) fn insert(&mut self, key: Key) {
        match self {
            Self::Empty => *self = Self::One(key),
            Self::One(previous) if *previous == key => {}
            Self::One(previous) => *self = Self::Many(BTreeSet::from([*previous, key])),
            Self::Many(keys) => {
                keys.insert(key);
            }
        }
    }
    pub(super) fn remove(&mut self, key: &Key) {
        match self {
            Self::One(previous) if previous == key => *self = Self::Empty,
            Self::Many(keys) => {
                keys.remove(key);
                if keys.len() == 1 {
                    *self = Self::One(*keys.first().expect("one subscriber"));
                }
            }
            _ => {}
        }
    }
    pub(super) fn iter(&self) -> Iter<'_> {
        match self {
            Self::Empty => Iter::One(None),
            Self::One(key) => Iter::One(Some(key)),
            Self::Many(keys) => Iter::Many(keys.iter()),
        }
    }
}
pub(super) enum Iter<'a> {
    One(Option<&'a Key>),
    Many(btree_set::Iter<'a, Key>),
}
impl<'a> Iterator for Iter<'a> {
    type Item = &'a Key;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::One(key) => key.take(),
            Self::Many(keys) => keys.next(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transitions_preserve_order_and_membership() {
        let mut actual = Subscribers::new();
        let mut expected = BTreeSet::new();
        let mut random = 1u64;
        for _ in 0..4096 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let key = Key::unpack((random >> 32) % 31);
            if random & 8 == 0 {
                actual.insert(key);
                expected.insert(key);
            } else {
                actual.remove(&key);
                expected.remove(&key);
            }
            assert_eq!(actual.len(), expected.len());
            assert!(actual.iter().eq(expected.iter()));
        }
        for key in expected {
            actual.remove(&key);
        }
        assert_eq!(actual.len(), 0);
        assert!(matches!(actual, Subscribers::Empty));
    }
}
