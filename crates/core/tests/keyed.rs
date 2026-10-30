use glory_core::{AccessError, ListError, Runtime};
use std::{cell::Cell, rc::Rc};
#[derive(Debug)]
struct Row {
    id: usize,
    label: String,
    other: usize,
}
fn row(id: usize) -> Row {
    Row {
        id,
        label: format!("row {id}"),
        other: 0,
    }
}
#[test]
fn stable_membership_and_selective_updates() {
    let rt = Runtime::new();
    let scope = rt.scope();
    let list = scope.keyed_list(vec![row(1), row(2)], |r| r.id).unwrap();
    let counts = Rc::new([Cell::new(0), Cell::new(0), Cell::new(0)]);
    let c = counts.clone();
    scope.structural(move || {
        list.with_order(|_| ());
        c[0].set(c[0].get() + 1)
    });
    let child = scope.child();
    let entry = list.at(0).unwrap();
    let before_projection = rt.stats().nodes;
    let label = child.enter(|| entry.field("label", |r| &r.label, |r| &mut r.label));
    assert_eq!(
        rt.stats().nodes,
        before_projection + 1,
        "keyed inline field must not retain an intermediate row alias"
    );
    let other = child.enter(|| entry.field("other", |r| &r.other, |r| &mut r.other));
    let c = counts.clone();
    child.binding(move || {
        label.with(|_| ());
        c[1].set(c[1].get() + 1)
    });
    let c = counts.clone();
    child.binding(move || {
        other.get();
        c[2].set(c[2].get() + 1)
    });
    rt.flush().unwrap();
    assert_eq!(counts.each_ref().map(Cell::get), [1, 1, 1]);
    label.revise(|s| s.push('!'));
    rt.flush().unwrap();
    assert_eq!(counts.each_ref().map(Cell::get), [1, 2, 1]);
    list.swap(0, 1).unwrap();
    label.revise(|s| s.push('?'));
    rt.flush().unwrap();
    assert_eq!(entry.key(), &1);
    assert_eq!(
        list.at(1)
            .unwrap()
            .try_with_untracked(|r| r.label.clone())
            .unwrap(),
        "row 1!?"
    );
    list.replace(vec![
        row(2),
        Row {
            id: 1,
            label: "replaced".into(),
            other: 2,
        },
    ])
    .unwrap();
    rt.flush().unwrap();
    assert_eq!(label.get(), "replaced");
    assert_eq!(counts[0].get(), 2);
    list.remove_key(&1).unwrap();
    assert_eq!(label.try_get(), Err(AccessError::MissingField));
    list.extend(vec![row(1)]).unwrap();
    assert_ne!(entry.id(), list.at(1).unwrap().id());
    assert_eq!(
        entry.update(|r| r.other = 9),
        Err(AccessError::MissingField)
    );
    drop(child);
    list.clear().unwrap();
    rt.flush().unwrap();
    assert_eq!(rt.stats().fields, 0);
    assert_eq!(rt.stats().entry_keys, 0);
}
#[test]
fn errors_do_not_publish_half_a_collection_and_edits_do_not_register_aliases() {
    let rt = Runtime::new();
    let scope = rt.scope();
    let baseline = rt.stats().nodes;
    assert!(matches!(
        scope.keyed_list(vec![row(1), row(1)], |r| r.id),
        Err(ListError::DuplicateKey)
    ));
    assert_eq!(rt.stats().nodes, baseline);
    let list = scope.keyed_list(vec![row(1)], |r| r.id).unwrap();
    let other = scope.keyed_list(vec![row(2)], |r| r.id).unwrap();
    list.with_order(|tokens| {
        assert!(matches!(
            other.entry(&tokens[0]),
            Err(AccessError::WrongRuntime)
        ))
    });
    assert_eq!(
        list.extend(vec![row(2), row(1)]),
        Err(ListError::DuplicateKey)
    );
    assert_eq!(
        list.replace(vec![row(3), row(3)]),
        Err(ListError::DuplicateKey)
    );
    assert_eq!(list.len_untracked(), 1);
    for _ in 0..100 {
        list.update_at(0, |r| r.label.push('!')).unwrap();
    }
    assert_eq!(rt.stats().fields, 0);
    assert_eq!(rt.stats().entry_keys, 0);
    assert_eq!(
        list.at(0)
            .unwrap()
            .try_with_untracked(|_| list.clear())
            .unwrap(),
        Err(AccessError::Borrowed)
    );
    assert_eq!(list.len_untracked(), 1);
    let parent = scope.id();
    let child = parent.child_scope().unwrap();
    drop(scope);
    assert!(!child.is_alive());
    assert!(matches!(parent.child_scope(), Err(AccessError::Disposed)));
}
#[test]
fn independent_views_own_aliases_and_hash_collisions_keep_complete_keys() {
    use std::hash::{Hash, Hasher};
    #[derive(Clone, PartialEq, Eq)]
    struct Key(usize);
    impl Hash for Key {
        fn hash<H: Hasher>(&self, h: &mut H) {
            0usize.hash(h)
        }
    }
    let rt = Runtime::new();
    let owner = rt.scope();
    let list = owner
        .keyed_list(vec![row(1), row(2)], |r| Key(r.id))
        .unwrap();
    let a = owner.child();
    let b = owner.child();
    let entry = list.at(1).unwrap();
    let x = a.enter(|| entry.field("label", |r| &r.label, |r| &mut r.label));
    let y = b.enter(|| entry.field("label", |r| &r.label, |r| &mut r.label));
    x.revise(|v| v.push('!'));
    assert_eq!(y.get(), "row 2!");
    drop(a);
    list.update_at(1, |r| r.label.push('?')).unwrap();
    assert_eq!(y.get(), "row 2!?");
    drop(b);
    list.remove_key(&Key(1)).unwrap();
    assert_eq!(list.at(0).unwrap().key().0, 2);
    assert_eq!(rt.stats().fields, 0);
    assert_eq!(rt.stats().entry_keys, 0);
}
#[test]
fn repeated_replacement_reuses_slots_and_append_preserves_old_field_consumers() {
    let rt = Runtime::new();
    let scope = rt.scope();
    let list = scope.keyed_list(vec![row(1)], |r| r.id).unwrap();
    let child = scope.child();
    let label = child.enter(|| {
        list.at(0)
            .unwrap()
            .field("label", |r| &r.label, |r| &mut r.label)
    });
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    child.binding(move || {
        label.with(|_| ());
        c.set(c.get() + 1)
    });
    rt.flush().unwrap();
    list.extend(vec![row(2)]).unwrap();
    rt.flush().unwrap();
    assert_eq!(calls.get(), 1);
    drop(child);
    list.clear().unwrap();
    for round in 0..50 {
        list.replace((0..32).map(|i| row(round * 32 + i)).collect())
            .unwrap();
        list.clear().unwrap();
    }
    assert_eq!(list.storage_slots_untracked(), 32);
    assert_eq!(list.len_untracked(), 0);
    assert_eq!(rt.stats().fields, 0);
}

#[test]
fn single_member_appends_clone_only_incoming_keys_and_reject_duplicates_atomically() {
    use std::hash::{Hash, Hasher};
    struct Key(usize, Rc<Cell<usize>>);
    impl Clone for Key {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for Key {}
    impl Hash for Key {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.0.hash(state);
        }
    }
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let clones = Rc::new(Cell::new(0));
    let count = clones.clone();
    let list = owner
        .keyed_list(Vec::<usize>::new(), move |id| Key(*id, count.clone()))
        .unwrap();
    const COUNT: usize = 4096;
    for id in 0..COUNT {
        list.extend(vec![id]).unwrap();
    }
    eprintln!(
        "single-member append key clones: {} for {COUNT} members",
        clones.get()
    );
    assert!(
        clones.get() <= COUNT * 3,
        "existing keys must not be cloned on append"
    );
    let before = runtime.stats().source_writes;
    assert_eq!(list.extend(vec![COUNT, 0]), Err(ListError::DuplicateKey));
    assert_eq!(runtime.stats().source_writes, before);
    assert_eq!(list.len_untracked(), COUNT);
    assert!(list.find(&Key(COUNT, clones)).unwrap().is_none());
}
