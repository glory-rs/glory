use crate::NodeId;
use std::{collections::HashMap, ops::Deref, sync::OnceLock};

// Promote only when a wide sequence would search or shift siblings. Append and
// amortized front retirement retain the original contiguous representation.
const INDEX_THRESHOLD: usize = 2048;
#[derive(Default)]
enum Order {
    #[default]
    Empty,
    Contiguous {
        items: Vec<NodeId>,
        start: usize,
    },
    Indexed(Box<Indexed>),
}
#[derive(Clone, Copy)]
struct Links {
    previous: Option<NodeId>,
    next: Option<NodeId>,
}
struct Indexed {
    links: HashMap<NodeId, Links>,
    first: Option<NodeId>,
    last: Option<NodeId>,
    snapshot: OnceLock<Vec<NodeId>>,
}
impl Indexed {
    fn ordered(&self) -> Vec<NodeId> {
        let mut result = Vec::with_capacity(self.links.len());
        let mut next = self.first;
        while let Some(id) = next {
            result.push(id);
            next = self.links[&id].next;
        }
        result
    }
}
/// Ordered live children. Small/append-only sequences use a vector with an
/// amortized front-retirement cursor. Wide sequences promote on non-boundary
/// edits to a live-identity index, avoiding repeated sibling searches/shifts.
/// No retired identities or business values enter that index.
///
/// The public contiguous slice is materialized lazily after indexed edits.
/// Reading the slice after *each* edit still costs O(n); repeated reads without
/// an edit reuse it. Internal placement and adjacency do not read the slice.
#[derive(Default)]
pub struct Children {
    order: Order,
    #[cfg(test)]
    work: Work,
}
#[cfg(test)]
#[derive(Default)]
struct Work {
    moved: usize,
    searched: usize,
    promoted: usize,
    indexed: usize,
}
impl Children {
    pub fn len(&self) -> usize {
        match &self.order {
            Order::Empty => 0,
            Order::Contiguous { items, start } => items.len() - start,
            Order::Indexed(order) => order.links.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn first(&self) -> Option<&NodeId> {
        match &self.order {
            Order::Empty => None,
            Order::Contiguous { items, start } => items.get(*start),
            Order::Indexed(order) => order.first.as_ref(),
        }
    }
    pub fn last(&self) -> Option<&NodeId> {
        match &self.order {
            Order::Empty => None,
            Order::Contiguous { items, start } => items.get(*start..).and_then(|v| v.last()),
            Order::Indexed(order) => order.last.as_ref(),
        }
    }
    fn promote(&mut self) {
        let Order::Contiguous { items, start } = std::mem::take(&mut self.order) else {
            unreachable!("promote contiguous children");
        };
        let live = &items[start..];
        let mut links = HashMap::with_capacity(live.len());
        for (index, &id) in live.iter().enumerate() {
            links.insert(
                id,
                Links {
                    previous: index.checked_sub(1).map(|i| live[i]),
                    next: live.get(index + 1).copied(),
                },
            );
        }
        #[cfg(test)]
        {
            self.work.promoted += live.len();
        }
        self.order = Order::Indexed(Box::new(Indexed {
            first: live.first().copied(),
            last: live.last().copied(),
            links,
            snapshot: OnceLock::new(),
        }));
    }
    /// Insert a detached identity and return its preceding live sibling.
    pub(super) fn insert_before(&mut self, id: NodeId, before: Option<NodeId>) -> Option<NodeId> {
        if matches!(self.order, Order::Empty) {
            assert!(before.is_none());
            let mut items = Vec::new();
            // Match the original vector's amortized small-child capacity.
            items.insert(0, id);
            self.order = Order::Contiguous { items, start: 0 };
            return None;
        }
        if let Order::Contiguous { items, start } = &self.order
            && before.is_some()
            && !(before.as_ref() == items.get(*start) && *start != 0)
            && items.len() - start > INDEX_THRESHOLD
        {
            self.promote();
        }
        match &mut self.order {
            Order::Empty => unreachable!(),
            Order::Contiguous { items, start } => {
                let index = before.map_or(items.len() - *start, |before| {
                    if items[*start] == before {
                        return 0;
                    }
                    let index = items[*start..]
                        .iter()
                        .position(|&child| child == before)
                        .expect("validated sibling");
                    #[cfg(test)]
                    {
                        self.work.searched += index + 1;
                    }
                    index
                });
                let previous = index.checked_sub(1).map(|i| items[*start + i]);
                if index == 0 && *start != 0 {
                    *start -= 1;
                    items[*start] = id;
                } else {
                    #[cfg(test)]
                    {
                        self.work.moved += items.len() - *start - index;
                    }
                    items.insert(*start + index, id);
                }
                previous
            }
            Order::Indexed(order) => {
                order.snapshot.take();
                let previous = before.map_or(order.last, |before| order.links[&before].previous);
                let old = order.links.insert(
                    id,
                    Links {
                        previous,
                        next: before,
                    },
                );
                assert!(old.is_none(), "insert detached identity");
                if let Some(previous) = previous {
                    order
                        .links
                        .get_mut(&previous)
                        .expect("previous sibling")
                        .next = Some(id);
                } else {
                    order.first = Some(id);
                }
                if let Some(before) = before {
                    order.links.get_mut(&before).expect("next sibling").previous = Some(id);
                } else {
                    order.last = Some(id);
                }
                #[cfg(test)]
                {
                    self.work.indexed += 1;
                }
                previous
            }
        }
    }
    /// Remove a placed identity and return its preceding live sibling.
    pub(super) fn remove_id(&mut self, id: NodeId) -> Option<NodeId> {
        if let Order::Contiguous { items, start } = &self.order
            && items.get(*start) != Some(&id)
            && items.last() != Some(&id)
            && items.len() - start > INDEX_THRESHOLD
        {
            self.promote();
        }
        let previous = match &mut self.order {
            Order::Empty => unreachable!("remove placed identity"),
            Order::Contiguous { items, start } => {
                let index = if items[*start] == id {
                    0
                } else if items.last() == Some(&id) {
                    items.len() - *start - 1
                } else {
                    let index = items[*start..]
                        .iter()
                        .position(|&child| child == id)
                        .expect("placed sibling");
                    #[cfg(test)]
                    {
                        self.work.searched += index + 1;
                    }
                    index
                };
                let previous = index.checked_sub(1).map(|i| items[*start + i]);
                if index == 0 {
                    *start += 1;
                } else {
                    #[cfg(test)]
                    {
                        self.work.moved += items.len() - *start - index - 1;
                    }
                    items.remove(*start + index);
                }
                let live = items.len() - *start;
                if live == 0 {
                    items.clear();
                    *start = 0;
                } else if *start > live.saturating_add(64) {
                    #[cfg(test)]
                    {
                        self.work.moved += live;
                    }
                    items.drain(..*start);
                    *start = 0;
                }
                previous
            }
            Order::Indexed(order) => {
                order.snapshot.take();
                let removed = order.links.remove(&id).expect("placed sibling");
                if let Some(previous) = removed.previous {
                    order
                        .links
                        .get_mut(&previous)
                        .expect("previous sibling")
                        .next = removed.next;
                } else {
                    order.first = removed.next;
                }
                if let Some(next) = removed.next {
                    order.links.get_mut(&next).expect("next sibling").previous = removed.previous;
                } else {
                    order.last = removed.previous;
                }
                #[cfg(test)]
                {
                    self.work.indexed += 1;
                }
                removed.previous
            }
        };
        // Hysteresis avoids repeated promotion while releasing a formerly wide
        // index when only a small tail remains.
        if let Order::Indexed(order) = &self.order
            && order.links.len() <= INDEX_THRESHOLD / 2
        {
            let items = order.ordered();
            self.order = Order::Contiguous { items, start: 0 };
        }
        previous
    }
}
impl Deref for Children {
    type Target = [NodeId];
    fn deref(&self) -> &Self::Target {
        match &self.order {
            Order::Empty => &[],
            Order::Contiguous { items, start } => &items[*start..],
            Order::Indexed(order) => order.snapshot.get_or_init(|| order.ordered()),
        }
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
            order: Order::Contiguous {
                items: self.to_vec(),
                start: 0,
            },
            #[cfg(test)]
            work: Work::default(),
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
        let (items, start) = match self.order {
            Order::Empty => (Vec::new(), 0),
            Order::Contiguous { items, start } => (items, start),
            Order::Indexed(mut order) => {
                (order.snapshot.take().unwrap_or_else(|| order.ordered()), 0)
            }
        };
        items.into_iter().skip(start)
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
    use crate::{memory::Memory, *};
    #[test]
    fn edits_and_lazy_snapshots_match_vector_across_promotions_and_retirement() {
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
                let previous = children.insert_before(id, expected.get(index).copied());
                assert_eq!(previous, index.checked_sub(1).map(|i| expected[i]));
                expected.insert(index, id);
            } else {
                let index = match turn % 3 {
                    0 => 0,
                    1 => expected.len() - 1,
                    _ => expected.len() / 2,
                };
                let previous = children.remove_id(expected[index]);
                assert_eq!(previous, index.checked_sub(1).map(|i| expected[i]));
                ids.release(expected.remove(index));
            }
            assert_eq!(children.len(), expected.len());
            assert_eq!(children.first(), expected.first());
            assert_eq!(children.last(), expected.last());
            assert_eq!(children.as_ref(), expected);
            if turn % 127 == 0 {
                assert_eq!(children.clone().into_iter().collect::<Vec<_>>(), expected);
            }
        }
        for id in expected {
            children.remove_id(id);
        }
        assert!(children.is_empty());
        assert!(matches!(children.order, Order::Contiguous { .. }));
    }
    #[test]
    fn append_and_front_retirement_keep_the_amortized_vector_path() {
        let mut ids = NodeIds::new(RuntimeId::new());
        let mut children = Children::default();
        let expected: Vec<_> = (0..32_768).map(|_| ids.allocate()).collect();
        for &id in &expected {
            children.insert_before(id, None);
        }
        for (index, &id) in expected.iter().enumerate() {
            assert_eq!(children.remove_id(id), None);
            assert_eq!(children.first(), expected.get(index + 1));
            let Order::Contiguous { items, .. } = &children.order else {
                panic!("vector fast path");
            };
            assert!(items.len() <= 2 * children.len() + 64);
        }
        assert!(children.work.moved < expected.len());
        assert_eq!(children.work.promoted, 0);
        assert_eq!(children.work.searched, 0);
    }
    #[test]
    fn wide_reversal_has_linear_index_work_and_no_implicit_slice_reads() {
        for count in [4_096, 10_000, 50_000] {
            let mut ids = NodeIds::new(RuntimeId::new());
            let expected: Vec<_> = (0..count).map(|_| ids.allocate()).collect();
            let mut children = Children::default();
            for &id in &expected {
                children.insert_before(id, None);
            }
            for &id in &expected[1..] {
                children.remove_id(id);
                children.insert_before(id, children.first().copied());
            }
            let Order::Indexed(order) = &children.order else {
                panic!("wide index");
            };
            assert!(order.snapshot.get().is_none());
            assert_eq!(children.work.promoted, count);
            assert_eq!(children.work.indexed, 2 * (count - 1));
            assert_eq!(children.work.moved + children.work.searched, 0);
            assert!(children.iter().copied().eq(expected.iter().rev().copied()));
            assert!(order.snapshot.get().is_some());
        }
    }

    #[test]
    fn wide_prepend_and_shuffle_use_live_identity_work_without_searching_or_shifting() {
        for count in [10_000, 50_000] {
            for prepend in [false, true] {
                let mut ids = NodeIds::new(RuntimeId::new());
                let rows: Vec<_> = (0..count).map(|_| ids.allocate()).collect();
                let mut children = Children::default();
                for &id in &rows {
                    children.insert_before(id, None);
                }
                let mut expected = rows.clone();
                if prepend {
                    let incoming: Vec<_> = (0..count).map(|_| ids.allocate()).collect();
                    for &id in &incoming {
                        children.insert_before(id, Some(rows[0]));
                    }
                    expected.splice(..0, incoming);
                } else {
                    let mut seed = 17_u64;
                    for index in (1..count).rev() {
                        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                        expected.swap(index, seed as usize % (index + 1));
                    }
                    let mut before = None;
                    for &id in expected.iter().rev() {
                        children.remove_id(id);
                        children.insert_before(id, before);
                        before = Some(id);
                    }
                }
                let Order::Indexed(order) = &children.order else {
                    panic!("wide index");
                };
                assert!(order.snapshot.get().is_none());
                assert_eq!(children.work.promoted, count);
                assert!(children.work.indexed <= count * 2);
                assert_eq!(children.work.moved + children.work.searched, 0);
                assert_eq!(children.as_ref(), expected);
            }
        }
    }

    #[test]
    fn complete_memory_placement_does_not_materialize_wide_sibling_slices() {
        // Repeat inside a declared focus scope as well: its validation must not
        // hide a sibling scan in scope_movement/preflight on ordinary moves.
        for scoped in [false, true] {
            let runtime = RuntimeId::new();
            let mut ids = NodeIds::new(runtime);
            let mut tree = Memory::new(runtime);
            let parent = ids.allocate();
            let mut descriptor = Descriptor::new(Props::View);
            if scoped {
                descriptor.focus = FocusMode::Programmatic;
                descriptor.semantics = Some(Box::new(Semantics {
                    focus_scope: Some(FocusScopeConfig::default()),
                    ..Default::default()
                }));
            }
            tree.apply(&[Mutation::Create {
                id: parent,
                node: Box::new(descriptor),
            }])
            .unwrap();
            let rows: Vec<_> = (0..3_000).map(|_| ids.allocate()).collect();
            for &id in &rows {
                tree.apply(&[
                    Mutation::Create {
                        id,
                        node: Box::new(Descriptor::new(Props::View)),
                    },
                    Mutation::Place {
                        id,
                        parent: Some(parent),
                        before: None,
                    },
                ])
                .unwrap();
            }
            for &id in &rows[1..] {
                let before = tree.nodes[&parent].children.first().copied();
                tree.validate_placement(id, Some(parent), before, false)
                    .unwrap();
                tree.apply(&[Mutation::Place {
                    id,
                    parent: Some(parent),
                    before,
                }])
                .unwrap();
                assert_eq!(tree.next_sibling(id).unwrap(), before);
                let Order::Indexed(order) = &tree.nodes[&parent].children.order else {
                    panic!("wide index");
                };
                assert!(order.snapshot.get().is_none());
            }
            assert!(
                tree.nodes[&parent]
                    .children
                    .iter()
                    .copied()
                    .eq(rows.iter().rev().copied())
            );
            if scoped {
                assert_eq!(tree.focus_scope_nodes(parent).unwrap(), Some(3_001));
            }
        }
    }
}
