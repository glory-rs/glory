use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::ops::Range;

use super::Error;

/// A runtime adapter supplies indexed data access and owns row mounting. `key`
/// must be O(1) or O(log n), and keys must be globally stable/unique. Validation
/// checks the current window only, avoiding an O(n) scan on every scroll.
pub trait RowHost {
    type Key: Clone + Eq + Hash;
    type Row;
    fn key(&mut self, index: usize) -> Self::Key;
    fn mount(&mut self, index: usize, key: &Self::Key) -> Self::Row;
    /// Refresh an existing row after a data revision or its index changes.
    fn update(&mut self, index: usize, key: &Self::Key, row: &mut Self::Row);
    fn place(&mut self, row: &Self::Row, before: Option<&Self::Row>);
    /// Empty logical widgets do not provide a physical insertion anchor.
    fn has_content(&self, _row: &Self::Row) -> bool {
        true
    }
    fn dispose(&mut self, row: Self::Row);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReconcileStats {
    pub keys_read: usize,
    pub created: usize,
    pub updated: usize,
    pub moved: usize,
    pub disposed: usize,
}

#[derive(Debug)]
struct Entry<Row> {
    index: usize,
    // Position in the last committed window, independent of the source index.
    // Kept with the live row instead of cloning every key into a temporary map.
    order: usize,
    row: Row,
}

struct Next<Key> {
    index: usize,
    key: Key,
    old: Option<usize>,
}

/// Owns the mounted window plus an optional single pin. Other leaving rows are
/// disposed immediately. The widget adapter resolves the focused stable key;
/// this is not a keep-alive cache of every row ever visited.
#[derive(Debug)]
pub struct Cache<Key, Row> {
    rows: HashMap<Key, Entry<Row>>,
    order: Vec<Key>,
    range: Range<usize>,
    revision: Option<u64>,
    pin: Option<(usize, Key)>,
}

impl<Key: Clone + Eq + Hash, Row> Default for Cache<Key, Row> {
    fn default() -> Self {
        Self {
            rows: HashMap::new(),
            order: Vec::new(),
            range: 0..0,
            revision: None,
            pin: None,
        }
    }
}

impl<Key: Clone + Eq + Hash, Row> Cache<Key, Row> {
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn row(&self, key: &Key) -> Option<&Row> {
        self.rows.get(key).map(|entry| &entry.row)
    }
    pub fn order(&self) -> &[Key] {
        &self.order
    }

    pub fn reconcile(
        &mut self,
        range: Range<usize>,
        revision: u64,
        host: &mut impl RowHost<Key = Key, Row = Row>,
    ) -> Result<ReconcileStats, Error> {
        self.reconcile_pinned(range, revision, None, host)
    }

    /// Reconcile the window plus at most one indexed stable key. The caller
    /// resolves/validates the pin without scanning the source. It is part of
    /// the same order/LIS reconciliation, never a separate historical cache.
    pub fn reconcile_pinned(
        &mut self,
        range: Range<usize>,
        revision: u64,
        pin: Option<(usize, Key)>,
        host: &mut impl RowHost<Key = Key, Row = Row>,
    ) -> Result<ReconcileStats, Error> {
        if range.start > range.end {
            return Err(Error::InvalidRange);
        }
        if self.range == range && self.revision == Some(revision) && self.pin == pin {
            return Ok(ReconcileStats::default());
        }
        let extra = usize::from(
            pin.as_ref()
                .is_some_and(|(index, _)| !range.contains(index)),
        );
        let requested = range.len().checked_add(extra).ok_or(Error::Allocation)?;
        let mut next = Vec::new();
        next.try_reserve(requested).map_err(|_| Error::Allocation)?;
        let mut keys = HashSet::new();
        keys.try_reserve(requested).map_err(|_| Error::Allocation)?;
        for index in range.clone() {
            let key = host.key(index);
            if !keys.insert(key.clone()) {
                return Err(Error::DuplicateKey);
            }
            let old = self.rows.get(&key).map(|entry| entry.order);
            next.push(Next { index, key, old });
        }
        if let Some((index, key)) = &pin {
            if range.contains(index) {
                if next[index - range.start].key != *key {
                    return Err(Error::InvalidLookup);
                }
            } else {
                if !keys.insert(key.clone()) {
                    return Err(Error::DuplicateKey);
                }
                let row = Next {
                    index: *index,
                    key: key.clone(),
                    old: self.rows.get(key).map(|entry| entry.order),
                };
                if *index < range.start {
                    next.insert(0, row);
                } else {
                    next.push(row);
                }
            }
        }
        // All key validation precedes side effects on mounted instances.
        // Ordinary scrolls, distant seeks, insertions and deletions retain row
        // order. Only a real inversion needs the LIS allocations and search.
        let mut previous = None;
        let reordered = next.iter().filter_map(|row| row.old).any(|old| {
            let inverted = previous.is_some_and(|previous| previous >= old);
            previous = Some(old);
            inverted
        });
        let stable = reordered.then(|| {
            let positions: Vec<_> = next.iter().map(|row| row.old).collect();
            stable_positions(&positions)
        });
        let mut stats = ReconcileStats {
            keys_read: range.len(),
            ..ReconcileStats::default()
        };
        for key in &self.order {
            if !keys.contains(key) {
                host.dispose(self.rows.remove(key).expect("mounted key").row);
                stats.disposed += 1;
            }
        }
        let changed = self.revision != Some(revision);
        for (order, Next { index, key, .. }) in next.iter().enumerate() {
            if let Some(entry) = self.rows.get_mut(key) {
                if changed || entry.index != *index {
                    host.update(*index, key, &mut entry.row);
                    entry.index = *index;
                    stats.updated += 1;
                }
                entry.order = order;
            } else {
                let row = host.mount(*index, key);
                self.rows.insert(
                    key.clone(),
                    Entry {
                        index: *index,
                        order,
                        row,
                    },
                );
                stats.created += 1;
            }
        }
        let mut before = None;
        for i in (0..next.len()).rev() {
            if next[i].old.is_none() || stable.as_ref().is_some_and(|stable| !stable[i]) {
                let anchor = before.map(|index: usize| &self.rows[&next[index].key].row);
                host.place(&self.rows[&next[i].key].row, anchor);
                if next[i].old.is_some() {
                    stats.moved += 1;
                }
            }
            if host.has_content(&self.rows[&next[i].key].row) {
                before = Some(i);
            }
        }
        self.order = next.into_iter().map(|next| next.key).collect();
        self.range = range;
        self.revision = Some(revision);
        self.pin = pin;
        Ok(stats)
    }

    pub fn clear(&mut self, host: &mut impl RowHost<Key = Key, Row = Row>) {
        for key in self.order.drain(..) {
            host.dispose(self.rows.remove(&key).expect("mounted key").row);
        }
        self.range = 0..0;
        self.revision = None;
        self.pin = None;
    }
}

/// Mark a longest increasing subsequence of retained old positions. This keeps
/// window shifts free of moves, while minimizing moves on actual data reorder.
fn stable_positions(positions: &[Option<usize>]) -> Vec<bool> {
    let mut tails: Vec<usize> = Vec::new();
    let mut previous = vec![None; positions.len()];
    for (i, position) in positions.iter().enumerate() {
        let Some(position) = position else { continue };
        let at =
            tails.partition_point(|tail| positions[*tail].expect("retained position") < *position);
        if at > 0 {
            previous[i] = Some(tails[at - 1]);
        }
        if at == tails.len() {
            tails.push(i);
        } else {
            tails[at] = i;
        }
    }
    let mut stable = vec![false; positions.len()];
    let mut cursor = tails.last().copied();
    while let Some(i) = cursor {
        stable[i] = true;
        cursor = previous[i];
    }
    stable
}
