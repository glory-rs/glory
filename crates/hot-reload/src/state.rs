//! Reload-time state preservation.
//!
//! Function reload ([`crate::FunctionRegistry`] / [`crate::ReloadableFn`]) swaps
//! the body of a registered closure in place, but it does **not** carry the
//! surrounding component state across a reload: the host has to re-register
//! closures, and any value held only inside the old closure is dropped. The
//! [`ReloadStateStore`] gives the host an opt-in place to stash the few values
//! that matter (the contents of key `Cage`s, scroll positions, form drafts, …)
//! *before* a reload and read them back *after*.
//!
//! # Collaboration pattern
//!
//! The store is deliberately small: it is an in-memory `key -> bytes` map where
//! each entry is a `serde` snapshot of a value. The host drives it in two
//! phases around a reload:
//!
//! 1. **Before the reload** (e.g. at the top of an `on_function_reload`
//!    callback, or in a `before` reload hook) snapshot the values you want to
//!    keep:
//!
//!    ```
//!    use glory_hot_reload::ReloadStateStore;
//!
//!    let store = ReloadStateStore::new();
//!    // `count` is whatever a key `Cage<i32>` currently holds.
//!    let count: i32 = 41;
//!    store.snapshot("counter", &count).unwrap();
//!    ```
//!
//! 2. **After the reload** — once the new closures are registered and a fresh
//!    component tree has been built — restore the snapshots back into the new
//!    `Cage`s:
//!
//!    ```
//!    # use glory_hot_reload::ReloadStateStore;
//!    # let store = ReloadStateStore::new();
//!    # store.snapshot("counter", &41i32).unwrap();
//!    if let Some(count) = store.restore::<i32>("counter") {
//!        // write `count` back into the freshly created `Cage<i32>`.
//!        assert_eq!(count, 41);
//!    }
//!    ```
//!
//! A typical `on_function_reload` callback therefore looks like:
//!
//! ```no_run
//! use glory_hot_reload::ReloadStateStore;
//!
//! fn on_function_reload(store: &ReloadStateStore) {
//!     // 1. snapshot the live Cage values before swapping closures.
//!     // store.snapshot("counter", &counter_cage.get()).unwrap();
//!
//!     // 2. apply the function reload (swap closures, rebuild the tree)…
//!
//!     // 3. restore the values into the rebuilt Cages.
//!     // if let Some(v) = store.restore::<i32>("counter") { counter_cage.set(v); }
//! }
//! ```
//!
//! Because every entry is independent, the host can preserve only the state it
//! actually cares about and let everything else reset — which is usually the
//! desired behaviour when iterating on a component.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// In-memory store of serialized state snapshots, keyed by string.
///
/// Cheap to [`Clone`] — clones share the same underlying map, so a snapshot
/// written through one handle is visible through any other. This lets the host
/// hold a clone in a reload hook while the live one stays with the app.
///
/// Snapshots are serialized to JSON bytes via `serde`, so any
/// `Serialize`/`DeserializeOwned` value can be preserved. Type safety is
/// enforced on read: [`restore`](Self::restore) returns `None` if the stored
/// bytes cannot be deserialized into the requested type.
#[derive(Clone, Default)]
pub struct ReloadStateStore {
    entries: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl ReloadStateStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Serialize `value` and store it under `key`, overwriting any previous
    /// snapshot for the same key.
    ///
    /// Returns an error only if `value` fails to serialize.
    pub fn snapshot<T: Serialize>(&self, key: impl Into<String>, value: &T) -> Result<(), serde_json::Error> {
        let bytes = serde_json::to_vec(value)?;
        self.entries.write().insert(key.into(), bytes);
        Ok(())
    }

    /// Read back the snapshot stored under `key`, deserialized as `T`.
    ///
    /// Returns `None` when there is no entry for `key`, or when the stored
    /// bytes do not deserialize into `T` (e.g. the type changed across the
    /// reload). The entry is left in place either way; call [`take`](Self::take)
    /// to remove it on read.
    pub fn restore<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let entries = self.entries.read();
        let bytes = entries.get(key)?;
        serde_json::from_slice(bytes).ok()
    }

    /// Like [`restore`](Self::restore), but removes the entry from the store on
    /// a successful read. The entry is preserved if deserialization fails, so a
    /// type mismatch never silently discards the snapshot.
    pub fn take<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        // Read under the write lock so the check-then-remove is atomic.
        let mut entries = self.entries.write();
        let value: T = entries.get(key).and_then(|bytes| serde_json::from_slice(bytes).ok())?;
        entries.remove(key);
        Some(value)
    }

    /// Returns `true` if a snapshot exists for `key` (regardless of its type).
    pub fn contains(&self, key: &str) -> bool {
        self.entries.read().contains_key(key)
    }

    /// Remove the snapshot stored under `key`, if any.
    pub fn remove(&self, key: &str) {
        self.entries.write().remove(key);
    }

    /// Remove all snapshots.
    pub fn clear(&self) {
        self.entries.write().clear();
    }

    /// Number of stored snapshots.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Returns `true` if no snapshots are stored.
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }
}

impl std::fmt::Debug for ReloadStateStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReloadStateStore").field("len", &self.len()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Counter {
        count: i32,
        label: String,
    }

    #[test]
    fn snapshot_restore_round_trips() {
        let store = ReloadStateStore::new();
        let value = Counter {
            count: 7,
            label: "rows".into(),
        };
        store.snapshot("counter", &value).unwrap();

        let restored: Counter = store.restore("counter").unwrap();
        assert_eq!(restored, value);
        // restore leaves the entry in place.
        assert!(store.contains("counter"));
    }

    #[test]
    fn restore_returns_none_for_missing_key() {
        let store = ReloadStateStore::new();
        assert!(store.restore::<i32>("missing").is_none());
    }

    #[test]
    fn restore_returns_none_on_type_mismatch() {
        let store = ReloadStateStore::new();
        store.snapshot("counter", &"a string".to_string()).unwrap();

        // The stored bytes are a JSON string; deserializing as a struct fails.
        assert!(store.restore::<Counter>("counter").is_none());
        // The entry is untouched so a correctly typed read still works.
        assert_eq!(store.restore::<String>("counter").as_deref(), Some("a string"));
    }

    #[test]
    fn snapshot_overwrites_existing_entry() {
        let store = ReloadStateStore::new();
        store.snapshot("counter", &1i32).unwrap();
        store.snapshot("counter", &42i32).unwrap();

        assert_eq!(store.restore::<i32>("counter"), Some(42));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn take_removes_entry_on_success() {
        let store = ReloadStateStore::new();
        store.snapshot("counter", &9i32).unwrap();

        assert_eq!(store.take::<i32>("counter"), Some(9));
        assert!(!store.contains("counter"));
        assert!(store.take::<i32>("counter").is_none());
    }

    #[test]
    fn take_preserves_entry_on_type_mismatch() {
        let store = ReloadStateStore::new();
        store.snapshot("counter", &"text".to_string()).unwrap();

        assert!(store.take::<i32>("counter").is_none());
        // mismatch must not discard the snapshot.
        assert!(store.contains("counter"));
        assert_eq!(store.restore::<String>("counter").as_deref(), Some("text"));
    }

    #[test]
    fn remove_and_clear() {
        let store = ReloadStateStore::new();
        store.snapshot("a", &1i32).unwrap();
        store.snapshot("b", &2i32).unwrap();

        store.remove("a");
        assert!(!store.contains("a"));
        assert!(store.contains("b"));
        assert_eq!(store.len(), 1);

        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn clones_share_state() {
        let store = ReloadStateStore::new();
        let clone = store.clone();
        store.snapshot("shared", &123i32).unwrap();

        assert_eq!(clone.restore::<i32>("shared"), Some(123));
    }

    #[test]
    fn preserves_state_across_simulated_reload() {
        // Models the host workflow: snapshot key Cage values, run the reload,
        // then restore them into the rebuilt tree.
        let store = ReloadStateStore::new();

        // before reload: live Cage holds 41.
        let live_value = 41i32;
        store.snapshot("counter", &live_value).unwrap();

        // …function reload swaps closures and rebuilds the tree, so the new
        // Cage starts at its default…
        let mut rebuilt_value = 0i32;

        // after reload: restore the preserved value back into the new Cage.
        if let Some(v) = store.restore::<i32>("counter") {
            rebuilt_value = v;
        }
        assert_eq!(rebuilt_value, 41);
    }
}
