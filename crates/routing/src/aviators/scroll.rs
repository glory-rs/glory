//! Per-history-entry scroll position memory.
//!
//! [`ScrollMemory`] is the platform-independent core of scroll restoration:
//! it associates a history-entry key (the navigation target's URL, in the
//! browser backend) with the `(x, y)` scroll offset that was in effect when
//! the user last left that entry. Navigating back/forward to a remembered
//! entry can then restore the saved offset, while navigating to a fresh
//! entry (no record) falls back to scrolling to the top.
//!
//! The type is deliberately free of any `web_sys` / DOM dependency so it
//! compiles and is unit-tested on the host target. The browser backend
//! ([`BrowserAviator`](super::BrowserAviator)) is the only place that reads
//! and writes the live `window.scrollX/scrollY`; it merely feeds those
//! numbers into / out of this store.

/// Default upper bound on the number of remembered scroll positions.
///
/// Browsing histories are effectively unbounded over a long session, so the
/// store evicts the least-recently-used entry once it would exceed this many
/// records. The cap keeps memory bounded without a visible behavior change
/// for realistic back/forward distances.
pub const DEFAULT_SCROLL_MEMORY_CAPACITY: usize = 64;

/// An `(x, y)` scroll offset, in CSS pixels, matching `window.scrollX` /
/// `window.scrollY`.
pub type ScrollPosition = (f64, f64);

/// LRU-bounded map from a history-entry key to its remembered scroll offset.
///
/// Keys are arbitrary strings; the browser backend uses the entry's full URL
/// (path + query + fragment). The most recently `record`ed or `restore`d key
/// is considered most-recently-used and is evicted last.
#[derive(Debug, Clone)]
pub struct ScrollMemory {
    capacity: usize,
    /// `(key, position)` pairs ordered from least- to most-recently-used.
    /// Kept as a `Vec` rather than a map because the working set is small
    /// (bounded by `capacity`) and we need cheap recency reordering.
    entries: Vec<(String, ScrollPosition)>,
}

impl Default for ScrollMemory {
    fn default() -> Self {
        Self::new(DEFAULT_SCROLL_MEMORY_CAPACITY)
    }
}

impl ScrollMemory {
    /// Create a store that remembers at most `capacity` scroll positions.
    ///
    /// A `capacity` of `0` is clamped to `1` so the store always remembers at
    /// least the most recent entry.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    /// Maximum number of remembered positions.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of currently remembered positions.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is remembered yet.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn index_of(&self, key: &str) -> Option<usize> {
        self.entries.iter().position(|(k, _)| k == key)
    }

    /// Record the scroll position for `key`, overwriting any previous value
    /// and marking the key as most-recently-used. Evicts the least-recently
    /// used entry if recording would exceed the capacity.
    pub fn record(&mut self, key: impl Into<String>, x: f64, y: f64) {
        let key = key.into();
        if let Some(idx) = self.index_of(&key) {
            // Overwrite in place, then move to the most-recently-used end.
            let mut entry = self.entries.remove(idx);
            entry.1 = (x, y);
            self.entries.push(entry);
            return;
        }
        if self.entries.len() >= self.capacity {
            // Evict least-recently-used (front).
            self.entries.remove(0);
        }
        self.entries.push((key, (x, y)));
    }

    /// Return the remembered scroll position for `key`, if any, marking the
    /// key as most-recently-used. Returns `None` for keys never recorded
    /// (or already evicted/forgotten) — the browser backend treats this as
    /// "scroll to top".
    pub fn restore(&mut self, key: &str) -> Option<ScrollPosition> {
        let idx = self.index_of(key)?;
        let entry = self.entries.remove(idx);
        let pos = entry.1;
        self.entries.push(entry);
        Some(pos)
    }

    /// Look up the remembered position without affecting recency.
    pub fn peek(&self, key: &str) -> Option<ScrollPosition> {
        self.index_of(key).map(|idx| self.entries[idx].1)
    }

    /// Drop the remembered position for `key`. Returns the dropped value, if
    /// the key was present.
    pub fn forget(&mut self, key: &str) -> Option<ScrollPosition> {
        let idx = self.index_of(key)?;
        Some(self.entries.remove(idx).1)
    }

    /// Drop all remembered positions.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_then_restore_round_trips() {
        let mut mem = ScrollMemory::default();
        mem.record("/a", 10.0, 20.0);
        assert_eq!(mem.restore("/a"), Some((10.0, 20.0)));
    }

    #[test]
    fn unknown_key_returns_none() {
        let mut mem = ScrollMemory::default();
        mem.record("/a", 1.0, 2.0);
        assert_eq!(mem.restore("/missing"), None);
        assert_eq!(mem.peek("/missing"), None);
    }

    #[test]
    fn record_overwrites_existing_key() {
        let mut mem = ScrollMemory::default();
        mem.record("/a", 1.0, 2.0);
        mem.record("/a", 3.0, 4.0);
        assert_eq!(mem.len(), 1);
        assert_eq!(mem.restore("/a"), Some((3.0, 4.0)));
    }

    #[test]
    fn capacity_evicts_least_recently_used() {
        let mut mem = ScrollMemory::new(2);
        mem.record("/a", 1.0, 1.0);
        mem.record("/b", 2.0, 2.0);
        // Inserting a third entry evicts the LRU ("/a").
        mem.record("/c", 3.0, 3.0);
        assert_eq!(mem.len(), 2);
        assert_eq!(mem.peek("/a"), None);
        assert_eq!(mem.peek("/b"), Some((2.0, 2.0)));
        assert_eq!(mem.peek("/c"), Some((3.0, 3.0)));
    }

    #[test]
    fn restore_refreshes_recency_protecting_from_eviction() {
        let mut mem = ScrollMemory::new(2);
        mem.record("/a", 1.0, 1.0);
        mem.record("/b", 2.0, 2.0);
        // Touch "/a" so it becomes most-recently-used.
        assert_eq!(mem.restore("/a"), Some((1.0, 1.0)));
        // Now inserting "/c" should evict "/b" (the LRU), not "/a".
        mem.record("/c", 3.0, 3.0);
        assert_eq!(mem.peek("/a"), Some((1.0, 1.0)));
        assert_eq!(mem.peek("/b"), None);
        assert_eq!(mem.peek("/c"), Some((3.0, 3.0)));
    }

    #[test]
    fn record_refreshes_recency_protecting_from_eviction() {
        let mut mem = ScrollMemory::new(2);
        mem.record("/a", 1.0, 1.0);
        mem.record("/b", 2.0, 2.0);
        // Re-record "/a" -> most-recently-used.
        mem.record("/a", 9.0, 9.0);
        mem.record("/c", 3.0, 3.0);
        assert_eq!(mem.peek("/a"), Some((9.0, 9.0)));
        assert_eq!(mem.peek("/b"), None);
        assert_eq!(mem.peek("/c"), Some((3.0, 3.0)));
    }

    #[test]
    fn zero_capacity_is_clamped_to_one() {
        let mut mem = ScrollMemory::new(0);
        assert_eq!(mem.capacity(), 1);
        mem.record("/a", 1.0, 1.0);
        mem.record("/b", 2.0, 2.0);
        assert_eq!(mem.len(), 1);
        assert_eq!(mem.peek("/a"), None);
        assert_eq!(mem.peek("/b"), Some((2.0, 2.0)));
    }

    #[test]
    fn forget_drops_single_entry() {
        let mut mem = ScrollMemory::default();
        mem.record("/a", 1.0, 2.0);
        mem.record("/b", 3.0, 4.0);
        assert_eq!(mem.forget("/a"), Some((1.0, 2.0)));
        assert_eq!(mem.forget("/a"), None);
        assert_eq!(mem.peek("/a"), None);
        assert_eq!(mem.peek("/b"), Some((3.0, 4.0)));
    }

    #[test]
    fn clear_drops_everything() {
        let mut mem = ScrollMemory::default();
        mem.record("/a", 1.0, 2.0);
        mem.record("/b", 3.0, 4.0);
        mem.clear();
        assert!(mem.is_empty());
        assert_eq!(mem.peek("/a"), None);
        assert_eq!(mem.peek("/b"), None);
    }
}
