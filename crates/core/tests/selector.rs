use glory_core::{AccessError, Runtime};
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    rc::Rc,
};

#[test]
fn switches_publish_only_old_and_new_live_channels_and_batch_to_the_final_key() {
    const COUNT: usize = 4096;
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(None);
    let selector = owner.selector(source).unwrap();
    let rows: Vec<_> = (0..COUNT)
        .map(|key| {
            let row = owner.child();
            let selected = row.enter(|| selector.selected(key));
            let count = Rc::new(Cell::new(0));
            let calls = count.clone();
            let observed = selected.clone();
            row.binding(move || {
                observed.get();
                calls.set(calls.get() + 1);
            });
            (row, selected, count)
        })
        .collect();
    runtime.flush().unwrap();
    source.revise(|key| *key = Some(12));
    assert!(
        rows[12].1.get(),
        "reads before flush use the current source"
    );
    runtime.flush().unwrap();
    assert!(rows[12].1.get());
    assert_eq!(rows.iter().map(|row| row.2.get()).sum::<usize>(), COUNT + 1);
    let before = runtime.stats();
    source.revise(|key| *key = Some(23));
    runtime.flush().unwrap();
    let after = runtime.stats();
    let calls = rows.iter().map(|row| row.2.get()).sum::<usize>() - (COUNT + 1);
    eprintln!(
        "{COUNT} selection consumers: executed={calls}, graph marked={}",
        after.marked - before.marked
    );
    assert_eq!(calls, 2);
    assert!(after.marked - before.marked <= 3);
    assert!(!rows[12].1.get());
    assert!(rows[23].1.get());
    source.revise(|key| *key = Some(23));
    runtime.flush().unwrap();
    assert_eq!(rows.iter().map(|row| row.2.get()).sum::<usize>(), COUNT + 3);
    runtime
        .batch(|| {
            source.revise(|key| *key = Some(1));
            assert!(rows[1].1.get());
            assert!(!rows[23].1.get());
            source.revise(|key| *key = Some(2));
            assert!(rows[2].1.get());
            assert!(!rows[1].1.get());
            source.revise(|key| *key = Some(3));
            assert!(rows[3].1.get());
        })
        .unwrap();
    assert!(rows[3].1.get());
    // Old/new plus the two intermediate keys actually read in this batch.
    assert_eq!(rows.iter().map(|row| row.2.get()).sum::<usize>(), COUNT + 7);
    assert_eq!(selector.stats().unwrap().channels, COUNT);
    drop(rows);
    assert_eq!(selector.stats().unwrap().channels, 0);
    assert_eq!(selector.stats().unwrap().buckets, 0);

    // Execute the former shared-source implementation with the same scale.
    let count = Rc::new(Cell::new(0));
    for id in 0..COUNT {
        let count = count.clone();
        owner.binding(move || {
            let _selected = source.get() == Some(id);
            count.set(count.get() + 1);
        });
    }
    runtime.flush().unwrap();
    count.set(0);
    source.revise(|key| *key = Some(12));
    runtime.flush().unwrap();
    eprintln!(
        "{COUNT} former shared-source consumers: executed={}",
        count.get()
    );
    assert_eq!(count.get(), COUNT);
}

#[test]
fn collision_keys_multiview_owner_cleanup_and_reinsertion_keep_no_history() {
    #[derive(Clone, PartialEq, Eq)]
    struct Key(usize);
    impl Hash for Key {
        fn hash<H: Hasher>(&self, hash: &mut H) {
            0usize.hash(hash);
        }
    }
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(Some(Key(1)));
    let selector = owner.selector(source).unwrap();
    let a = owner.child();
    let b = owner.child();
    let one = a.enter(|| selector.selected(Key(1)));
    let again = a.enter(|| selector.selected(Key(1)));
    let other = b.enter(|| selector.selected(Key(2)));
    assert_eq!(selector.stats().unwrap().channels, 2);
    assert!(one.get() && again.get() && !other.get());
    runtime.flush().unwrap();
    source.revise(|key| *key = Some(Key(2)));
    runtime.flush().unwrap();
    assert!(!one.get() && !again.get() && other.get());
    drop(a);
    assert_eq!(one.try_get(), Err(AccessError::Disposed));
    assert_eq!(selector.stats().unwrap().channels, 1);
    for key in 3..103 {
        let row = owner.child();
        let value = row.enter(|| selector.selected(Key(key)));
        assert!(!value.get());
        drop(row);
        assert_eq!(value.try_get(), Err(AccessError::Disposed));
        assert_eq!(selector.stats().unwrap().channels, 1);
    }
    drop(b);
    assert_eq!(selector.stats().unwrap().channels, 0);
    assert_eq!(selector.stats().unwrap().buckets, 0);
    let c = owner.child();
    let current = c.enter(|| selector.selected(Key(2)));
    assert!(current.get());
    source.revise(|key| *key = None);
    runtime.flush().unwrap();
    assert!(!current.get());
}

#[test]
fn selector_owner_retirement_invalidates_independent_consumers_and_pure_or_foreign_registration_fails()
 {
    let runtime = Runtime::new();
    let root = runtime.owner();
    let owner = root.child();
    let source = owner.cage(Some(1usize));
    let selector = owner.selector(source).unwrap();
    let consumer = root.child();
    let selected = consumer.enter(|| selector.selected(1));
    assert!(selected.get());
    assert!(root.compute(|| selector.try_selected(2)).unwrap().is_err());
    let foreign = Runtime::new();
    let foreign_owner = foreign.owner();
    assert!(matches!(
        foreign_owner.selector(source),
        Err(AccessError::WrongRuntime)
    ));
    assert!(matches!(
        foreign_owner.enter(|| selector.try_selected(2)),
        Err(AccessError::WrongRuntime)
    ));
    drop(owner);
    assert_eq!(selected.try_get(), Err(AccessError::Disposed));
    assert_eq!(selector.stats(), Err(AccessError::Disposed));
    assert!(matches!(
        consumer.enter(|| selector.try_selected(1)),
        Err(AccessError::Disposed)
    ));
}

#[test]
fn key_callbacks_and_destructors_are_outside_graph_and_index_borrows() {
    struct Key {
        id: usize,
        inspect: Rc<dyn Fn()>,
    }
    impl Clone for Key {
        fn clone(&self) -> Self {
            (self.inspect)();
            Self {
                id: self.id,
                inspect: self.inspect.clone(),
            }
        }
    }
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            (self.inspect)();
            self.id == other.id
        }
    }
    impl Eq for Key {}
    impl Hash for Key {
        fn hash<H: Hasher>(&self, hash: &mut H) {
            (self.inspect)();
            0usize.hash(hash);
        }
    }
    impl Drop for Key {
        fn drop(&mut self) {
            (self.inspect)();
        }
    }
    let runtime = Rc::new(Runtime::new());
    let owner = runtime.owner();
    let inspected = Rc::new(Cell::new(0));
    let callback: Rc<dyn Fn()> = Rc::new({
        let runtime = runtime.clone();
        let count = inspected.clone();
        move || {
            runtime.stats();
            count.set(count.get() + 1);
        }
    });
    let key = |id| Key {
        id,
        inspect: callback.clone(),
    };
    let source = owner.cage(Some(key(1)));
    let selector = owner.selector(source).unwrap();
    let row = owner.child();
    let selected = row.enter(|| selector.selected(key(2)));
    runtime.flush().unwrap();
    source.revise(|value| *value = Some(key(2)));
    runtime.flush().unwrap();
    assert!(selected.get());
    drop(row);
    assert_eq!(selector.stats().unwrap().channels, 0);
    assert!(inspected.get() > 0);
}

#[test]
fn retiring_selector_during_key_hashing_cannot_register_an_orphan_channel() {
    use std::cell::RefCell;
    #[derive(Clone)]
    struct Key(Rc<dyn Fn()>);
    impl PartialEq for Key {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }
    impl Eq for Key {}
    impl Hash for Key {
        fn hash<H: Hasher>(&self, hash: &mut H) {
            (self.0)();
            0usize.hash(hash);
        }
    }
    let runtime = Runtime::new();
    let root = runtime.owner();
    let owner = Rc::new(RefCell::new(Some(root.child())));
    let selector = {
        let owner = owner.borrow();
        let owner = owner.as_ref().unwrap();
        owner.selector(owner.cage(None::<Key>)).unwrap()
    };
    let consumer = root.child();
    let key = Key(Rc::new({
        let owner = owner.clone();
        move || {
            drop(owner.borrow_mut().take());
        }
    }));
    assert!(matches!(
        consumer.enter(|| selector.try_selected(key)),
        Err(AccessError::Disposed)
    ));
    assert_eq!(runtime.stats().nodes, 0);
}

#[test]
fn registration_between_unflushed_writes_uses_the_last_published_notification_baseline() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(Some(1usize));
    let selector = owner.selector(source).unwrap();
    runtime.flush().unwrap();
    source.revise(|key| *key = Some(2));
    let picked = selector.selected(2);
    assert!(
        picked.get(),
        "direct reads see the current unflushed source"
    );
    source.revise(|key| *key = Some(1));
    runtime.flush().unwrap();
    let observed = Rc::new(Cell::new(false));
    let calls = Rc::new(Cell::new(0));
    let result = observed.clone();
    let count = calls.clone();
    owner.binding(move || {
        result.set(picked.get());
        count.set(count.get() + 1);
    });
    runtime.flush().unwrap();
    assert!(!observed.get());
    assert_eq!(calls.get(), 1);
    source.revise(|key| *key = Some(2));
    runtime.flush().unwrap();
    assert!(
        observed.get(),
        "the next real transition must publish to the registered channel"
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn derived_bond_observes_selection_notifications_at_flush_while_direct_reads_are_current() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(None::<usize>);
    let selector = owner.selector(source).unwrap();
    let picked = selector.selected(1);
    let cached = picked.clone();
    let derived = owner.bond(move || cached.get());
    runtime.flush().unwrap();
    assert!(!derived.get());
    source.revise(|key| *key = Some(1));
    assert!(picked.get());
    assert!(
        !derived.get(),
        "derived cache notification occurs at the flush boundary"
    );
    runtime.flush().unwrap();
    assert!(derived.get());
}

#[test]
fn registering_for_a_temporary_batch_key_does_not_suppress_its_later_notification() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(Some(1usize));
    let selector = owner.selector(source).unwrap();
    runtime.flush().unwrap();
    let picked = runtime
        .batch(|| {
            source.revise(|key| *key = Some(2));
            let picked = selector.selected(2);
            assert!(picked.get());
            source.revise(|key| *key = Some(1));
            assert!(!picked.get());
            picked
        })
        .unwrap();
    let observed = Rc::new(Cell::new(false));
    let result = observed.clone();
    owner.binding(move || result.set(picked.get()));
    runtime.flush().unwrap();
    assert!(!observed.get());
    source.revise(|key| *key = Some(2));
    runtime.flush().unwrap();
    assert!(observed.get());
}

#[test]
fn a_bond_first_read_during_a_temporary_selection_is_correct_after_flush() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(None::<usize>);
    let selector = owner.selector(source).unwrap();
    let picked = selector.selected(1);
    runtime.flush().unwrap();
    source.revise(|key| *key = Some(1));
    let bond = owner.bond(move || picked.get());
    assert!(bond.get());
    source.revise(|key| *key = None);
    runtime.flush().unwrap();
    assert!(
        !bond.get(),
        "flush must invalidate a cached read of the temporary key"
    );
}

#[test]
fn read_ahead_is_deduplicated_with_normal_publication_and_removed_on_owner_retirement() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(None::<usize>);
    let selector = owner.selector(source).unwrap();
    let row = owner.child();
    let picked = row.enter(|| selector.selected(1));
    runtime.flush().unwrap();
    source.revise(|key| *key = Some(1));
    for _ in 0..100 {
        assert!(picked.get());
    }
    assert_eq!(selector.stats().unwrap().read_ahead, 1);
    let before = runtime.stats().source_writes;
    runtime.flush().unwrap();
    assert_eq!(
        runtime.stats().source_writes - before,
        1,
        "old/new plus read-ahead publishes once"
    );
    assert_eq!(selector.stats().unwrap().read_ahead, 0);
    source.revise(|key| *key = None);
    assert!(!picked.get());
    assert_eq!(selector.stats().unwrap().read_ahead, 1);
    drop(row);
    assert_eq!(selector.stats().unwrap().read_ahead, 0);
    assert_eq!(selector.stats().unwrap().channels, 0);
    runtime.flush().unwrap();
    assert_eq!(picked.try_get(), Err(AccessError::Disposed));
    for key in 2..102 {
        let row = owner.child();
        let picked = row.enter(|| selector.selected(key));
        source.revise(|current| *current = Some(key));
        assert!(picked.get());
        assert_eq!(selector.stats().unwrap().read_ahead, 1);
        drop(row);
        assert_eq!(selector.stats().unwrap().read_ahead, 0);
        assert_eq!(selector.stats().unwrap().channels, 0);
    }
    runtime.flush().unwrap();
    assert_eq!(selector.stats().unwrap().read_ahead, 0);
}
