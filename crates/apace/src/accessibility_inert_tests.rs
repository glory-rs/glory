use super::*;
use glory_render::{Action, Semantics};
#[test]
fn inherited_inert_retires_the_complete_ax_branch_and_focus_without_replacing_engine_nodes() {
    let app = WindowApp::new();
    let blocked = app.app.scope().cage(false);
    let value = app.app.scope().cage("Retained text".to_string());
    app.mount(
        view().fill((
            view()
                .name("Work area")
                .semantics(blocked.map(|blocked| {
                    Some(Box::new(Semantics {
                        inert: *blocked,
                        ..Default::default()
                    }))
                }))
                .fill((
                    input().value(value).name("Editor"),
                    button().label("Inside"),
                )),
            button().label("Outside"),
        )),
    )
    .unwrap();
    let root = app.app.tree().roots()[0];
    let region = app.app.tree().node(root).unwrap().children[0];
    let input = app.app.tree().node(region).unwrap().children[0];
    let button = app.app.tree().node(region).unwrap().children[1];
    let mut tree = Consumer::new(build(&app, true), true);
    let count = nodes(&tree).len();
    let old_input = id(&app, input);
    let old_button = id(&app, button);
    let old_region = id(&app, region);
    let physical = app
        .renderer
        .engine_id(input, glory_html::Part::Main)
        .unwrap();
    app.app.scope().action(Action::Focus(input)).unwrap();
    app.app.flush().unwrap();
    assert_eq!(app.renderer.0.borrow().focused, Some(input));
    blocked.revise(|value| *value = true);
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert!(nodes(&tree).len() + 3 <= count);
    assert!(app.renderer.0.borrow().focused.is_none());
    for old in [old_region, old_input, old_button] {
        assert!(app.accessibility.borrow().ids.values().all(|id| *id != old));
        assert!(!action(&app, &request(old, AxAction::Focus, None)).unwrap());
    }
    assert_eq!(value.get(), "Retained text");
    assert_eq!(
        app.renderer
            .engine_id(input, glory_html::Part::Main)
            .unwrap(),
        physical
    );
    assert!(app.app.scope().action(Action::Focus(input)).is_err());
    blocked.revise(|value| *value = false);
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert_ne!(id(&app, input), old_input);
    assert_ne!(id(&app, button), old_button);
    assert_ne!(id(&app, region), old_region);
    assert!(app.renderer.0.borrow().focused.is_none());
    assert_eq!(
        app.renderer
            .engine_id(input, glory_html::Part::Main)
            .unwrap(),
        physical
    );
}
