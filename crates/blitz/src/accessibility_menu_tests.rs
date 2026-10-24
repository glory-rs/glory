use super::*;
use glory_render::{MenuItemKind, MenuItemSemantics, PopupKind, SemanticId, Semantics};

#[test]
fn menu_metadata_refines_actual_intrinsic_controls_and_exports_typed_popup() {
    let app = WindowApp::new();
    let checked = app.app.scope().cage(false);
    let owner = SemanticId::new("native-menu-model").unwrap();
    let check_owner = owner.clone();
    // A live semantic model without a Layer is supported; opening Menu layers
    // and menu input are separately rejected by the native adapter.
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
