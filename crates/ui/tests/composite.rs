use glory_core::Cage;
use glory_render::{FocusMode, SemanticId, Semantics};
use glory_ui::{App, Widget, button, view};

#[test]
fn duplicate_names_fail_before_identity_allocation_and_focus_is_reactive() {
    let app = App::memory();
    let semantics = Some(Box::new(Semantics {
        id: Some(SemanticId::new("unique").unwrap()),
        ..Default::default()
    }));
    app.mount(view().semantics(semantics.clone())).unwrap();
    let before = app.stats();
    assert!(app.mount(view().semantics(semantics)).is_err());
    assert_eq!(app.stats().node_slots, before.node_slots);
    assert_eq!(app.stats().nodes, before.nodes);
    let reference = app.scope().cage(None);
    let focus: Cage<FocusMode> = app.scope().cage(FocusMode::Programmatic);
    button()
        .focus(focus)
        .node_ref(reference)
        .show_in(app.scope())
        .unwrap();
    app.flush().unwrap();
    let node = reference.get().unwrap();
    assert_eq!(
        app.tree().node(node).unwrap().descriptor.focus,
        FocusMode::Programmatic
    );
    focus.revise(|value| *value = FocusMode::TabStop);
    app.flush().unwrap();
    assert_eq!(
        app.tree().node(node).unwrap().descriptor.focus,
        FocusMode::TabStop
    );
}
