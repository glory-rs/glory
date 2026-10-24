use glory_render::{Change, Patch, Props};
use glory_ui::*;
use std::{cell::Cell, rc::Rc};

#[test]
fn builder_tracks_only_its_binding_switches_edges_and_keeps_identity_on_equal_output() {
    let app = App::memory();
    let choose = app.scope().cage(true);
    let left = app.scope().cage("left".to_owned());
    let right = app.scope().cage("right".to_owned());
    let calls = Rc::new(Cell::new(0));
    let counter = calls.clone();
    app.mount(text().content_fn(move || {
        counter.set(counter.get() + 1);
        if choose.get() {
            left.get()
        } else {
            right.get()
        }
    }))
    .unwrap();
    let id = app.tree().roots()[0];
    assert_eq!(app.graph_stats().nodes[1], 0);
    let before = app.tree().stats().patched;
    let count = calls.get();
    right.revise(|v| *v = "other".into());
    app.flush().unwrap();
    assert_eq!(calls.get(), count);
    choose.revise(|v| *v = false);
    app.flush().unwrap();
    assert_eq!(app.tree().roots(), &[id]);
    let Props::Text(props) = app.tree().node(id).unwrap().descriptor.view().props.clone() else {
        panic!()
    };
    assert_eq!(props.content, "other");
    let count = calls.get();
    left.revise(|v| *v = "unobserved".into());
    app.flush().unwrap();
    assert_eq!(calls.get(), count);
    let patched = app.tree().stats().patched;
    right.revise(|_| {});
    app.flush().unwrap();
    assert_eq!(app.tree().stats().patched, patched);
    assert_eq!(patched, before + 1);
    app.clear().unwrap();
    right.revise(|v| *v = "retired".into());
    let count = calls.get();
    app.flush().unwrap();
    assert_eq!(calls.get(), count);
}

#[test]
fn invalid_computed_initial_style_cannot_allocate_semantic_identity_or_queue_a_patch() {
    let app = App::memory();
    let before = app.stats();
    assert!(
        app.mount(
            text().content("x").style_fn(|| {
                glory_style::Style::new().width(glory_style::Length::Px(f64::NAN))
            })
        )
        .is_err()
    );
    assert_eq!(app.stats().nodes, before.nodes);
    assert_eq!(app.stats().node_slots, before.node_slots);
    assert_eq!(app.stats().pending, before.pending);
    assert_eq!(app.tree().stats().created, 0);
}

#[test]
fn computed_plan_input_uses_live_revisions_and_reports_exhaustion_without_changing_value() {
    let app = App::memory();
    let value = app.scope().cage("seed".to_owned());
    let declaration = plan(input().mark("editor")).unwrap();
    let part = declaration.part("editor").unwrap();
    let instance = app.scope().instantiate(&declaration).unwrap();
    instance
        .bind_fn(
            &part,
            move || value.get(),
            |s| Patch::Value {
                value: Change::Set(s),
                revision: 0,
            },
        )
        .unwrap();
    app.flush().unwrap();
    let id = instance.node(&part).unwrap();
    instance
        .patch(
            &part,
            Patch::Value {
                value: Change::Set("explicit".into()),
                revision: 42,
            },
        )
        .unwrap();
    value.revise(|v| *v = "next".into());
    app.flush().unwrap();
    let get = || match app.tree().node(id).unwrap().descriptor.view().props {
        Props::Input(p) => (p.value.clone(), p.revision),
        _ => panic!(),
    };
    assert_eq!(get(), ("next".into(), 43));
    instance
        .patch(
            &part,
            Patch::Value {
                value: Change::Set("terminal".into()),
                revision: u64::MAX,
            },
        )
        .unwrap();
    app.flush().unwrap();
    value.revise(|v| *v = "rejected".into());
    assert!(app.flush().is_err());
    assert_eq!(get(), ("terminal".into(), u64::MAX));
}
