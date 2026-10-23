use glory_render::{DragPhase, Event, EventKind, KeyboardCommand, Props};
use glory_ui::*;
use std::{cell::RefCell, rc::Rc};

#[test]
fn keyboard_bubbles_across_groups_only_to_nearest_live_observer_and_keeps_origin() {
    let app = App::memory();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let outer = seen.clone();
    let inner = seen.clone();
    let reference = app.scope().cage(None);
    app.mount(
        view()
            .on(EventKind::Keyboard, move |event| {
                outer.borrow_mut().push(("outer", event))
            })
            .fill(
                group().fill(
                    input()
                        .node_ref(reference)
                        .on(EventKind::Keyboard, move |event| {
                            inner.borrow_mut().push(("input", event))
                        }),
                ),
            ),
    )
    .unwrap();
    let id = reference.get().unwrap();
    let event = Event::Keyboard {
        from: id,
        command: KeyboardCommand::Submit,
    };
    assert!(app.dispatch(id, event.clone()).unwrap());
    assert_eq!(&*seen.borrow(), &[("input", event)]);
    let wrong = app.tree().roots()[0];
    assert!(
        app.dispatch(
            wrong,
            Event::Keyboard {
                from: id,
                command: KeyboardCommand::Help
            }
        )
        .is_err()
    );
    app.clear().unwrap();
    assert_eq!(reference.get(), None);
    assert!(
        !app.dispatch(
            id,
            Event::Keyboard {
                from: id,
                command: KeyboardCommand::Submit
            }
        )
        .unwrap()
    );
}

#[test]
fn node_reference_cleanup_does_not_clear_a_newer_branch_and_weak_scope_cannot_resurrect() {
    let app = App::memory();
    let reference = app.scope().cage(None);
    let weak = app.scope().downgrade();
    let first = view().node_ref(reference).mount(app.scope()).unwrap();
    let old = reference.get().unwrap();
    let second = view().node_ref(reference).mount(app.scope()).unwrap();
    let new = reference.get().unwrap();
    assert_ne!(old, new);
    drop(first);
    assert_eq!(reference.get(), Some(new));
    drop(second);
    assert_eq!(reference.get(), None);
    drop(app);
    assert!(weak.upgrade().is_none());
}

#[test]
fn drag_drop_accepts_live_siblings_but_rejects_nonfinite_foreign_and_retired_sources() {
    let app = App::memory();
    let reference = app.scope().cage(None);
    let target_ref = app.scope().cage(None);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let observed = seen.clone();
    let source = view()
        .node_ref(reference)
        .on(EventKind::Drag, |_| {})
        .mount(app.scope())
        .unwrap();
    let target = view()
        .node_ref(target_ref)
        .on(EventKind::Drag, move |event| {
            observed.borrow_mut().push(event)
        })
        .mount(app.scope())
        .unwrap();
    let source_id = reference.get().unwrap();
    let target_id = target_ref.get().unwrap();
    assert!(
        app.dispatch(
            target_id,
            Event::Drag {
                from: source_id,
                phase: DragPhase::Drop,
                x: 1.0,
                y: 2.0
            }
        )
        .unwrap()
    );
    assert_eq!(seen.borrow().len(), 1);
    assert!(
        app.dispatch(
            target_id,
            Event::Drag {
                from: source_id,
                phase: DragPhase::Move,
                x: 1.0,
                y: 2.0
            }
        )
        .is_err()
    );
    assert!(
        app.dispatch(
            target_id,
            Event::Drag {
                from: source_id,
                phase: DragPhase::Drop,
                x: f64::NAN,
                y: 2.0
            }
        )
        .is_err()
    );
    let foreign = App::memory();
    let other = foreign
        .scope()
        .create(glory_render::Descriptor::new(Props::View))
        .unwrap();
    assert!(
        app.dispatch(
            target_id,
            Event::Drag {
                from: other,
                phase: DragPhase::Drop,
                x: 1.0,
                y: 2.0
            }
        )
        .is_err()
    );
    drop(source);
    assert!(
        !app.dispatch(
            target_id,
            Event::Drag {
                from: source_id,
                phase: DragPhase::Drop,
                x: 1.0,
                y: 2.0
            }
        )
        .unwrap()
    );
    drop(target);
}
