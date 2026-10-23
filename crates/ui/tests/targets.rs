use glory_render::{Action, Error, Event, EventKind, NodeIds, RuntimeId};
use glory_ui::{App, button, group, input, view};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn invalid_interests_do_not_replace_handlers_allocate_slots_or_fault_the_app() {
    let app = App::memory();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    app.mount(view().fill((
        group(),
        input().on(EventKind::ValueChanged, move |_| {
            observed.set(observed.get() + 1)
        }),
    )))
    .unwrap();
    let root = app.tree().roots()[0];
    let nodes = app.tree().node(root).unwrap().children.clone();
    let before = app.stats().handlers;
    for (id, kind) in [
        (root, EventKind::ValueChanged),
        (nodes[0], EventKind::Resize),
        (nodes[1], EventKind::CheckedChanged),
    ] {
        assert!(
            app.scope()
                .on(id, kind, |_| panic!("invalid handler ran"))
                .is_err()
        );
        assert_eq!(app.stats().handlers, before);
    }
    app.dispatch(
        nodes[1],
        Event::ValueChanged {
            value: "ok".into(),
            composing: false,
            revision: 0,
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert!(app.dispatch(nodes[0], Event::FocusChanged(true)).is_err());
    app.flush().unwrap();
    app.mount(button().label("still usable")).unwrap();
    app.clear().unwrap();
    assert_eq!(app.stats().handlers, 0);
}

#[test]
fn action_identity_and_arguments_are_validated_before_capability_and_queue_changes() {
    let app = App::memory();
    app.mount(view()).unwrap();
    let id = app.tree().roots()[0];
    let foreign = NodeIds::new(RuntimeId::new()).allocate();
    assert_eq!(
        app.scope().action(Action::Focus(foreign)),
        Err(glory_ui::Error::Render(Error::ForeignRuntime))
    );
    assert!(matches!(
        app.scope().action(Action::ScrollTo {
            node: id,
            x: f64::NAN,
            y: 0.
        }),
        Err(glory_ui::Error::Render(Error::InvalidProps(_)))
    ));
    assert!(matches!(
        app.scope().action(Action::Focus(id)),
        Err(glory_ui::Error::Render(Error::Unsupported(_)))
    ));
    app.clear().unwrap();
    assert_eq!(
        app.scope().action(Action::Focus(id)),
        Err(glory_ui::Error::Render(Error::NodeGone(id)))
    );
    app.mount(input().value("recovered")).unwrap();
}
