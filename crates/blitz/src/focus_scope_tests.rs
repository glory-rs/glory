use super::focus_tests::{focus, key, settle};
use super::*;
use glory_render::{FocusMode, FocusScopeConfig, Semantics};
use glory_ui::{button, input, view};
fn focused(app: &WindowApp) -> Option<NodeId> {
    let state = app.renderer.0.borrow();
    state
        .dom
        .doc
        .get_focussed_node_id()
        .and_then(|id| state.dom.owner(id))
}

#[test]
fn native_focus_scope_loops_real_controls_restores_and_keeps_background_nonmodal() {
    let mut app = WindowApp::new();
    let active = app.app.scope().cage(false);
    let outside = app.app.scope().cage(None);
    let first = app.app.scope().cage(None);
    let last = app.app.scope().cage(None);
    app.mount(
        view().fill((
            button().label("Outside").node_ref(outside),
            view()
                .focus(FocusMode::Programmatic)
                .semantics(active.map(|active| {
                    Some(Box::new(Semantics {
                        focus_scope: Some(FocusScopeConfig {
                            active: *active,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .fill((
                    input().value("Draft").node_ref(first),
                    button().label("Last").node_ref(last),
                )),
        )),
    )
    .unwrap();
    settle(&mut app);
    focus(&mut app, outside.get().unwrap());
    active.revise(|value| *value = true);
    settle(&mut app);
    assert_eq!(focused(&app), first.get());
    assert!(app.renderer.inspect(|doc| doc.top_layers().is_empty()));
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), last.get());
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), first.get());
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::SHIFT);
    assert_eq!(focused(&app), last.get());
    assert!(
        app.renderer
            .clone()
            .action(glory_render::Action::Focus(outside.get().unwrap()))
            .is_err()
    );
    active.revise(|value| *value = false);
    settle(&mut app);
    assert_eq!(focused(&app), outside.get());
    assert!(app.renderer.0.borrow().focus_scopes.is_empty());
}

#[test]
fn native_focus_scope_no_auto_focus_and_uncontained_tab_can_leave() {
    let mut app = WindowApp::new();
    let active = app.app.scope().cage(false);
    let outside = app.app.scope().cage(None);
    let inside = app.app.scope().cage(None);
    app.mount(
        view().fill((
            view()
                .focus(FocusMode::Programmatic)
                .semantics(active.map(|active| {
                    Some(Box::new(Semantics {
                        focus_scope: Some(FocusScopeConfig {
                            active: *active,
                            auto_focus: false,
                            loop_tab: false,
                            contain_focus: false,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .fill(input().node_ref(inside)),
            button().label("Outside").node_ref(outside),
        )),
    )
    .unwrap();
    settle(&mut app);
    focus(&mut app, outside.get().unwrap());
    active.revise(|value| *value = true);
    settle(&mut app);
    assert_eq!(focused(&app), outside.get());
    focus(&mut app, inside.get().unwrap());
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), outside.get());
    active.revise(|value| *value = false);
    settle(&mut app);
    assert_eq!(
        focused(&app),
        outside.get(),
        "closing an unfocused scope does not steal focus"
    );
}

#[test]
fn native_nested_focus_scope_owner_retirement_follows_the_original_return_chain() {
    use glory_ui::when;
    let mut app = WindowApp::new();
    let mounted = app.app.scope().cage(true);
    let outer_active = app.app.scope().cage(false);
    let inner_active = app.app.scope().cage(false);
    let outside = app.app.scope().cage(None);
    let outer_input = app.app.scope().cage(None);
    let inner_input = app.app.scope().cage(None);
    app.mount(view().fill((
        button().label("Outside").node_ref(outside),
        when(mounted, move || {
            view()
                .focus(FocusMode::Programmatic)
                .semantics(outer_active.map(|active| {
                    Some(Box::new(Semantics {
                        focus_scope: Some(FocusScopeConfig {
                            active: *active,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .fill((
                    input().node_ref(outer_input),
                    view()
                        .focus(FocusMode::Programmatic)
                        .semantics(inner_active.map(|active| {
                            Some(Box::new(Semantics {
                                focus_scope: Some(FocusScopeConfig {
                                    active: *active,
                                    ..Default::default()
                                }),
                                ..Default::default()
                            }))
                        }))
                        .fill(input().node_ref(inner_input)),
                ))
        }),
    )))
    .unwrap();
    settle(&mut app);
    focus(&mut app, outside.get().unwrap());
    outer_active.revise(|value| *value = true);
    settle(&mut app);
    assert_eq!(focused(&app), outer_input.get());
    inner_active.revise(|value| *value = true);
    settle(&mut app);
    assert_eq!(focused(&app), inner_input.get());
    mounted.revise(|value| *value = false);
    settle(&mut app);
    assert_eq!(focused(&app), outside.get());
    assert!(app.renderer.0.borrow().focus_scopes.is_empty());
}

#[test]
fn native_focus_scope_admits_a_current_anchored_popup_and_modal_policy_keeps_priority() {
    use glory_render::{AnchorPlacement, Layer, LayerPlacement, Role, SemanticId};
    let mut app = WindowApp::new();
    let active = app.app.scope().cage(false);
    let popup = app.app.scope().cage(false);
    let modal = app.app.scope().cage(false);
    let anchor = app.app.scope().cage(None);
    let inside = app.app.scope().cage(None);
    app.mount(
        view().fill((
            view()
                .focus(FocusMode::Programmatic)
                .semantics(active.map(|active| {
                    Some(Box::new(Semantics {
                        focus_scope: Some(FocusScopeConfig {
                            active: *active,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .fill(input().node_ref(anchor).semantics(Some(Box::new(Semantics {
                    id: Some(SemanticId::new("focus-scope-anchor").unwrap()),
                    ..Default::default()
                })))),
            view()
                .role(Role::Dialog)
                .name("Associated popup")
                .focus(FocusMode::Programmatic)
                .style(
                    glory_render::style::Style::new()
                        .width(glory_render::style::Length::Px(200.))
                        .height(glory_render::style::Length::Px(120.)),
                )
                .semantics(popup.map(move |open| {
                    Some(Box::new(Semantics {
                        id: Some(SemanticId::new("focus-scope-popup").unwrap()),
                        hidden: !*open,
                        layer: Some(Layer {
                            modal: modal.get(),
                            anchor: Some(SemanticId::new("focus-scope-anchor").unwrap()),
                            placement: LayerPlacement::Anchor(AnchorPlacement::default()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .fill(input().node_ref(inside)),
        )),
    )
    .unwrap();
    settle(&mut app);
    active.revise(|value| *value = true);
    settle(&mut app);
    assert_eq!(focused(&app), anchor.get());
    popup.revise(|value| *value = true);
    settle(&mut app);
    assert_eq!(
        focused(&app),
        inside.get(),
        "external current popup is admitted by its live anchor"
    );
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), anchor.get());
    modal.revise(|value| *value = true);
    settle(&mut app);
    focus(&mut app, inside.get().unwrap());
    assert!(
        app.renderer
            .clone()
            .action(glory_render::Action::Focus(anchor.get().unwrap()))
            .is_err()
    );
    assert_eq!(focused(&app), inside.get());
}

#[test]
fn native_focus_scope_disabled_candidates_fall_back_to_the_real_container() {
    let mut app = WindowApp::new();
    let active = app.app.scope().cage(false);
    let root = app.app.scope().cage(None);
    app.mount(
        view()
            .focus(FocusMode::Programmatic)
            .node_ref(root)
            .style(
                glory_render::style::Style::new()
                    .width(glory_render::style::Length::Px(200.))
                    .height(glory_render::style::Length::Px(120.)),
            )
            .semantics(active.map(|active| {
                Some(Box::new(Semantics {
                    focus_scope: Some(FocusScopeConfig {
                        active: *active,
                        ..Default::default()
                    }),
                    ..Default::default()
                }))
            }))
            .fill(button().label("Disabled").enabled(false)),
    )
    .unwrap();
    settle(&mut app);
    active.revise(|value| *value = true);
    settle(&mut app);
    assert_eq!(focused(&app), root.get());
    key(&mut app, Key::Named(NamedKey::Tab), Modifiers::empty());
    assert_eq!(focused(&app), root.get());
}
