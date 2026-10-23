use glory_core::{AccessError, Cage, Runtime};
use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap},
    hash::{Hash, Hasher},
    rc::Rc,
};

fn observe<T: 'static>(owner: &glory_core::Owner, cage: Cage<T>) -> Rc<Cell<usize>> {
    let calls = Rc::new(Cell::new(0));
    let out = calls.clone();
    owner.binding(move || {
        let _ = cage.try_with(|_| ());
        out.set(out.get() + 1);
    });
    calls
}

#[test]
fn vec_edits_touch_only_changed_positions_including_missing_and_descendant_paths() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let model = owner.cage((vec![(10usize,), (20,)], 0usize));
    let rows = model.field("rows", |m| &m.0, |m| &mut m.0);
    let other = model.field("other", |m| &m.1, |m| &mut m.1);
    let positions: [_; 4] = std::array::from_fn(|i| rows.item(i));
    let first = positions[0].field("count", |v| &v.0, |v| &mut v.0);
    let calls = positions.map(|p| observe(&owner, p));
    let whole = observe(&owner, model);
    let collection = observe(&owner, rows);
    let sibling = observe(&owner, other);
    let child = observe(&owner, first);
    runtime.flush().unwrap();
    let counts = || calls.each_ref().map(|n| n.get());
    rows.push((30,));
    runtime.flush().unwrap();
    assert_eq!(counts(), [1, 1, 2, 1]);
    assert_eq!(child.get(), 1);
    rows.insert(1, (15,));
    runtime.flush().unwrap();
    assert_eq!(counts(), [1, 2, 3, 2]);
    assert_eq!(child.get(), 1);
    assert_eq!(rows.remove(2), (20,));
    runtime.flush().unwrap();
    assert_eq!(counts(), [1, 2, 4, 3]);
    assert_eq!(rows.pop(), Some((30,)));
    runtime.flush().unwrap();
    assert_eq!(counts(), [1, 2, 5, 3]);
    rows.truncate(1);
    runtime.flush().unwrap();
    assert_eq!(counts(), [1, 3, 5, 3]);
    rows.clear();
    runtime.flush().unwrap();
    assert_eq!(counts(), [2, 3, 5, 3]);
    assert_eq!(child.get(), 2);
    assert_eq!(first.try_get(), Err(AccessError::MissingField));
    rows.push((99,));
    runtime.flush().unwrap();
    assert_eq!(first.get(), 99);
    assert_eq!(child.get(), 3);
    assert_eq!((whole.get(), collection.get(), sibling.get()), (8, 8, 1));
}

#[test]
fn noops_invalid_indices_missing_parents_and_pure_edits_do_not_publish() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let rows = owner.cage(Vec::<usize>::new());
    let future = rows.item(10_000_000);
    let map = owner.cage(BTreeMap::<usize, Vec<usize>>::new());
    let absent = map.entry(7);
    let _ = observe(&owner, future);
    let _ = observe(&owner, absent);
    runtime.flush().unwrap();
    let before = runtime.stats();
    assert_eq!(rows.try_pop(), Ok(None));
    rows.truncate(100);
    rows.clear();
    assert_eq!(rows.try_insert(1, 2), Err(AccessError::OutOfBounds));
    assert_eq!(rows.try_remove(0), Err(AccessError::OutOfBounds));
    assert_eq!(map.try_remove(&7), Ok(None));
    assert_eq!(absent.try_push(1), Err(AccessError::MissingField));
    assert_eq!(runtime.stats(), before);
    let pure = owner.bond(move || rows.try_push(1));
    assert_eq!(pure.get(), Err(AccessError::WriteDuringCompute));
    assert!(rows.with(Vec::is_empty));
    rows.with(|_| assert_eq!(rows.try_push(1), Err(AccessError::Borrowed)));
    drop(owner);
    assert_eq!(rows.try_push(1), Err(AccessError::Disposed));
}

struct Row {
    count: usize,
    payload: Vec<u8>,
}
#[test]
fn map_insert_replace_remove_and_recovery_leave_other_keys_and_parent_fields_clean() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let model = owner.cage((
        BTreeMap::from([
            (
                1usize,
                Row {
                    count: 1,
                    payload: vec![9; 1024],
                },
            ),
            (
                2,
                Row {
                    count: 2,
                    payload: vec![8; 1024],
                },
            ),
        ]),
        0usize,
    ));
    let rows = model.field("rows", |m| &m.0, |m| &mut m.0);
    let background = model.field("background", |m| &m.1, |m| &mut m.1);
    let one = rows.entry(1);
    let two = rows.entry(2);
    let missing = rows.entry(3);
    let count = one.field("count", |r| &r.count, |r| &mut r.count);
    let calls = [
        observe(&owner, one),
        observe(&owner, two),
        observe(&owner, missing),
        observe(&owner, background),
        observe(&owner, count),
    ];
    let root = observe(&owner, model);
    let map = observe(&owner, rows);
    runtime.flush().unwrap();
    let pointer = two.with(|r| r.payload.as_ptr());
    let snapshot = || calls.each_ref().map(|n| n.get());
    assert_eq!(
        rows.insert(
            1,
            Row {
                count: 10,
                payload: vec![]
            }
        )
        .unwrap()
        .count,
        1
    );
    runtime.flush().unwrap();
    assert_eq!(snapshot(), [2, 1, 1, 1, 2]);
    assert_eq!(rows.remove(&1).unwrap().count, 10);
    runtime.flush().unwrap();
    assert_eq!(count.try_get(), Err(AccessError::MissingField));
    assert_eq!(snapshot(), [3, 1, 1, 1, 3]);
    assert!(
        rows.insert(
            1,
            Row {
                count: 20,
                payload: vec![]
            }
        )
        .is_none()
    );
    runtime.flush().unwrap();
    assert_eq!(count.get(), 20);
    rows.insert(
        3,
        Row {
            count: 30,
            payload: vec![],
        },
    );
    runtime.flush().unwrap();
    assert_eq!(snapshot(), [4, 1, 2, 1, 4]);
    rows.insert(
        999,
        Row {
            count: 99,
            payload: vec![],
        },
    );
    runtime.flush().unwrap();
    assert_eq!(snapshot(), [4, 1, 2, 1, 4]);
    assert_eq!((root.get(), map.get()), (6, 6));
    assert_eq!(two.with(|r| r.payload.as_ptr()), pointer);
    assert_eq!(runtime.stats().entry_keys, 3); // Inserting unobserved 999 creates no path.
}

#[test]
fn nested_map_vec_edits_and_mid_batch_reads_match_plain_data_for_a_thousand_actions() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let rows = owner.cage(HashMap::<usize, Vec<usize>>::new());
    let entries: [_; 3] = std::array::from_fn(|k| rows.entry(k));
    let cells: [[_; 4]; 3] = std::array::from_fn(|k| std::array::from_fn(|i| entries[k].item(i)));
    let all = owner.bond(move || cells.map(|r| r.map(|c| c.try_get())));
    let mut plain = HashMap::<usize, Vec<usize>>::new();
    let calls = observe(&owner, rows);
    runtime.flush().unwrap();
    for step in 0..1000usize {
        runtime
            .batch(|| {
                let key = step % 3;
                match step % 7 {
                    0 => {
                        assert_eq!(rows.insert(key, vec![step]), plain.insert(key, vec![step]));
                    }
                    1 => {
                        assert_eq!(rows.remove(&key), plain.remove(&key));
                    }
                    _ => {
                        if let Some(v) = plain.get_mut(&key) {
                            let slot = entries[key];
                            if step % 2 == 0 {
                                v.push(step);
                                slot.push(step);
                            } else {
                                assert_eq!(slot.pop(), v.pop());
                            }
                        }
                    }
                }
                let expected = std::array::from_fn(|k| {
                    std::array::from_fn(|i| {
                        plain
                            .get(&k)
                            .and_then(|v| v.get(i))
                            .copied()
                            .ok_or(AccessError::MissingField)
                    })
                });
                assert_eq!(all.get(), expected);
            })
            .unwrap();
        assert_eq!(runtime.stats().entry_keys, 3);
        assert_eq!(runtime.stats().fields, 15);
        assert!(runtime.stats().node_slots < 40);
    }
    assert!(calls.get() > 1);
    drop(owner);
    assert_eq!((runtime.stats().fields, runtime.stats().entry_keys), (0, 0));
}

struct Key {
    id: usize,
    runtime: Rc<Runtime>,
    dropped: Cage<usize>,
}
impl Hash for Key {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.runtime.stats();
        0u8.hash(h);
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.runtime.stats();
        self.id == other.id
    }
}
impl Eq for Key {}
impl Drop for Key {
    fn drop(&mut self) {
        self.dropped.revise(|n| *n += 1);
    }
}
#[test]
fn colliding_non_clone_keys_and_duplicate_removed_key_destructors_run_outside_runtime_borrows() {
    let runtime = Rc::new(Runtime::new());
    let audit = runtime.owner();
    let dropped = audit.cage(0usize);
    let owner = runtime.owner();
    let map = owner.cage(HashMap::<Key, usize>::new());
    let key = |id| Key {
        id,
        runtime: runtime.clone(),
        dropped,
    };
    let a = map.entry(key(1));
    let b = map.entry(key(2));
    let calls = [observe(&owner, a), observe(&owner, b)];
    runtime.flush().unwrap();
    assert_eq!(map.insert(key(1), 10), None);
    runtime.flush().unwrap();
    assert_eq!([calls[0].get(), calls[1].get()], [2, 1]);
    assert_eq!(map.insert(key(1), 20), Some(10));
    runtime.flush().unwrap();
    assert_eq!([calls[0].get(), calls[1].get()], [3, 1]);
    assert_eq!(dropped.get(), 1); // Duplicate incoming key can write another Cage.
    assert_eq!(map.remove(&key(1)), Some(20));
    runtime.flush().unwrap();
    assert_eq!([calls[0].get(), calls[1].get()], [4, 1]);
    assert_eq!(dropped.get(), 3); // Stored key and borrowed lookup temporary.
    map.insert(key(2), 30);
    runtime.flush().unwrap();
    assert_eq!([calls[0].get(), calls[1].get()], [4, 2]);
    drop(owner);
    assert_eq!(dropped.get(), 6);
    assert_eq!(
        (runtime.stats().entry_keys, runtime.stats().entry_buckets),
        (0, 0)
    );
}

#[test]
fn unobserved_mutations_and_owner_churn_never_build_a_visited_key_index() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let map = owner.cage(HashMap::<usize, usize>::new());
    for key in 0..1000 {
        map.insert(key, key);
        map.remove(&key);
    }
    assert_eq!(
        (
            runtime.stats().fields,
            runtime.stats().entry_keys,
            runtime.stats().field_target_caches
        ),
        (0, 0, 0)
    );
    for key in 0..1000 {
        let row = runtime.owner();
        row.enter(|| {
            let _ = map.entry(key);
        });
        map.insert(key, key);
        drop(row);
        map.remove(&key);
        assert_eq!(
            (
                runtime.stats().fields,
                runtime.stats().entry_keys,
                runtime.stats().field_target_caches
            ),
            (0, 0, 0)
        );
    }
}

#[test]
#[allow(
    clippy::mutable_key_type,
    reason = "Immutable numeric identity; Cell counts key operations only"
)]
fn hash_edits_scale_with_live_paths_not_source_size_and_insert_rehash_preserves_other_paths() {
    struct CountKey(usize, Rc<Cell<usize>>);
    impl Hash for CountKey {
        fn hash<H: Hasher>(&self, h: &mut H) {
            self.1.set(self.1.get() + 1);
            self.0.hash(h);
        }
    }
    impl PartialEq for CountKey {
        fn eq(&self, o: &Self) -> bool {
            self.1.set(self.1.get() + 1);
            self.0 == o.0
        }
    }
    impl Eq for CountKey {}
    for size in [1024, 131072] {
        let runtime = Runtime::new();
        let owner = runtime.owner();
        let probes = Rc::new(Cell::new(0));
        let data: HashMap<_, _> = (0..size)
            .map(|n| (CountKey(n, probes.clone()), n))
            .collect();
        let rows = owner.cage(data);
        let calls = Rc::new(Cell::new(0));
        for index in 0..256 {
            let item = rows.entry(CountKey(index, probes.clone()));
            let out = calls.clone();
            owner.binding(move || {
                item.with(|v| std::hint::black_box(*v));
                out.set(out.get() + 1);
            });
        }
        runtime.flush().unwrap();
        let initial = calls.get();
        probes.set(0);
        assert_eq!(rows.insert(CountKey(0, probes.clone()), size), Some(0));
        runtime.flush().unwrap();
        assert_eq!(calls.get() - initial, 1);
        assert!(probes.get() <= 32, "size={size}: {} key ops", probes.get());
        assert_eq!(runtime.stats().entry_keys, 256);
    }
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let rows = owner.cage(HashMap::from([
        (
            0usize,
            Row {
                count: 0,
                payload: vec![9; 1024 * 1024],
            },
        ),
        (
            1,
            Row {
                count: 1,
                payload: vec![],
            },
        ),
    ]));
    let zero = rows.entry(0);
    let one = rows.entry(1);
    let calls = [observe(&owner, zero), observe(&owner, one)];
    runtime.flush().unwrap();
    let pointer = zero.with(|r| r.payload.as_ptr());
    let capacity = rows.with(HashMap::capacity);
    for index in 2..1000 {
        rows.insert(
            index,
            Row {
                count: index,
                payload: vec![],
            },
        );
    }
    runtime.flush().unwrap();
    assert_eq!([calls[0].get(), calls[1].get()], [1, 1]);
    assert!(rows.with(HashMap::capacity) > capacity);
    assert_eq!(zero.with(|r| r.payload.as_ptr()), pointer);
    assert_eq!((runtime.stats().fields, runtime.stats().entry_keys), (2, 2));
}

#[test]
fn truncation_panic_publishes_actual_shorter_vec_and_unblocks_future_edits() {
    struct Value {
        n: usize,
        armed: Rc<Cell<bool>>,
    }
    impl Drop for Value {
        fn drop(&mut self) {
            if self.n == 1 && self.armed.replace(false) {
                panic!("one destructor failed");
            }
        }
    }
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let armed = Rc::new(Cell::new(true));
    let rows = owner.cage(
        (0..3)
            .map(|n| Value {
                n,
                armed: armed.clone(),
            })
            .collect::<Vec<_>>(),
    );
    let last = rows.item(2);
    let calls = observe(&owner, last);
    runtime.flush().unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rows.truncate(1))).is_err());
    runtime.flush().unwrap();
    assert_eq!(rows.with(Vec::len), 1);
    assert_eq!(last.try_with(|v| v.n), Err(AccessError::MissingField));
    assert_eq!(calls.get(), 2);
    rows.push(Value {
        n: 3,
        armed: armed.clone(),
    });
    rows.push(Value { n: 4, armed });
    runtime.flush().unwrap();
    assert_eq!(last.with(|v| v.n), 4);
}
