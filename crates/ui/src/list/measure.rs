use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::ops::Range;

use super::{Error, Heights, Layout};

#[derive(Clone, Copy)]
struct Measured {
    index: usize,
    height: f64,
    age: u64,
}

/// Height metadata has a separate lifetime from row instances. The sparse index
/// stores at most retained measurements * log(total rows), with no full-data
/// allocation. Dropping a row still drops all of its UI/graph/host resources.
pub(super) struct Measurements<K> {
    pub layout: Layout,
    estimate: f64,
    adaptive: bool,
    limit: usize,
    measured: HashMap<K, Measured>,
    ages: BTreeMap<u64, K>,
    by_index: HashMap<usize, K>,
    // Coalesce by key without letting randomized hash iteration choose recency
    // (or floating-point update order). Repeated keys keep their first position
    // in this batch and replace only their final index/height.
    pending: Vec<(K, usize, f64)>,
    pending_indices: HashMap<K, usize>,
    clock: u64,
}
impl<K: Clone + Eq + Hash> Measurements<K> {
    pub fn new(
        len: usize,
        estimate: f64,
        adaptive: bool,
        limit: usize,
        overscan: usize,
    ) -> Result<Self, Error> {
        if adaptive && (limit == 0 || estimate < 1.0) {
            return Err(Error::InvalidSize);
        }
        let heights = if adaptive {
            Heights::sparse(len, estimate)?
        } else {
            Heights::fixed(len, estimate)?
        };
        Ok(Self {
            layout: Layout::new(heights, overscan),
            estimate,
            adaptive,
            limit,
            measured: HashMap::new(),
            ages: BTreeMap::new(),
            by_index: HashMap::new(),
            pending: Vec::new(),
            pending_indices: HashMap::new(),
            clock: 0,
        })
    }
    pub fn measured(&self) -> usize {
        self.measured.len()
    }
    pub fn queue(&mut self, key: K, index: usize, height: f64) {
        if let Some(&slot) = self.pending_indices.get(&key) {
            let entry = &mut self.pending[slot];
            entry.1 = index;
            entry.2 = height;
        } else {
            self.pending_indices.insert(key.clone(), self.pending.len());
            self.pending.push((key, index, height));
        }
    }
    fn remove(&mut self, key: &K) -> Result<bool, Error> {
        let Some(entry) = self.measured.remove(key) else {
            return Ok(false);
        };
        self.ages.remove(&entry.age);
        self.by_index.remove(&entry.index);
        self.layout
            .heights_mut()
            .measure(entry.index, self.estimate)
    }
    pub fn apply(&mut self, mut current: impl FnMut(usize, &K) -> bool) -> Result<bool, Error> {
        let mut pending = std::mem::take(&mut self.pending);
        self.pending_indices.clear();
        let result = (|| {
            let mut changed = false;
            for (key, index, height) in pending.drain(..) {
                // A queued callback may precede a data splice in the same host tick.
                if index >= self.layout.heights().len() || !current(index, &key) {
                    continue;
                }
                if let Some(previous) = self.measured.get(&key)
                    && previous.index != index
                {
                    changed |= self.remove(&key)?;
                }
                if let Some(previous) = self
                    .by_index
                    .get(&index)
                    .filter(|previous| **previous != key)
                    .cloned()
                {
                    changed |= self.remove(&previous)?;
                }
                changed |= self.layout.heights_mut().measure(index, height)?;
                if let Some(previous) = self.measured.remove(&key) {
                    self.ages.remove(&previous.age);
                }
                let age = self.clock;
                self.clock = self
                    .clock
                    .checked_add(1)
                    .expect("measurement age exhausted");
                self.ages.insert(age, key.clone());
                self.by_index.insert(index, key.clone());
                self.measured.insert(key, Measured { index, height, age });
            }
            Ok(changed)
        })();
        // Reuse only the current batch's storage, never a visited-key history.
        self.pending = pending;
        result
    }
    /// Data edits may relocate a bounded cache. The caller resolves stable keys;
    /// this never scans the source, nor runs on an ordinary scroll.
    pub fn rebuild(
        &mut self,
        len: usize,
        overscan: usize,
        mut locate: impl FnMut(&K) -> Result<Option<usize>, Error>,
    ) -> Result<(), Error> {
        let heights = if self.adaptive {
            Heights::sparse(len, self.estimate)?
        } else {
            Heights::fixed(len, self.estimate)?
        };
        let mut layout = Layout::new(heights, overscan);
        let mut entries = Vec::new();
        let mut indices = HashMap::new();
        for key in self.ages.values() {
            let entry = &self.measured[key];
            if let Some(index) = locate(key)? {
                if index >= len {
                    return Err(Error::InvalidLookup);
                }
                if indices.insert(index, key.clone()).is_some() {
                    return Err(Error::DuplicateKey);
                }
                layout.heights_mut().measure(index, entry.height)?;
                entries.push((key.clone(), Measured { index, ..*entry }));
            }
        }
        self.layout = layout;
        self.measured.clear();
        self.ages.clear();
        self.by_index = indices;
        for (key, entry) in entries {
            self.ages.insert(entry.age, key.clone());
            self.measured.insert(key, entry);
        }
        Ok(())
    }
    pub fn prune(&mut self, window: &Range<usize>) -> Result<bool, Error> {
        let limit = self.limit.max(window.len());
        if self.measured.len() <= limit {
            return Ok(false);
        }
        let evict: Vec<_> = self
            .ages
            .values()
            .filter(|key| !window.contains(&self.measured[*key].index))
            .take(self.measured.len() - limit)
            .cloned()
            .collect();
        let mut changed = false;
        for key in evict {
            changed |= self.remove(&key)?;
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retained(cache: &Measurements<usize>) -> Vec<usize> {
        cache.ages.values().copied().collect()
    }

    #[test]
    fn batch_order_and_eviction_are_independent_of_hash_seed() {
        for limit in [64, 1024] {
            for _ in 0..16 {
                let mut cache = Measurements::new(100_000_000, 24., true, limit, 2).unwrap();
                let count = limit + 12;
                for index in 0..count {
                    cache.queue(index, index, [24., 28., 32.][index % 3]);
                }
                cache.apply(|index, key| index == *key).unwrap();
                assert_eq!(retained(&cache), (0..count).collect::<Vec<_>>());
                cache.prune(&(count - 10..count)).unwrap();
                assert_eq!(retained(&cache), (12..count).collect::<Vec<_>>());
                assert_eq!(cache.measured(), limit);
                assert_eq!(cache.by_index.len(), limit);
                assert!(cache.layout.heights().storage_len() <= limit * 28);
                assert!(cache.pending.is_empty() && cache.pending_indices.is_empty());
            }
        }
    }

    #[test]
    fn repeated_keys_coalesce_final_value_at_first_batch_position() {
        let mut cache = Measurements::new(100, 24., true, 2, 0).unwrap();
        cache.queue(10, 0, 28.);
        cache.queue(20, 1, 32.);
        cache.queue(10, 2, 36.);
        cache.queue(30, 3, 40.);
        assert_eq!(cache.pending.len(), 3);
        cache.apply(|_, _| true).unwrap();
        assert_eq!(retained(&cache), vec![10, 20, 30]);
        assert_eq!(cache.measured[&10].index, 2);
        assert_eq!(cache.measured[&10].height, 36.);
        cache.prune(&(3..4)).unwrap();
        assert_eq!(retained(&cache), vec![20, 30]);
        // A later batch refreshes recency, even for an unchanged height.
        cache.queue(20, 1, 32.);
        assert!(!cache.apply(|_, _| true).unwrap());
        assert_eq!(retained(&cache), vec![30, 20]);
    }

    #[test]
    fn splice_revalidation_and_rebuild_keep_only_live_measurements() {
        let mut cache = Measurements::new(8, 24., true, 2, 0).unwrap();
        cache.queue(10, 0, 28.);
        cache.queue(20, 1, 32.);
        cache.apply(|_, _| true).unwrap();
        cache.queue(10, 0, 36.); // queued before a splice; now stale
        cache.queue(20, 2, 40.);
        cache
            .rebuild(8, 0, |key| Ok(Some(if *key == 10 { 1 } else { 2 })))
            .unwrap();
        cache.apply(|index, key| (*key, index) == (20, 2)).unwrap();
        assert_eq!(cache.measured[&10].height, 28.);
        assert_eq!(cache.measured[&20].height, 40.);
        assert_eq!(retained(&cache), vec![10, 20]);
        cache.rebuild(8, 0, |_| Ok(None)).unwrap(); // width invalidation
        assert!(cache.measured.is_empty() && cache.ages.is_empty());
        assert!(cache.by_index.is_empty());
        cache.queue(30, 0, 32.);
        cache.apply(|_, _| true).unwrap();
        assert_eq!(retained(&cache), vec![30]);
    }

    #[test]
    fn invalid_batch_does_not_poison_reused_pending_slots() {
        let mut cache = Measurements::new(4, 24., true, 2, 0).unwrap();
        cache.queue(0, 0, f64::NAN);
        cache.queue(1, 1, 32.);
        assert!(cache.apply(|_, _| true).is_err());
        assert!(cache.pending.is_empty() && cache.pending_indices.is_empty());
        cache.queue(1, 1, 36.);
        cache.apply(|_, _| true).unwrap();
        assert_eq!(cache.measured[&1].height, 36.);
        assert_eq!(retained(&cache), vec![1]);
    }

    #[test]
    fn fractional_updates_and_rebuild_have_reproducible_prefix_bits() {
        let mut expected = None;
        for _ in 0..16 {
            let mut cache = Measurements::new(1_000_000, 24., true, 64, 0).unwrap();
            for index in 0..80 {
                cache.queue(index, index, [24.1, 28.3, 32.125][index % 3]);
            }
            cache.apply(|_, _| true).unwrap();
            cache.prune(&(70..80)).unwrap();
            cache
                .rebuild(1_000_000, 0, |key| Ok(Some(100 + key)))
                .unwrap();
            let bits: Vec<_> = (100..=180)
                .map(|count| cache.layout.heights().prefix(count).unwrap().to_bits())
                .collect();
            assert_eq!(expected.get_or_insert_with(|| bits.clone()), &bits);
        }
    }
}
