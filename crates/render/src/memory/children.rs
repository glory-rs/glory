use std::ops::Deref;

use crate::NodeId;

/// Ordered live children, exposed as a contiguous slice. Consuming the front
/// advances a cursor instead of moving every remaining sibling. The inactive
/// prefix contains numeric identities only and is bounded by live + 64; an
/// empty sequence clears it immediately. Capacity follows peak live work.
#[derive(Default)]
pub struct Children {
    items: Vec<NodeId>,
    start: usize,
    #[cfg(test)]
    moved: usize,
}

impl Children {
    pub(super) fn insert(&mut self, index: usize, id: NodeId) {
        assert!(index <= self.len());
        if index == 0 && self.start != 0 {
            self.start -= 1;
            self.items[self.start] = id;
        } else {
            self.items.insert(self.start + index, id);
        }
    }

    pub(super) fn remove(&mut self, index: usize) {
        assert!(index < self.len());
        if index == 0 {
            self.start += 1;
        } else {
            self.items.remove(self.start + index);
        }
        let live = self.len();
        if live == 0 {
            self.items.clear();
            self.start = 0;
        } else if self.start > live.saturating_add(64) {
            #[cfg(test)]
            {
                self.moved += live;
            }
            self.items.drain(..self.start);
            self.start = 0;
        }
    }
}

impl Deref for Children {
    type Target = [NodeId];

    fn deref(&self) -> &Self::Target {
        &self.items[self.start..]
    }
}

impl std::fmt::Debug for Children {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl AsRef<[NodeId]> for Children {
    fn as_ref(&self) -> &[NodeId] {
        self
    }
}

impl Clone for Children {
    fn clone(&self) -> Self {
        Self {
            items: self.to_vec(),
            ..Self::default()
        }
    }
}

impl<T: AsRef<[NodeId]> + ?Sized> PartialEq<T> for Children {
    fn eq(&self, other: &T) -> bool {
        self.as_ref() == other.as_ref()
    }
}
impl Eq for Children {}

impl IntoIterator for Children {
    type Item = NodeId;
    type IntoIter = std::iter::Skip<std::vec::IntoIter<NodeId>>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter().skip(self.start)
    }
}

impl<'a> IntoIterator for &'a Children {
    type Item = &'a NodeId;
    type IntoIter = std::slice::Iter<'a, NodeId>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodeIds, RuntimeId};

    #[test]
    fn mixed_front_middle_and_tail_edits_match_an_ordered_vector() {
        let mut ids = NodeIds::new(RuntimeId::new());
        let mut children = Children::default();
        let mut expected = Vec::new();
        for turn in 0..10_000 {
            if expected.is_empty() || turn % 5 < 3 {
                let index = match turn % 3 {
                    0 => 0,
                    1 => expected.len(),
                    _ => expected.len() / 2,
                };
                let id = ids.allocate();
                children.insert(index, id);
                expected.insert(index, id);
            } else {
                let index = match turn % 3 {
                    0 => 0,
                    1 => expected.len() - 1,
                    _ => expected.len() / 2,
                };
                children.remove(index);
                ids.release(expected.remove(index));
            }
            assert_eq!(children.as_ref(), expected);
            assert!(children.items.len() <= 2 * children.len() + 64);
        }
    }

    #[test]
    fn front_retirement_is_bounded_and_amortized_without_exposing_old_identities() {
        let mut ids = NodeIds::new(RuntimeId::new());
        let mut children = Children::default();
        let mut expected = Vec::new();
        for _ in 0..32_768 {
            let id = ids.allocate();
            children.insert(children.len(), id);
            expected.push(id);
        }
        for index in 0..32_768 {
            let id = expected[index];
            children.remove(0);
            ids.release(id);
            assert_eq!(children.len(), expected.len() - index - 1);
            assert_eq!(children.first(), expected.get(index + 1));
            assert!(children.items.len() <= 2 * children.len() + 64);
            if index % 1_024 == 0 {
                // A cloned snapshot and owned iteration contain only live IDs.
                assert_eq!(children.as_ref(), &expected[index + 1..]);
                assert_eq!(
                    children.clone().into_iter().collect::<Vec<_>>(),
                    children.as_ref()
                );
            }
        }
        assert!(children.items.is_empty());
        assert!(children.moved < 32_768);
        let fresh = ids.allocate();
        children.insert(0, fresh);
        assert_eq!(children.as_ref(), &[fresh]);
    }
}
