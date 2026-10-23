use glory_core::{AccessError, Runtime};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

struct Row {
    name: String,
    count: usize,
    payload: Vec<u8>,
}
fn row(name: &str) -> Row {
    Row {
        name: name.into(),
        count: 0,
        payload: vec![9; 1024],
    }
}

#[test]
fn vec_positions_follow_reallocation_reorder_removal_and_recovery_without_cloning_rows() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let rows = owner.cage(vec![row("a"), row("b")]);
    let first = rows.item(0);
    let second = rows.item(1);
    let name = first.field("name", |r| &r.name, |r| &mut r.name);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let out = seen.clone();
    owner.binding(move || out.borrow_mut().push(name.try_get()));
    runtime.flush().unwrap();
    let payload = first.with(|r| r.payload.as_ptr());
    first.revise(|r| r.count += 1);
    runtime.flush().unwrap();
    assert_eq!(second.with(|r| r.count), 0);
    rows.revise(|rows| {
        rows.reserve(10_000);
        rows.swap(0, 1);
    });
    runtime.flush().unwrap();
    assert_eq!(name.get(), "b");
    assert_eq!(second.with(|r| r.payload.as_ptr()), payload);
    rows.revise(Vec::clear);
    runtime.flush().unwrap();
    assert_eq!(name.try_get(), Err(AccessError::MissingField));
    let stats = runtime.stats();
    let called = Cell::new(false);
    assert_eq!(
        first.try_revise(|r| {
            called.set(true);
            r.count = 9;
        }),
        Err(AccessError::MissingField)
    );
    assert!(!called.get());
    assert_eq!(runtime.stats(), stats);
    rows.revise(|rows| rows.push(row("replacement")));
    runtime.flush().unwrap();
    assert_eq!(name.get(), "replacement");
    assert_eq!(seen.borrow().last(), Some(&Ok("replacement".into())));
    assert_eq!(runtime.stats().entry_keys, 0);
    drop(owner);
    assert_eq!(runtime.stats().fields, 0);
}

#[test]
fn keyed_rows_notify_only_related_paths_and_missing_reads_remain_subscribed() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let rows = owner.cage(BTreeMap::from([(1, row("one")), (2, row("two"))]));
    let one = rows.entry(1);
    let two = rows.entry(2);
    let name = one.field("name", |r| &r.name, |r| &mut r.name);
    let count = one.field("count", |r| &r.count, |r| &mut r.count);
    let runs = Rc::new(RefCell::new([0; 5]));
    for i in 0..5 {
        let runs = runs.clone();
        owner.binding(move || {
            match i {
                0 => {
                    rows.with(|r| r.len());
                }
                1 => {
                    one.try_with(|r| r.count).ok();
                }
                2 => {
                    two.try_with(|r| r.count).ok();
                }
                3 => {
                    name.try_get().ok();
                }
                _ => {
                    count.try_get().ok();
                }
            }
            runs.borrow_mut()[i] += 1;
        });
    }
    runtime.flush().unwrap();
    name.revise(|s| s.push('!'));
    runtime.flush().unwrap();
    assert_eq!(*runs.borrow(), [2, 2, 1, 2, 1]);
    rows.revise(|r| {
        r.remove(&1);
    });
    runtime.flush().unwrap();
    assert_eq!(one.try_with(|r| r.count), Err(AccessError::MissingField));
    assert_eq!(name.try_get(), Err(AccessError::MissingField));
    rows.revise(|r| {
        r.insert(1, row("new"));
    });
    runtime.flush().unwrap();
    assert_eq!(name.get(), "new");
    assert_eq!(*runs.borrow(), [4, 4, 3, 4, 3]);
    assert_eq!(
        (runtime.stats().entry_keys, runtime.stats().entry_buckets),
        (2, 2)
    );
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct Key(u64); // Deliberately not Clone; all hashes collide.
impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0u8.hash(state);
    }
}

#[test]
fn collisions_use_full_keys_across_owners_and_hashmap_rehash_never_caches_addresses() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let rows = owner.cage(HashMap::from([(Key(1), row("one")), (Key(2), row("two"))]));
    let one = rows.entry(Key(1));
    let two = rows.entry(Key(2));
    let child = runtime.owner();
    let alias = child.enter(|| rows.entry(Key(1)));
    assert_eq!(
        (runtime.stats().entry_keys, runtime.stats().entry_buckets),
        (2, 1)
    );
    let seen = Rc::new(RefCell::new(Vec::new()));
    let out = seen.clone();
    owner.binding(move || out.borrow_mut().push(one.try_with(|r| r.count)));
    runtime.flush().unwrap();
    alias.revise(|r| r.count = 3);
    runtime.flush().unwrap();
    assert_eq!(seen.borrow().last(), Some(&Ok(3)));
    assert_eq!(two.with(|r| r.count), 0);
    rows.revise(|r| r.reserve(10000));
    runtime.flush().unwrap();
    assert_eq!(one.with(|r| r.count), 3);
    drop(child);
    assert_eq!(runtime.stats().entry_keys, 2);
    let foreign = runtime.owner();
    let key = foreign.enter(|| rows.entry(Key(1)));
    drop(owner);
    assert_eq!(key.try_with(|r| r.count), Err(AccessError::Disposed));
    assert_eq!(
        (
            runtime.stats().fields,
            runtime.stats().entry_keys,
            runtime.stats().entry_buckets
        ),
        (0, 0, 0)
    );
}

#[test]
fn absent_nested_collection_paths_recycle_keys_without_any_visited_history() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let rows = owner.cage(BTreeMap::<Key, Vec<Row>>::new());
    for i in 0..6000 {
        let row_owner = runtime.owner();
        let slot = row_owner.enter(|| rows.entry(Key(i)).item(10_000_000));
        assert_eq!(slot.try_with(|r| r.count), Err(AccessError::MissingField));
        assert_eq!(
            (
                runtime.stats().fields,
                runtime.stats().entry_keys,
                runtime.stats().entry_buckets
            ),
            (2, 1, 1)
        );
        drop(row_owner);
        assert_eq!(
            (
                runtime.stats().fields,
                runtime.stats().entry_keys,
                runtime.stats().entry_buckets
            ),
            (0, 0, 0)
        );
        assert!(runtime.stats().field_slots <= 2 && runtime.stats().node_slots <= 3);
    }
    // A path can be subscribed before data exists, then recover from root writes.
    let slot = rows.entry(Key(9)).item(0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let out = seen.clone();
    owner.binding(move || out.borrow_mut().push(slot.try_with(|r| r.name.clone())));
    runtime.flush().unwrap();
    rows.revise(|r| {
        r.insert(Key(9), vec![row("loaded")]);
    });
    runtime.flush().unwrap();
    assert_eq!(
        &*seen.borrow(),
        &[Err(AccessError::MissingField), Ok("loaded".into())]
    );
}

#[test]
fn collection_and_inline_siblings_cannot_silently_alias_the_same_container() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let rows = owner.cage(vec![0usize]);
    let all = rows.field("all", |r| r, |r| r);
    let before = runtime.stats();
    assert!(matches!(
        rows.try_item(0),
        Err(AccessError::InvalidField(_))
    ));
    assert_eq!(runtime.stats(), before);
    all.item(0).revise(|n| *n = 1);
    assert_eq!(rows.with(|r| r[0]), 1);
    let map = owner.cage(BTreeMap::from([(1, 0usize)]));
    let _entry = map.entry(1);
    let before = runtime.stats();
    assert!(matches!(
        map.try_field("all", |r| r, |r| r),
        Err(AccessError::InvalidField(_))
    ));
    assert_eq!(runtime.stats(), before);
    let map2 = owner.cage(BTreeMap::<u64, usize>::new());
    let all = map2.field("all", |r| r, |r| r);
    let before = runtime.stats();
    assert!(matches!(
        map2.try_entry(8),
        Err(AccessError::InvalidField(_))
    ));
    assert_eq!(runtime.stats(), before);
    assert_eq!(all.entry(8).try_get(), Err(AccessError::MissingField));
}

#[test]
fn collection_registration_during_root_or_entry_mutation_is_rejected_before_allocation() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let rows = owner.cage(vec![1usize]);
    rows.revise(|_| assert!(matches!(rows.try_item(0), Err(AccessError::Borrowed))));
    assert_eq!(runtime.stats().fields, 0);
    let map = owner.cage(BTreeMap::from([(1, 0usize)]));
    let first = map.entry(1);
    first.revise(|_| assert!(matches!(map.try_entry(2), Err(AccessError::Borrowed))));
    assert_eq!(runtime.stats().entry_keys, 1);
}

#[test]
fn key_hash_equality_and_drop_run_without_graph_or_registry_borrows() {
    struct CallbackKey {
        id: u64,
        runtime: Rc<Runtime>,
        drops: glory_core::Cage<usize>,
    }
    impl CallbackKey {
        fn probe(&self) {
            self.runtime.stats();
            assert!(self.drops.is_alive());
        }
    }
    impl Hash for CallbackKey {
        fn hash<H: Hasher>(&self, h: &mut H) {
            self.probe();
            0u8.hash(h);
        }
    }
    impl PartialEq for CallbackKey {
        fn eq(&self, other: &Self) -> bool {
            self.probe();
            self.id == other.id
        }
    }
    impl Eq for CallbackKey {}
    impl Drop for CallbackKey {
        fn drop(&mut self) {
            self.drops.revise(|n| *n += 1);
        }
    }
    let runtime = Rc::new(Runtime::new());
    let audit = runtime.owner();
    let drops = audit.cage(0);
    let owner = runtime.owner();
    let map = owner.cage(HashMap::<CallbackKey, usize>::new());
    let key = |id| CallbackKey {
        id,
        runtime: runtime.clone(),
        drops,
    };
    let _a = map.entry(key(1));
    let _b = map.entry(key(1));
    let _c = map.entry(key(2));
    assert_eq!(drops.get(), 1); // Duplicate key rejected outside the borrows.
    drop(owner);
    assert_eq!(drops.get(), 3);
    assert_eq!(
        (runtime.stats().entry_keys, runtime.stats().entry_buckets),
        (0, 0)
    );
}

#[test]
fn mixed_removal_reinsertion_batch_reads_and_position_changes_match_plain_data() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let rows = owner.cage(BTreeMap::<u64, Vec<usize>>::new());
    let mut plain = BTreeMap::<u64, Vec<usize>>::new();
    let slots: [_; 3] = std::array::from_fn(|i| rows.entry(i as u64).item(0));
    let values = owner.bond(move || slots.map(|slot| slot.try_get()));
    let observed = Rc::new(RefCell::new(Vec::new()));
    let out = observed.clone();
    owner.binding(move || out.borrow_mut().push(values.get()));
    runtime.flush().unwrap();
    for step in 0..1000 {
        runtime
            .batch(|| {
                let key = (step % 3) as u64;
                if step % 5 == 0 {
                    plain.remove(&key);
                    rows.revise(|m| {
                        m.remove(&key);
                    });
                } else {
                    plain.insert(key, vec![step]);
                    rows.revise(|m| {
                        m.insert(key, vec![step]);
                    });
                }
                if let Some(v) = plain.get_mut(&key) {
                    v[0] += 1;
                    slots[key as usize].revise(|v| *v += 1);
                }
                let expected = std::array::from_fn(|i| {
                    plain
                        .get(&(i as u64))
                        .map(|v| v[0])
                        .ok_or(AccessError::MissingField)
                });
                assert_eq!(values.get(), expected);
            })
            .unwrap();
        assert_eq!(observed.borrow().last(), Some(&values.get()));
        assert_eq!(runtime.stats().entry_keys, 3);
    }
}

#[test]
fn panicking_keys_restore_purity_and_do_not_leave_failed_registration_metadata() {
    struct PanicKey(u64, Rc<Cell<u8>>);
    impl Hash for PanicKey {
        fn hash<H: Hasher>(&self, h: &mut H) {
            assert_ne!(self.1.get(), 1, "hash failed");
            0u8.hash(h);
        }
    }
    impl PartialEq for PanicKey {
        fn eq(&self, r: &Self) -> bool {
            assert_ne!(self.1.get(), 2, "eq failed");
            self.0 == r.0
        }
    }
    impl Eq for PanicKey {}
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let mode = Rc::new(Cell::new(1));
    let map = owner.cage(HashMap::<PanicKey, usize>::new());
    let key = || PanicKey(1, mode.clone());
    let before = runtime.stats();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| map.entry(key()))).is_err());
    assert_eq!(runtime.stats(), before);
    mode.set(0);
    let item = map.entry(key());
    let before = runtime.stats();
    mode.set(2);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| map.entry(key()))).is_err());
    assert_eq!(runtime.stats(), before);
    mode.set(0);
    map.revise(|m| {
        m.insert(key(), 4);
    });
    assert_eq!(item.get(), 4);
    drop(owner);
    assert_eq!(
        (runtime.stats().entry_keys, runtime.stats().entry_buckets),
        (0, 0)
    );
}

#[test]
fn disposing_the_source_during_custom_key_hashing_cannot_register_an_orphan() {
    struct DisposeKey(Rc<RefCell<Option<glory_core::Owner>>>);
    impl Hash for DisposeKey {
        fn hash<H: Hasher>(&self, h: &mut H) {
            drop(self.0.borrow_mut().take());
            0u8.hash(h);
        }
    }
    impl PartialEq for DisposeKey {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }
    impl Eq for DisposeKey {}
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let map = owner.cage(HashMap::<DisposeKey, usize>::new());
    assert!(matches!(
        map.try_entry(DisposeKey(Rc::new(RefCell::new(Some(owner))))),
        Err(AccessError::Disposed)
    ));
    assert_eq!(
        (
            runtime.stats().fields,
            runtime.stats().entry_keys,
            runtime.stats().entry_buckets
        ),
        (0, 0, 0)
    );
}

#[test]
fn indexed_hashmap_reads_and_writes_do_not_visit_the_source_or_other_live_keys() {
    struct CountKey(usize, Rc<Cell<usize>>);
    impl Hash for CountKey {
        fn hash<H: Hasher>(&self, h: &mut H) {
            self.1.set(self.1.get() + 1);
            self.0.hash(h);
        }
    }
    impl PartialEq for CountKey {
        fn eq(&self, r: &Self) -> bool {
            self.1.set(self.1.get() + 1);
            self.0 == r.0
        }
    }
    impl Eq for CountKey {}
    struct Value(usize); // No Clone.
    for count in [1024, 131072] {
        let runtime = Runtime::new();
        let owner = runtime.owner();
        let calls = Rc::new(Cell::new(0));
        let map = owner.cage(
            (0..count)
                .map(|i| (CountKey(i, calls.clone()), Value(i)))
                .collect::<HashMap<_, _>>(),
        );
        let keys: Vec<_> = (0..32)
            .map(|i| map.entry(CountKey(i, calls.clone())))
            .collect();
        let runs = Rc::new(Cell::new(0));
        for key in &keys {
            let key = *key;
            let runs = runs.clone();
            owner.binding(move || {
                key.with(|v| v.0);
                runs.set(runs.get() + 1);
            });
        }
        runtime.flush().unwrap();
        calls.set(0);
        let before = runs.get();
        keys[0].revise(|v| v.0 += 1);
        runtime.flush().unwrap();
        assert_eq!(runs.get() - before, 1);
        assert_eq!(keys[0].with(|v| v.0), 1);
        assert!(
            calls.get() <= 20,
            "indexed access performed {} key operations in a {count}-entry map",
            calls.get()
        );
        assert_eq!(runtime.stats().entry_keys, 32);
    }
}
