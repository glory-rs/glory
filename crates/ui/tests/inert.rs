use glory_render::{Change, Event, EventKind, Patch, Semantics};
use glory_ui::{App, button, input, view};
use std::{cell::Cell, rc::Rc};
fn inert(value: bool) -> Patch {
    Patch::Semantics(Change::Set(Some(Box::new(Semantics {
        inert: value,
        ..Default::default()
    }))))
}
#[test]
fn routed_events_observe_current_inert_ancestry_and_terminal_loss_still_cleans_up() {
    let app = App::memory();
    let calls = Rc::new(Cell::new(0));
    let clicked = calls.clone();
    let focused = calls.clone();
    app.mount(view().fill((
        button().on(EventKind::Activate, move |_| clicked.set(clicked.get() + 1)),
        input().on(EventKind::FocusChanged, move |_| {
            focused.set(focused.get() + 10)
        }),
    )))
    .unwrap();
    let root = app.tree().roots()[0];
    let button = app.tree().node(root).unwrap().children[0];
    let input = app.tree().node(root).unwrap().children[1];
    app.scope().patch(root, inert(true)).unwrap();
    app.flush().unwrap();
    assert!(!app.dispatch(button, Event::Activate).unwrap());
    assert!(!app.dispatch(input, Event::FocusChanged(true)).unwrap());
    assert_eq!(calls.get(), 0);
    assert!(app.dispatch(input, Event::FocusChanged(false)).unwrap());
    assert_eq!(calls.get(), 10);
    app.scope().patch(root, inert(false)).unwrap();
    app.flush().unwrap();
    assert!(app.dispatch(button, Event::Activate).unwrap());
    assert_eq!(calls.get(), 11);
    app.clear().unwrap();
    assert_eq!(app.stats().handlers, 0);
}
#[test]
fn a_direct_callback_that_blocks_its_region_does_not_publish_a_later_focus_within_callback() {
    let app = App::memory();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    app.mount(
        view()
            .on(EventKind::FocusWithin, move |_| {
                observed.set(observed.get() + 1)
            })
            .fill(input()),
    )
    .unwrap();
    let root = app.tree().roots()[0];
    let input = app.tree().node(root).unwrap().children[0];
    let weak = app.scope().downgrade();
    app.scope()
        .on(input, EventKind::FocusChanged, move |_| {
            if let Some(scope) = weak.upgrade() {
                scope.patch(root, inert(true)).unwrap();
            }
        })
        .unwrap();
    assert!(app.dispatch(input, Event::FocusChanged(true)).unwrap());
    assert_eq!(calls.get(), 0);
    app.clear().unwrap();
}
