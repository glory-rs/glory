use super::*;
use glory_render::{MenuItemKind, MenuItemSemantics, PopupKind, SemanticId, Semantics};

#[test]
fn menu_metadata_refines_actual_intrinsic_controls_and_exports_typed_popup() {
    let app = WindowApp::new();
    let checked = app.app.scope().cage(false);
    let owner = SemanticId::new("native-menu-model").unwrap();
    let check_owner = owner.clone();
    // Intrinsic roles agree with both inline models and admitted Menu layers.
    let action = button()
        .label("Submenu action")
        .semantics(Some(Box::new(Semantics {
            popup: Some(PopupKind::Menu),
            expanded: Some(false),
            menu_item: Some(MenuItemSemantics {
                kind: MenuItemKind::Action,
                owner: owner.clone(),
            }),
            ..Default::default()
        })));
    let checkbox = check()
        .label("Check action")
        .checked(checked)
        .semantics(Some(Box::new(Semantics {
            menu_item: Some(MenuItemSemantics {
                kind: MenuItemKind::Checkbox,
                owner: check_owner,
            }),
            ..Default::default()
        })));
    app.mount(
        view()
            .role(Role::Menu)
            .name("Menu model")
            .semantics(Some(Box::new(Semantics {
                id: Some(owner),
                ..Default::default()
            })))
            .fill((action, checkbox)),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&app, true), true);
    let all = nodes(&tree);
    assert!(all.iter().any(|node| node.role() == AxRole::Menu));
    assert!(all.iter().any(|node| node.role() == AxRole::MenuItem
        && node.data().has_popup() == Some(accesskit::HasPopup::Menu)));
    let check = all
        .iter()
        .find(|node| node.role() == AxRole::MenuItemCheckBox)
        .unwrap()
        .id();
    assert_eq!(
        tree.state().node_by_id(check).unwrap().data().toggled(),
        Some(Toggled::False)
    );
    drop(all);
    checked.revise(|value| *value = true);
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert_eq!(
        tree.state().node_by_id(check).unwrap().data().toggled(),
        Some(Toggled::True)
    );
}

#[test]
fn actual_menu_layer_exports_only_live_modal_scope_and_retires_item_actions_after_close() {
    use crate::document::shell::menu_tests::modal_fixture;
    let f = modal_fixture(1.);
    let mut tree = Consumer::new(build(&f.host, true), true);
    let item = id(&f.host, f.item);
    let all = nodes(&tree);
    assert!(
        all.iter()
            .any(|n| n.role() == AxRole::Menu && n.label().as_deref() == Some("Context actions"))
    );
    let action_node = all.iter().find(|n| n.role() == AxRole::MenuItem).unwrap();
    assert!(action_node.data().supports_action(AxAction::Click));
    assert!(
        all.iter()
            .all(|n| n.label().as_deref() != Some("Background"))
    );
    assert!(action(&f.host, &request(item, AxAction::Click, None)).unwrap());
    build(&f.host, false);
    assert_eq!(f.picked.get(), 1);
    f.open.revise(|v| *v = false);
    drop(all);
    tree.update_and_process_changes(build(&f.host, false), &mut Changes);
    assert!(
        nodes(&tree)
            .iter()
            .all(|n| n.role() != AxRole::Menu && n.role() != AxRole::MenuItem)
    );
    assert!(!action(&f.host, &request(item, AxAction::Click, None)).unwrap());
    f.host.app.clear().unwrap();
    build(&f.host, false);
    assert!(!action(&f.host, &request(item, AxAction::Focus, None)).unwrap());
}

#[test]
fn native_menubar_ax_popup_relations_and_current_item_actions_follow_controlled_open() {
    use glory_render::{FocusMode, Layer, LayerFocus, LayerPlacement, Orientation};
    let host = WindowApp::new();
    let open = host.app.scope().cage(false);
    let trigger = host.app.scope().cage(None);
    let panel = host.app.scope().cage(None);
    host.mount(
        group().fill((
            view()
                .role(Role::Menubar)
                .name("Application actions")
                .semantics(Some(Box::new(Semantics {
                    id: Some(SemanticId::new("bar").unwrap()),
                    navigation: Some(Orientation::Horizontal),
                    ..Default::default()
                })))
                .fill(
                    button()
                        .label("File")
                        .node_ref(trigger)
                        .focus(FocusMode::TabStop)
                        .semantics(open.map(|open| {
                            Some(Box::new(Semantics {
                                id: Some(SemanticId::new("trigger").unwrap()),
                                navigation_item: true,
                                menubar_item: true,
                                menu_item: Some(MenuItemSemantics {
                                    kind: MenuItemKind::Action,
                                    owner: SemanticId::new("bar").unwrap(),
                                }),
                                popup: Some(PopupKind::Menu),
                                expanded: Some(*open),
                                controls: Some(SemanticId::new("panel").unwrap()),
                                ..Default::default()
                            }))
                        }))
                        .on(EventKind::Activate, move |_| open.revise(|v| *v = true)),
                ),
            view()
                .role(Role::Menu)
                .name("File actions")
                .focus(FocusMode::Programmatic)
                .node_ref(panel)
                .semantics(open.map(|open| {
                    Some(Box::new(Semantics {
                        id: Some(SemanticId::new("panel").unwrap()),
                        hidden: !*open,
                        navigation: Some(Orientation::Vertical),
                        layer: Some(Layer {
                            modal: false,
                            focus: LayerFocus::Manage,
                            placement: LayerPlacement::Anchor(glory_render::AnchorPlacement::new(
                                glory_render::AnchorSide::Bottom,
                                glory_render::AnchorAlign::Start,
                            )),
                            anchor: Some(SemanticId::new("trigger").unwrap()),
                            return_focus: Some(SemanticId::new("trigger").unwrap()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .fill(
                    button()
                        .label("New file")
                        .semantics(Some(Box::new(Semantics {
                            menu_item: Some(MenuItemSemantics {
                                kind: MenuItemKind::Action,
                                owner: SemanticId::new("panel").unwrap(),
                            }),
                            ..Default::default()
                        }))),
                ),
        )),
    )
    .unwrap();
    let first = build(&host, true);
    let trigger_id = id(&host, trigger.get().unwrap());
    let node = &first
        .nodes
        .iter()
        .find(|(id, _)| *id == trigger_id)
        .unwrap()
        .1;
    assert_eq!(node.role(), AxRole::MenuItem);
    assert_eq!(node.has_popup(), Some(accesskit::HasPopup::Menu));
    assert_eq!(node.is_expanded(), Some(false));
    assert!(
        first
            .nodes
            .iter()
            .any(|(_, node)| node.role() == AxRole::MenuBar)
    );
    assert!(action(&host, &request(trigger_id, AxAction::Click, None)).unwrap());
    let second = build(&host, true);
    let panel_id = id(&host, panel.get().unwrap());
    let node = &second
        .nodes
        .iter()
        .find(|(id, _)| *id == trigger_id)
        .unwrap()
        .1;
    assert_eq!(node.controls(), &[panel_id]);
    assert_eq!(node.is_expanded(), Some(true));
    open.revise(|v| *v = false);
    let third = build(&host, false);
    assert!(
        third
            .nodes
            .iter()
            .any(|(id, node)| *id == trigger_id && node.is_expanded() == Some(false))
    );
    assert!(!action(&host, &request(panel_id, AxAction::Focus, None)).unwrap());
}
