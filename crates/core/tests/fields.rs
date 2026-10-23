use glory_core::{AccessError, Runtime};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn repeatedly_selecting_live_paths_in_one_owner_keeps_nodes_and_notification_targets_bounded() {
    use std::collections::BTreeMap;
    let runtime = Runtime::new();
    let root = runtime.owner();
    let source = root.cage((vec![1usize], BTreeMap::from([(7usize, 2usize)])));
    let row = runtime.owner();
    let (position, missing, entry) = row.enter(|| {
        let vector = source.field("vector", |v| &v.0, |v| &mut v.0);
        let map = source.field("map", |v| &v.1, |v| &mut v.1);
        (vector.item(0), vector.item(100_000_000), map.entry(7))
    });
    source.revise(|value| value.0[0] += 1);
    let before = runtime.stats();
    for _ in 0..2000 {
        row.enter(|| {
            let vector = source.field("vector", |v| &v.0, |v| &mut v.0);
            let map = source.field("map", |v| &v.1, |v| &mut v.1);
            assert_eq!(vector.item(0).get(), 2);
            assert_eq!(
                vector.item(100_000_000).try_get(),
                Err(AccessError::MissingField)
            );
            assert_eq!(map.entry(7).get(), 2);
        });
        let after = runtime.stats();
        assert_eq!(
            (after.nodes, after.fields, after.entry_keys),
            (before.nodes, before.fields, before.entry_keys)
        );
        assert_eq!(
            (after.field_target_caches, after.field_target_keys),
            (before.field_target_caches, before.field_target_keys)
        );
    }
    source.revise(|value| value.0[0] += 1);
    assert_eq!(
        runtime.stats().field_target_builds,
        before.field_target_builds
    );
    assert_eq!(
        runtime.stats().field_target_hits,
        before.field_target_hits + 1
    );
    assert_eq!(position.get(), 3);
    drop(row);
    assert!(!position.is_alive() && !missing.is_alive() && !entry.is_alive());
    assert_eq!(
        (
            runtime.stats().nodes,
            runtime.stats().fields,
            runtime.stats().entry_keys
        ),
        (1, 0, 0)
    );
}

#[test]
fn one_logical_field_has_one_alias_per_live_owner_with_independent_retirement() {
    let runtime = Runtime::new();
    let root = runtime.owner();
    let source = root.cage((3usize,));
    let mut handles = Vec::new();
    let mut owners = Vec::new();
    for _ in 0..64 {
        let owner = runtime.owner();
        for _ in 0..64 {
            handles.push(owner.enter(|| source.field("value", |v| &v.0, |v| &mut v.0)));
        }
        owners.push(owner);
    }
    assert_eq!((runtime.stats().nodes, runtime.stats().fields), (65, 1));
    for (index, owner) in owners.into_iter().enumerate() {
        owner.dispose();
        assert!(
            handles[index * 64..(index + 1) * 64]
                .iter()
                .all(|handle| !handle.is_alive())
        );
        assert_eq!(runtime.stats().nodes, 64 - index);
        if index < 63 {
            assert_eq!(handles[(index + 1) * 64].get(), 3);
        }
    }
    assert_eq!(runtime.stats().fields, 0);
    assert!(source.is_alive());
}

#[test]
fn discarded_duplicate_projection_captures_drop_outside_the_runtime_borrow() {
    struct Capture {
        runtime: Rc<Runtime>,
        dropped: Rc<Cell<usize>>,
    }
    impl Drop for Capture {
        fn drop(&mut self) {
            let _ = self.runtime.stats();
            self.dropped.set(self.dropped.get() + 1);
        }
    }
    let runtime = Rc::new(Runtime::new());
    let owner = runtime.owner();
    let source = owner.cage((0usize,));
    let dropped = Rc::new(Cell::new(0));
    for _ in 0..1000 {
        let read = Capture {
            runtime: runtime.clone(),
            dropped: dropped.clone(),
        };
        let write = Capture {
            runtime: runtime.clone(),
            dropped: dropped.clone(),
        };
        let field = source.field(
            "value",
            move |value| {
                std::hint::black_box(&read);
                &value.0
            },
            move |value| {
                std::hint::black_box(&write);
                &mut value.0
            },
        );
        assert_eq!(field.get(), 0);
        assert_eq!(runtime.stats().nodes, 2);
    }
    assert_eq!(dropped.get(), 1998);
    drop(owner);
    assert_eq!(dropped.get(), 2000);
    assert_eq!(runtime.stats().nodes, 0);
}

struct Pair {
    left: i64,
    right: i64,
}
struct Model {
    pair: Pair,
    side: i64,
    bytes: Vec<u8>,
}
fn model() -> Model {
    Model {
        pair: Pair { left: 1, right: 2 },
        side: 3,
        bytes: vec![9; 1024 * 1024],
    }
}

#[test]
fn nested_field_writes_notify_the_target_and_ancestors_without_unrelated_computations() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let model = owner.cage(model());
    let pair = model.field("pair", |m| &m.pair, |m| &mut m.pair);
    let left = pair.field("left", |p| &p.left, |p| &mut p.left);
    let right = pair.field("right", |p| &p.right, |p| &mut p.right);
    let side = model.field("side", |m| &m.side, |m| &mut m.side);
    let runs = Rc::new(RefCell::new([0; 5]));
    for i in 0..5 {
        let runs = runs.clone();
        owner.binding(move || {
            match i {
                0 => {
                    model.with(|m| m.side);
                }
                1 => {
                    pair.with(|p| p.left);
                }
                2 => {
                    left.get();
                }
                3 => {
                    right.get();
                }
                _ => {
                    side.get();
                }
            }
            runs.borrow_mut()[i] += 1;
        });
    }
    runtime.flush().unwrap();
    let pointer = model.with(|m| m.bytes.as_ptr());
    left.revise(|v| *v = 10);
    runtime.flush().unwrap();
    assert_eq!(*runs.borrow(), [2, 2, 2, 1, 1]);
    pair.revise(|p| {
        p.left = 11;
        p.right = 12;
    });
    runtime.flush().unwrap();
    assert_eq!(*runs.borrow(), [3, 3, 3, 2, 1]);
    side.revise(|v| *v = 13);
    runtime.flush().unwrap();
    assert_eq!(*runs.borrow(), [4, 3, 3, 2, 2]);
    model.revise(|m| {
        m.pair.left = 20;
        m.side = 30;
    });
    runtime.flush().unwrap();
    assert_eq!(*runs.borrow(), [5, 4, 4, 3, 3]);
    assert_eq!((left.get(), right.get(), side.get()), (20, 12, 30));
    assert_eq!(model.with(|m| m.bytes.as_ptr()), pointer);
    assert_eq!(runtime.stats().source_writes, 4);
    assert_eq!(runtime.stats().fields, 4);
}

#[test]
fn a_single_field_reuses_copy_cage_building_blocks_and_batches_do_not_glitch() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let model = owner.cage(model());
    let pair = model.field("pair", |m| &m.pair, |m| &mut m.pair);
    let left = pair.field("left", |p| &p.left, |p| &mut p.left);
    let right = pair.field("right", |p| &p.right, |p| &mut p.right);
    let flag = owner.cage(true);
    let selected = owner
        .bond(move || if flag.get() { left.get() } else { right.get() })
        .with_partial_eq();
    let doubled = owner.bond(move || selected.get() * 2).with_partial_eq();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let out = seen.clone();
    owner.binding(move || out.borrow_mut().push(doubled.get()));
    runtime.flush().unwrap();
    runtime
        .batch(|| {
            left.revise(|v| *v = 5);
            assert_eq!(doubled.get(), 10);
            right.revise(|v| *v = 6);
            flag.revise(|v| *v = false);
            assert_eq!(doubled.get(), 12);
            model.revise(|m| m.pair.right = 7);
            assert_eq!(doubled.get(), 14);
            right.revise(|v| *v = 8);
        })
        .unwrap();
    assert_eq!(&*seen.borrow(), &[2, 16]);
    let runs = runtime.stats();
    left.revise(|v| *v = 99);
    runtime.flush().unwrap();
    assert_eq!(runtime.stats().derived_runs, runs.derived_runs);
    assert_eq!(doubled.get(), 16);
}

#[test]
fn shared_paths_follow_the_owners_of_aliases_and_recycle_without_visited_history() {
    let runtime = Runtime::new();
    let source_owner = runtime.owner();
    let source = source_owner.cage(model());
    let observe_owner = runtime.owner();
    let alias = observe_owner.enter(|| source.field("side", |m| &m.side, |m| &mut m.side));
    let runs = Rc::new(Cell::new(0));
    let seen = runs.clone();
    observe_owner.binding(move || {
        let _ = alias.try_get();
        seen.set(seen.get() + 1);
    });
    runtime.flush().unwrap();
    for i in 0..3000 {
        let row = runtime.owner();
        let field = row.enter(|| source.field("side", |m| &m.side, |m| &mut m.side));
        assert_eq!(runtime.stats().fields, 1);
        field.revise(|v| *v += 1);
        if i % 100 == 0 {
            runtime.flush().unwrap();
        }
        drop(row);
        assert!(!field.is_alive());
        assert_eq!(field.try_get(), Err(AccessError::Disposed));
        assert_eq!(runtime.stats().fields, 1);
        assert!(runtime.stats().field_slots <= 1 && runtime.stats().node_slots <= 4);
    }
    runtime.flush().unwrap();
    assert_eq!(alias.get(), 3003);
    assert!(runs.get() > 1);
    drop(observe_owner);
    assert_eq!(runtime.stats().fields, 0);
    for _ in 0..3000 {
        let row = runtime.owner();
        let field = row.enter(|| source.field("side", |m| &m.side, |m| &mut m.side));
        drop(row);
        assert!(!field.is_alive());
        assert_eq!(runtime.stats().fields, 0);
    }
    assert!(runtime.stats().field_slots <= 1);
}

#[test]
fn disposing_an_alias_inside_its_write_still_notifies_other_live_aliases() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(model());
    let row = runtime.owner();
    let local = row.enter(|| source.field("side", |m| &m.side, |m| &mut m.side));
    let other = source.field("side", |m| &m.side, |m| &mut m.side);
    let seen = Rc::new(Cell::new(0));
    let out = seen.clone();
    owner.binding(move || out.set(other.get()));
    runtime.flush().unwrap();
    local.revise(move |v| {
        *v = 25;
        drop(row);
    });
    runtime.flush().unwrap();
    assert_eq!(seen.get(), 25);
    assert!(!local.is_alive());
    assert!(other.is_alive());
    let foreign_owner = runtime.owner();
    let foreign = foreign_owner.enter(|| source.field("side", |m| &m.side, |m| &mut m.side));
    drop(owner);
    assert!(!foreign.is_alive());
    assert_eq!(foreign.try_get(), Err(AccessError::Disposed));
    assert_eq!(runtime.stats().fields, 0);
}

#[test]
fn rejected_field_paths_and_bad_mutable_accessors_do_not_publish_or_allocate() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(model());
    let pair = source.field("pair", |m| &m.pair, |m| &mut m.pair);
    let before = runtime.stats();
    assert!(matches!(
        source.try_field("alias", |m| &m.pair.left, |m| &mut m.pair.left),
        Err(AccessError::InvalidField(_))
    ));
    assert!(matches!(
        source.try_field("pair", |m| &m.side, |m| &mut m.side),
        Err(AccessError::TypeMismatch)
    ));
    assert!(matches!(
        source.try_field("heap", |m| &m.bytes[0], |m| &mut m.bytes[0]),
        Err(AccessError::InvalidField(_))
    ));
    assert!(matches!(
        source.try_field("", |m| &m.side, |m| &mut m.side),
        Err(AccessError::InvalidField(_))
    ));
    assert_eq!(runtime.stats(), before);
    let wrong = pair.field("left", |p| &p.left, |p| &mut p.right);
    let before = runtime.stats();
    let called = Cell::new(false);
    assert!(matches!(
        wrong.try_revise(|v| {
            called.set(true);
            *v = 999;
        }),
        Err(AccessError::InvalidField(_))
    ));
    assert!(!called.get());
    assert_eq!(runtime.stats(), before);
    assert_eq!(source.with(|m| (m.pair.left, m.pair.right)), (1, 2));
}

#[test]
fn accessor_failures_unwind_and_partial_business_writes_still_publish() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(model());
    let panic = Rc::new(Cell::new(false));
    let check = panic.clone();
    let side = source.field(
        "side",
        move |m| {
            assert!(!check.get(), "accessor retry");
            &m.side
        },
        |m| &mut m.side,
    );
    let seen = Rc::new(Cell::new(0));
    let out = seen.clone();
    owner.binding(move || out.set(side.get()));
    runtime.flush().unwrap();
    panic.set(true);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| side.get())).is_err());
    panic.set(false);
    assert_eq!(source.with(|m| m.side), 3);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| side.revise(|v| {
            *v = 77;
            panic!("partial write")
        })))
        .is_err()
    );
    runtime.flush().unwrap();
    assert_eq!(seen.get(), 77);
    assert_eq!(runtime.stats().source_writes, 1);
    source.with(|_| assert_eq!(side.try_revise(|v| *v = 0), Err(AccessError::Borrowed)));
    assert_eq!(side.get(), 77);
}

#[test]
fn projection_and_pure_computation_rules_reject_hidden_dependencies_and_writes() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(model());
    let other = owner.cage(1);
    let errors = Rc::new(RefCell::new(Vec::new()));
    let out = errors.clone();
    let field = source.field(
        "side",
        move |m| {
            out.borrow_mut().push(other.try_get());
            &m.side
        },
        |m| &mut m.side,
    );
    assert!(
        errors
            .borrow()
            .iter()
            .all(|v| matches!(v, Err(AccessError::InvalidField(_))))
    );
    let attempted = Rc::new(RefCell::new(None));
    let out = attempted.clone();
    let derived = owner.bond(move || {
        *out.borrow_mut() = Some(field.try_revise(|v| *v += 1));
        field.get()
    });
    assert_eq!(derived.get(), 3);
    assert_eq!(
        *attempted.borrow(),
        Some(Err(AccessError::WriteDuringCompute))
    );
    assert_eq!(other.get(), 1);
}

#[test]
fn projection_captures_drop_outside_graph_borrows_on_reuse_rejection_and_cleanup() {
    struct Capture(glory_core::Cage<usize>);
    impl Drop for Capture {
        fn drop(&mut self) {
            self.0.revise(|n| *n += 1);
        }
    }
    let runtime = Runtime::new();
    let observer = runtime.owner();
    let drops = observer.cage(0);
    let source_owner = runtime.owner();
    let source = source_owner.cage(model());
    let row = runtime.owner();
    let capture = Capture(drops);
    let first = row.enter(|| {
        source.field(
            "side",
            move |m| {
                let _ = &capture;
                &m.side
            },
            |m| &mut m.side,
        )
    });
    let capture = Capture(drops);
    let _duplicate = row.enter(|| {
        source.field(
            "side",
            move |m| {
                let _ = &capture;
                &m.side
            },
            |m| &mut m.side,
        )
    });
    assert_eq!(drops.get(), 1); // Duplicate accessors are not retained.
    let capture = Capture(drops);
    assert!(matches!(
        source.try_field(
            "overlap",
            move |m| {
                let _ = &capture;
                &m.side
            },
            |m| &mut m.side
        ),
        Err(AccessError::InvalidField(_))
    ));
    assert_eq!(drops.get(), 2);
    drop(row);
    assert!(!first.is_alive());
    assert_eq!(drops.get(), 3);
    let capture = Capture(drops);
    let _foreign = observer.enter(|| {
        source.field(
            "side",
            move |m| {
                let _ = &capture;
                &m.side
            },
            |m| &mut m.side,
        )
    });
    drop(source_owner);
    assert_eq!(drops.get(), 4);
    assert_eq!(runtime.stats().fields, 0);
}

#[test]
fn moving_selectors_are_rejected_and_nesting_is_bounded_and_reclaimed() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let model = owner.cage(model());
    let switch = Rc::new(Cell::new(false));
    let read = switch.clone();
    let write = switch.clone();
    let field = model.field(
        "selected",
        move |m| {
            if read.get() {
                &m.pair.right
            } else {
                &m.pair.left
            }
        },
        move |m| {
            if write.get() {
                &mut m.pair.right
            } else {
                &mut m.pair.left
            }
        },
    );
    switch.set(true);
    let before = runtime.stats();
    assert!(matches!(field.try_get(), Err(AccessError::InvalidField(_))));
    assert!(matches!(
        field.try_revise(|v| *v = 99),
        Err(AccessError::InvalidField(_))
    ));
    assert_eq!(runtime.stats(), before);
    assert_eq!(model.with(|m| (m.pair.left, m.pair.right)), (1, 2));
    switch.set(false);
    assert_eq!(field.get(), 1);
    let root = owner.cage(0usize);
    let mut nested = root;
    for _ in 0..128 {
        nested = nested.field("inner", |n| n, |n| n);
    }
    assert!(matches!(
        nested.try_field("too-deep", |n| n, |n| n),
        Err(AccessError::InvalidField(_))
    ));
    nested.revise(|n| *n = 10);
    assert_eq!(root.get(), 10);
    drop(owner);
    assert_eq!(runtime.stats().fields, 0);
    assert_eq!(runtime.stats().nodes, 0);
}

#[test]
fn mixed_field_and_root_writes_match_plain_evaluation_with_late_subscribers() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(model());
    let pair = source.field("pair", |m| &m.pair, |m| &mut m.pair);
    let left = pair.field("left", |p| &p.left, |p| &mut p.left);
    let right = pair.field("right", |p| &p.right, |p| &mut p.right);
    let flag = owner.cage(true);
    let result = owner
        .bond(move || if flag.get() { left.get() } else { right.get() })
        .with_partial_eq();
    let mut expected = (1, 2, true);
    for step in 0..1000 {
        runtime
            .batch(|| {
                let (l, r) = (step * 3, step * 5);
                match step % 3 {
                    0 => {
                        left.revise(|n| *n = l);
                        expected.0 = l;
                    }
                    1 => {
                        pair.revise(|p| p.right = r);
                        expected.1 = r;
                    }
                    _ => {
                        source.revise(|m| {
                            m.pair.left = l;
                            m.pair.right = r;
                        });
                        expected.0 = l;
                        expected.1 = r;
                    }
                }
                if step % 5 == 0 {
                    expected.2 = !expected.2;
                    flag.revise(|f| *f = expected.2);
                }
                assert_eq!(
                    result.get(),
                    if expected.2 { expected.0 } else { expected.1 }
                );
                let late = runtime.owner();
                let alias = late.enter(|| pair.field("left", |p| &p.left, |p| &mut p.left));
                let seen = Rc::new(Cell::new(-1));
                let out = seen.clone();
                late.binding(move || out.set(alias.get()));
                alias.revise(|n| *n += 1);
                expected.0 += 1;
                assert_eq!(
                    result.get(),
                    if expected.2 { expected.0 } else { expected.1 }
                );
                drop(late);
            })
            .unwrap();
        assert_eq!(
            source.with(|m| (m.pair.left, m.pair.right)),
            (expected.0, expected.1)
        );
        assert_eq!(runtime.stats().fields, 3);
    }
}

#[test]
fn repeated_field_writes_reuse_one_target_without_coalescing_versions_or_mid_batch_reads() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let model = owner.cage((0usize, 0usize));
    let left = model.field("left", |m| &m.0, |m| &mut m.0);
    let right = model.field("right", |m| &m.1, |m| &mut m.1);
    let row = runtime.owner();
    let alias = row.enter(|| model.field("left", |m| &m.0, |m| &mut m.0));
    let doubled = owner.bond(move || alias.get() * 2).with_partial_eq();
    let seen = Rc::new(Cell::new(0));
    let out = seen.clone();
    owner.binding(move || out.set(doubled.get()));
    let unrelated = Rc::new(Cell::new(0));
    let out = unrelated.clone();
    owner.binding(move || {
        right.get();
        out.set(out.get() + 1);
    });
    runtime.flush().unwrap();
    let before = runtime.stats();
    for batch in 0..20 {
        runtime
            .batch(|| {
                for i in 0..1000 {
                    left.revise(|v| *v += 1);
                    if i == 500 {
                        assert_eq!(doubled.get(), 2 * (batch * 1000 + 501));
                    }
                }
            })
            .unwrap();
        assert_eq!(seen.get(), 2 * (batch + 1) * 1000);
    }
    let stats = runtime.stats();
    assert_eq!(stats.source_writes - before.source_writes, 20_000);
    assert_eq!(stats.field_target_builds - before.field_target_builds, 1);
    assert_eq!(stats.field_target_hits - before.field_target_hits, 19_999);
    assert_eq!(stats.field_target_visits - before.field_target_visits, 3);
    assert_eq!((stats.field_target_caches, stats.field_target_keys), (1, 3));
    assert_eq!(unrelated.get(), 1);
    drop(row);
    assert_eq!(
        (
            runtime.stats().field_target_caches,
            runtime.stats().field_target_keys
        ),
        (0, 0)
    );
    left.revise(|v| *v += 1);
    assert_eq!(
        (
            runtime.stats().field_target_caches,
            runtime.stats().field_target_keys
        ),
        (1, 2)
    );
}

#[test]
fn alias_and_descendant_changes_invalidate_targets_but_unrelated_roots_keep_theirs() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let model = owner.cage(((0usize, 0usize), 0usize));
    let pair = model.field("pair", |m| &m.0, |m| &mut m.0);
    let other = owner.cage((0usize,));
    let other_field = other.field("value", |m| &m.0, |m| &mut m.0);
    pair.revise(|v| v.0 += 1);
    other_field.revise(|v| *v += 1);
    assert_eq!(runtime.stats().field_target_caches, 2);
    let child = runtime.owner();
    let left = child.enter(|| pair.field("left", |p| &p.0, |p| &mut p.0));
    assert_eq!(runtime.stats().field_target_caches, 1);
    let seen = Rc::new(Cell::new(0));
    let out = seen.clone();
    child.binding(move || out.set(left.get()));
    runtime.flush().unwrap();
    pair.revise(|v| v.0 = 10);
    runtime.flush().unwrap();
    assert_eq!(seen.get(), 10);
    assert_eq!(runtime.stats().field_target_caches, 2);
    assert_eq!(runtime.stats().field_target_keys, 5);
    // Switching root/field selections replaces the single target for this root.
    for i in 0..10 {
        model.revise(|m| m.0.0 = i);
        pair.revise(|p| p.0 = i + 1);
        runtime.flush().unwrap();
        assert_eq!(seen.get(), i + 1);
        assert_eq!(runtime.stats().field_target_caches, 2);
    }
    drop(child);
    assert!(!left.is_alive());
    assert_eq!(runtime.stats().field_target_caches, 1);
    let next = runtime.owner();
    let right = next.enter(|| pair.field("right", |p| &p.1, |p| &mut p.1));
    let seen = Rc::new(Cell::new(0));
    let out = seen.clone();
    next.binding(move || out.set(right.get()));
    runtime.flush().unwrap();
    model.revise(|m| m.0.1 = 99);
    runtime.flush().unwrap();
    assert_eq!(seen.get(), 99);
    drop(owner);
    assert_eq!(
        (
            runtime.stats().field_target_caches,
            runtime.stats().field_target_keys
        ),
        (0, 0)
    );
}

#[test]
fn cached_identities_observe_late_bindings_effect_writes_panics_and_self_disposal() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage((0usize,));
    let field = source.field("value", |m| &m.0, |m| &mut m.0);
    field.revise(|v| *v = 1);
    let before = runtime.stats();
    let seen = Rc::new(Cell::new(0));
    let out = seen.clone();
    owner.binding(move || out.set(field.get()));
    runtime.flush().unwrap();
    field.revise(|v| *v = 2);
    runtime.flush().unwrap();
    assert_eq!(seen.get(), 2);
    assert_eq!(
        runtime.stats().field_target_hits,
        before.field_target_hits + 1
    );
    let effect = owner.effect(move || {
        if field.get() < 5 {
            field.revise(|v| *v += 1);
        }
    });
    runtime.flush().unwrap();
    assert_eq!(seen.get(), 5);
    effect.dispose();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| field.revise(|v| {
            *v = 6;
            panic!("partial cached write")
        })))
        .is_err()
    );
    runtime.flush().unwrap();
    assert_eq!(seen.get(), 6);
    let row = runtime.owner();
    let local = row.enter(|| source.field("value", |m| &m.0, |m| &mut m.0));
    local.revise(|v| *v = 7); // Warm a target containing both aliases.
    local.revise(move |v| {
        *v = 8;
        drop(row);
    });
    runtime.flush().unwrap();
    assert_eq!(seen.get(), 8);
    assert!(!local.is_alive());
    assert_eq!(runtime.stats().field_target_caches, 0);
    field.revise(|v| *v = 9);
    runtime.flush().unwrap();
    assert_eq!(seen.get(), 9);
}

#[test]
fn rejected_access_does_not_install_or_replace_a_valid_target_or_change_stats() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage((1usize, 2usize));
    let bad = source.field("bad", |m| &m.0, |m| &mut m.1);
    source.revise(|m| m.1 = 3);
    let before = runtime.stats();
    assert!(matches!(
        bad.try_revise(|v| *v = 100),
        Err(AccessError::InvalidField(_))
    ));
    assert_eq!(runtime.stats(), before);
    assert!(matches!(
        source.try_field("overlap", |m| m, |m| m),
        Err(AccessError::InvalidField(_))
    ));
    assert_eq!(runtime.stats(), before);
    source.with(|_| assert_eq!(source.try_revise(|_| ()), Err(AccessError::Borrowed)));
    assert_eq!(runtime.stats(), before);
    let rows = owner.cage(vec![0usize]);
    let missing = rows.item(2);
    rows.revise(|v| v[0] = 1);
    let before = runtime.stats();
    assert_eq!(
        missing.try_revise(|v| *v = 2),
        Err(AccessError::MissingField)
    );
    assert_eq!(runtime.stats(), before);
}

#[test]
fn moving_windows_discard_targets_and_reuse_slots_without_written_key_history() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let rows = owner.cage(vec![0usize; 10000]);
    for i in 0..3000 {
        let row = runtime.owner();
        let item = row.enter(|| rows.item(i));
        item.revise(|v| *v = i);
        item.revise(|v| *v += 1);
        assert_eq!(item.get(), i + 1);
        assert_eq!(
            (
                runtime.stats().field_target_caches,
                runtime.stats().field_target_keys
            ),
            (1, 2)
        );
        drop(row);
        assert!(!item.is_alive());
        assert_eq!(
            (
                runtime.stats().fields,
                runtime.stats().field_target_caches,
                runtime.stats().field_target_keys
            ),
            (0, 0, 0)
        );
        assert!(runtime.stats().field_slots <= 1 && runtime.stats().node_slots <= 2);
    }
    assert_eq!(runtime.stats().field_target_builds, 3000);
    assert_eq!(runtime.stats().field_target_hits, 3000);
}
