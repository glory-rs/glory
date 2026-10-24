use super::*;
use glory_render::{FocusMode, Layer, SemanticId, Semantics};

#[test]
fn native_environment_exports_inherited_language_and_nested_override_after_clear() {
    use glory_render::{Direction, Environment, LanguageTag};
    let app = WindowApp::new();
    let override_language = app.app.scope().cage(true);
    app.mount(
        view()
            .semantics(Some(Box::new(Semantics {
                environment: Some(Environment {
                    direction: Direction::Rtl,
                    language: LanguageTag::new("ar").unwrap(),
                }),
                ..Default::default()
            })))
            .fill((
                button().label("Inherited language"),
                view()
                    .semantics(override_language.map(|enabled| {
                        enabled.then(|| {
                            Box::new(Semantics {
                                environment: Some(Environment {
                                    direction: Direction::Ltr,
                                    language: LanguageTag::new("en-US").unwrap(),
                                }),
                                ..Default::default()
                            })
                        })
                    }))
                    .fill(input().name("Overridden language")),
            )),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&app, true), true);
    {
        let all = nodes(&tree);
        let inherited = all
            .iter()
            .find(|node| node.label().as_deref() == Some("Inherited language"))
            .unwrap();
        let nested = all
            .iter()
            .find(|node| node.label().as_deref() == Some("Overridden language"))
            .unwrap();
        assert_eq!(inherited.data().language(), Some("ar"));
        assert_eq!(nested.data().language(), Some("en-US"));
    }
    override_language.revise(|value| *value = false);
    app.app.settle(12).unwrap();
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    let all = nodes(&tree);
    let nested = all
        .iter()
        .find(|node| node.label().as_deref() == Some("Overridden language"))
        .unwrap();
    assert_eq!(nested.data().language(), Some("ar"));
}

#[test]
fn native_focus_scope_keeps_background_ax_and_click_but_limits_focus_actions() {
    let app = WindowApp::new();
    let active = app.app.scope().cage(false);
    let calls = app.app.scope().cage(0usize);
    let outside = app.app.scope().cage(None);
    let editor = app.app.scope().cage(None);
    app.mount(
        view().fill((
            button()
                .label("Scope background action")
                .node_ref(outside)
                .on(glory_render::EventKind::Activate, move |_| {
                    calls.revise(|value| *value += 1)
                }),
            view()
                .focus(FocusMode::Programmatic)
                .semantics(active.map(|active| {
                    Some(Box::new(Semantics {
                        focus_scope: Some(glory_render::FocusScopeConfig {
                            active: *active,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .fill(input().name("Scoped editor").node_ref(editor)),
        )),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&app, true), true);
    active.revise(|value| *value = true);
    app.app.settle(12).unwrap();
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    let all = nodes(&tree);
    let outside_node = all
        .iter()
        .find(|node| node.label().as_deref() == Some("Scope background action"))
        .unwrap();
    assert!(outside_node.data().supports_action(AxAction::Click));
    assert!(!outside_node.data().supports_action(AxAction::Focus));
    assert!(all.iter().all(|node| !node.data().is_modal()));
    assert_eq!(
        tree.state().focus().unwrap().id(),
        tree.state()
            .node_by_tree_local_id(id(&app, editor.get().unwrap()), TreeId::ROOT)
            .unwrap()
            .id()
    );
    assert!(
        action(
            &app,
            &request(id(&app, outside.get().unwrap()), AxAction::Click, None)
        )
        .unwrap()
    );
    app.app.settle(12).unwrap();
    assert_eq!(calls.get(), 1);
}

#[test]
fn native_modal_ax_exports_real_dialog_focus_only_current_scope_and_rejects_late_background_actions()
 {
    let app = WindowApp::new();
    let open = app.app.scope().cage(false);
    let calls = app.app.scope().cage(0usize);
    let outside = app.app.scope().cage(None);
    let editor = app.app.scope().cage(None);
    let dialog = app.app.scope().cage(None);
    app.mount(
        view().name("Application").fill((
            button()
                .label("Outside action")
                .node_ref(outside)
                .on(glory_render::EventKind::Activate, move |_| {
                    calls.revise(|v| *v += 1)
                }),
            view()
                .name("Decision")
                .role(Role::Dialog)
                .focus(FocusMode::Programmatic)
                .node_ref(dialog)
                .style(
                    Style::new()
                        .width(Length::Px(240.))
                        .height(Length::Px(160.)),
                )
                .semantics(open.map(|open| {
                    Some(Box::new(Semantics {
                        id: Some(SemanticId::new("ax-modal").unwrap()),
                        hidden: !*open,
                        layer: Some(Layer::default()),
                        ..Default::default()
                    }))
                }))
                .fill(input().name("Decision editor").node_ref(editor)),
        )),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&app, true), true);
    let previous = id(&app, outside.get().unwrap());
    open.revise(|v| *v = true);
    app.app.settle(12).unwrap();
    // The live modality check also rejects an old AT id before its next tree
    // update retires that mapping. No application callback is invoked.
    assert!(!action(&app, &request(previous, AxAction::Click, None)).unwrap());
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    let all = nodes(&tree);
    let modal = all
        .iter()
        .find(|node| node.role() == AxRole::Dialog)
        .unwrap();
    assert_eq!(modal.label().as_deref(), Some("Decision"));
    assert!(modal.data().is_modal());
    assert!(
        all.iter()
            .all(|node| node.label().as_deref() != Some("Outside action"))
    );
    assert_eq!(
        tree.state().focus().unwrap().id(),
        tree.state()
            .node_by_tree_local_id(id(&app, editor.get().unwrap()), TreeId::ROOT)
            .unwrap()
            .id()
    );
    assert_eq!(calls.get(), 0);
    drop(all);
    open.revise(|v| *v = false);
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert!(
        nodes(&tree)
            .iter()
            .all(|node| node.role() != AxRole::Dialog)
    );
    assert_ne!(
        id(&app, outside.get().unwrap()),
        previous,
        "reactivated AX mappings are never recycled"
    );
    assert!(!action(&app, &request(previous, AxAction::Click, None)).unwrap());
}
