use glory_core::Vary;
use glory_render::Props;
use glory_ui::{App, Widget, each, text};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn keyed_members_remain_non_clone_and_field_updates_do_not_reconcile_order() {
    struct Item {
        id: usize,
        label: String,
    }
    let app = App::memory();
    let source = app
        .scope()
        .keyed_list(
            vec![Item {
                id: 1,
                label: "one".into(),
            }],
            |row| row.id,
        )
        .unwrap();
    let made = Rc::new(Cell::new(0));
    let calls = made.clone();
    let handles = Rc::new(RefCell::new(Vec::new()));
    let captured = handles.clone();
    app.mount(each(source).children(move |row| {
        calls.set(calls.get() + 1);
        let label = row.field("label", |row| &row.label, |row| &mut row.label);
        captured.borrow_mut().push(label);
        text().content(label)
    }))
    .unwrap();
    let id = app.tree().node(app.tree().roots()[0]).unwrap().children[0];
    let runs = app.runtime_stats().structural_runs;
    source
        .update_key(&1, |row| row.label = "edited".into())
        .unwrap();
    app.flush().unwrap();
    assert_eq!(app.runtime_stats().structural_runs, runs);
    assert!(
        matches!(app.tree().node(id).unwrap().descriptor.view().props, Props::Text(value) if value.content == "edited")
    );
    // Even without an intervening observation, a new membership retires the old row.
    source.remove_key(&1).unwrap();
    source
        .extend(vec![Item {
            id: 1,
            label: "new".into(),
        }])
        .unwrap();
    app.flush().unwrap();
    assert_eq!(made.get(), 2);
    assert!(!handles.borrow()[0].is_alive());
    assert!(handles.borrow()[1].is_alive());
    assert!(app.tree().node(id).is_err());
    app.clear().unwrap();
    assert!(handles.borrow().iter().all(|value| !value.is_alive()));
}

#[test]
fn values_vary_cage_and_bond_use_the_same_key_and_row_builder() {
    let app = App::memory();
    let source = app.scope().cage(vec![1usize, 2]);
    fn widget(
        source: impl glory_ui::each::source::IntoSource<Source = glory_ui::each::source::Unkeyed<usize>>,
    ) -> impl Widget {
        each(source)
            .key(|id| *id)
            .children(|row| text().content(row.value.map(|id| id.to_string())))
    }
    app.mount(widget(vec![1, 2])).unwrap();
    app.mount(widget(source)).unwrap();
    app.mount(widget(source.map(Clone::clone))).unwrap();
    app.mount(widget(Vary::from(source))).unwrap();
    app.clear().unwrap();
    assert_eq!(app.stats().nodes, 0);
}

#[test]
fn foreign_and_disposed_sources_are_refused_before_creating_semantic_nodes() {
    let app = App::memory();
    let foreign = glory_core::Runtime::new();
    let owner = foreign.scope();
    let source = owner.cage(vec![1usize]);
    assert!(
        app.mount(each(source).key(|id| *id).children(|_| text()))
            .is_err()
    );
    assert!(app.tree().is_empty());
    let keyed = owner.keyed_list(vec![1usize], |id| *id).unwrap();
    assert!(app.mount(each(keyed).children(|_| text())).is_err());
    assert!(app.tree().is_empty());
    owner.owner().dispose();
    assert!(app.mount(each(keyed).children(|_| text())).is_err());
    assert!(app.tree().is_empty());
}

#[test]
fn disposing_a_live_order_source_reports_an_error_and_allows_owned_cleanup() {
    for keyed in [false, true] {
        let app = App::memory();
        let owner = app.scope().owner().child();
        if keyed {
            let source = owner.keyed_list(vec![1usize], |id| *id).unwrap();
            app.mount(each(source).children(|_| text())).unwrap();
        } else {
            let source = owner.cage(vec![1usize]);
            app.mount(each(source).key(|id| *id).children(|_| text()))
                .unwrap();
        }
        owner.dispose();
        assert!(app.flush().is_err());
        // A faulted App still owns and releases all row scopes on drop.
        let runtime = app.shared_runtime();
        drop(app);
        assert_eq!(runtime.stats().nodes, 0);
    }
}
