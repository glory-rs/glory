use glory_render::{ButtonProps, Change, Descriptor, Patch, Props};
use glory_ui::*;
use std::{cell::Cell, rc::Rc};

#[test]
fn live_instances_bind_and_route_independently_then_retire_without_a_retained_scope() {
    let app = App::memory();
    let baseline = app.runtime_stats();
    let declaration = plan(view().mark("root").fill((
        text().content("static").mark("text"),
        button().mark("button"),
    )))
    .unwrap();
    let text = declaration.part("text").unwrap();
    let button = declaration.part("button").unwrap();
    let first_owner = app.scope().child().unwrap();
    let second_owner = app.scope().child().unwrap();
    let a = first_owner.cage("first".to_owned());
    let b = second_owner.cage("second".to_owned());
    let first = first_owner.instantiate(&declaration).unwrap();
    let second = second_owner.instantiate(&declaration).unwrap();
    first.text(&text, a).unwrap();
    second.text(&text, b).unwrap();
    let calls = Rc::new(Cell::new(0));
    let seen = calls.clone();
    first
        .on(&button, EventKind::Activate, move |_| {
            seen.set(seen.get() + 1);
            a.revise(|value| *value = "clicked".into());
        })
        .unwrap();
    app.flush().unwrap();
    let old = first.node(&button).unwrap();
    app.dispatch(old, Event::Activate).unwrap();
    assert_eq!(calls.get(), 1);
    assert!(
        matches!(&app.tree().node(first.node(&text).unwrap()).unwrap().descriptor.view().props,Props::Text(v)if v.content=="clicked")
    );
    assert!(
        matches!(&app.tree().node(second.node(&text).unwrap()).unwrap().descriptor.view().props,Props::Text(v)if v.content=="second")
    );
    first_owner.dispose();
    second_owner.dispose();
    app.flush().unwrap();
    assert!(first.node(&text).is_err());
    assert!(!app.dispatch(old, Event::Activate).unwrap());
    assert_eq!(app.stats().nodes, 0);
    assert_eq!(app.runtime_stats().nodes, baseline.nodes);
}

#[test]
fn plan_rejection_precedes_identity_allocation_and_foreign_parts_do_not_run_business_code() {
    let app = App::memory();
    let owner = app.scope().child().unwrap();
    let button_id = owner
        .create(Descriptor::new(Props::Button(ButtonProps::default())))
        .unwrap();
    let inside = owner.child_in(button_id).unwrap();
    let declaration = plan(group().fill(view())).unwrap();
    let before = (app.stats(), app.tree().stats());
    assert!(inside.instantiate(&declaration).is_err());
    assert_eq!((app.stats(), app.tree().stats()), before);
    let next = owner.create(Descriptor::new(Props::View)).unwrap();
    assert_eq!(next.slot(), button_id.slot() + 1);
    let one = plan(text().mark("text")).unwrap();
    let two = plan(text().mark("text")).unwrap();
    let instance = owner.instantiate(&one).unwrap();
    let calls = Rc::new(Cell::new(0));
    let seen = calls.clone();
    assert!(
        instance
            .bind(&two.part("text").unwrap(), "value".to_owned(), move |v| {
                seen.set(1);
                Patch::Text(Change::Set(v))
            })
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    let value = owner.cage("reactive".to_owned());
    assert!(plan(text().content(value)).is_err());
    assert!(plan(button().on(EventKind::Activate, |_| {})).is_err());
}
