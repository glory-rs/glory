use glory_render::Props;
use glory_ui::*;
use std::{cell::RefCell, rc::Rc};

#[test]
fn seeds_keep_local_state_on_reorder_but_reinsertion_constructs_a_fresh_owner() {
    let app = App::memory();
    let items = app.scope().cage(vec![(1, "one"), (2, "two")]);
    let handles = Rc::new(RefCell::new(Vec::new()));
    let made = handles.clone();
    app.mount(each_seed(
        items,
        |item| item.0,
        move |(key, seed)| {
            // A map constructed by the factory belongs to the row, too.
            let owned = items.map(move |_| key);
            let made = made.clone();
            widget(move |scope| {
                let local = scope.cage(seed.to_owned());
                made.borrow_mut().push((key, local, owned));
                text().content(local).show_in(scope)
            })
        },
    ))
    .unwrap();
    let initial = app.runtime_stats();
    let one = handles.borrow()[0].1;
    one.revise(|value| *value = "local".into());
    items.revise(|items| *items = vec![(2, "ignored"), (1, "also ignored")]);
    app.flush().unwrap();
    assert_eq!(handles.borrow().len(), 2);
    assert_eq!(app.runtime_stats().nodes, initial.nodes);
    assert_eq!(app.runtime_stats().owners, initial.owners);
    let texts = || {
        app.tree()
            .roots()
            .iter()
            .flat_map(|id| app.tree().node(*id).unwrap().children.clone())
            .map(
                |id| match app.tree().node(id).unwrap().descriptor.view().props {
                    Props::Text(value) => value.content.clone(),
                    _ => panic!("text row"),
                },
            )
            .collect::<Vec<_>>()
    };
    assert_eq!(texts(), ["two", "local"]);
    items.revise(|items| items.retain(|item| item.0 != 1));
    app.flush().unwrap();
    assert!(!one.is_alive());
    assert!(!handles.borrow()[0].2.is_alive());
    items.revise(|items| items.push((1, "new")));
    app.flush().unwrap();
    assert_eq!(handles.borrow().len(), 3);
    assert_eq!(texts(), ["two", "new"]);
    app.clear().unwrap();
    assert!(
        handles
            .borrow()
            .iter()
            .all(|(_, value, derived)| !value.is_alive() && !derived.is_alive())
    );
    assert_eq!(app.stats().nodes, 0);
}

#[test]
fn seed_rows_omit_exactly_two_sources_and_one_data_owner_per_key() {
    fn counts(seed: bool) -> (usize, usize) {
        let app = App::memory();
        let rows = vec![1usize, 2, 3];
        if seed {
            app.mount(each_seed(
                rows,
                |id| *id,
                |id| text().content(id.to_string()),
            ))
            .unwrap();
        } else {
            app.mount(each(
                rows,
                |id| *id,
                |id, _| text().content(id.get_untracked().to_string()),
            ))
            .unwrap();
        }
        (app.runtime_stats().nodes, app.runtime_stats().owners)
    }
    let full = counts(false);
    let seed = counts(true);
    assert_eq!(full.0 - seed.0, 6);
    assert_eq!(full.1 - seed.1, 3);
}
