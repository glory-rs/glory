use super::*;
use glory_render::{Current, Semantics};

#[test]
fn checked_tree_items_export_each_toggled_state_and_clear_without_list_selection() {
    use glory_render::{TreeCheckState, TreeItem, TreeNavigation};
    let host = WindowApp::new();
    let checked = host.app.scope().cage(Some(TreeCheckState::Unchecked));
    host.mount(
        view()
            .role(Role::Tree)
            .name("Checks")
            .semantics(Some(Box::new(Semantics {
                tree_navigation: Some(TreeNavigation {
                    multiple: true,
                    ..Default::default()
                }),
                ..Default::default()
            })))
            .fill(
                view()
                    .role(Role::TreeItem)
                    .name("Checked folder")
                    .semantics(host.app.scope().bond(move || {
                        Some(Box::new(Semantics {
                            tree_item: Some(TreeItem {
                                level: 1,
                                position: 0,
                                count: 1,
                            }),
                            tree_checked: checked.get(),
                            ..Default::default()
                        }))
                    })),
            ),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&host, true), true);
    let identity = nodes(&tree)
        .iter()
        .find(|node| node.data().label() == Some("Checked folder"))
        .unwrap()
        .id();
    for value in [
        Some(TreeCheckState::Unchecked),
        Some(TreeCheckState::Checked),
        Some(TreeCheckState::Mixed),
        None,
    ] {
        checked.revise(|old| *old = value);
        host.app.flush().unwrap();
        tree.update_and_process_changes(build(&host, false), &mut Changes);
        let all = nodes(&tree);
        let node = all.iter().find(|node| node.id() == identity).unwrap();
        assert_eq!(node.role(), AxRole::TreeItem);
        assert_eq!(
            node.data().toggled(),
            value.map(|value| match value {
                TreeCheckState::Unchecked => accesskit::Toggled::False,
                TreeCheckState::Checked => accesskit::Toggled::True,
                TreeCheckState::Mixed => accesskit::Toggled::Mixed,
            })
        );
        assert_eq!(node.data().is_selected(), None);
    }
}

#[test]
fn shell_landmarks_and_toolbar_keep_their_intrinsic_accesskit_roles() {
    let host = WindowApp::new();
    host.mount(
        view().fill((
            view().role(Role::Banner).name("Application header"),
            view().role(Role::ContentInfo).name("Application footer"),
            view()
                .role(Role::Toolbar)
                .name("Editing")
                .semantics(Some(Box::new(Semantics {
                    navigation: Some(glory_render::Orientation::Vertical),
                    ..Default::default()
                })))
                .fill(button().label("Save")),
        )),
    )
    .unwrap();
    let tree = Consumer::new(build(&host, true), true);
    let all = nodes(&tree);
    for role in [AxRole::Banner, AxRole::ContentInfo, AxRole::Toolbar] {
        assert!(all.iter().any(|node| node.role() == role));
    }
    assert_eq!(
        all.iter()
            .find(|node| node.role() == AxRole::Toolbar)
            .unwrap()
            .data()
            .orientation(),
        Some(accesskit::Orientation::Vertical)
    );
}

#[test]
fn busy_updates_are_distinct_from_disabled_and_reset_in_the_live_accesskit_tree() {
    let host = WindowApp::new();
    let busy = host.app.scope().cage(Some(true));
    host.mount(
        button()
            .label("Save")
            .semantics(host.app.scope().bond(move || {
                Some(Box::new(Semantics {
                    busy: busy.get(),
                    ..Default::default()
                }))
            })),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&host, true), true);
    assert!(nodes(&tree).iter().any(|node| node.role() == AxRole::Button
        && node.data().is_busy()
        && !node.data().is_disabled()));
    for value in [Some(false), None] {
        busy.revise(|old| *old = value);
        tree.update_and_process_changes(build(&host, false), &mut Changes);
        assert!(
            nodes(&tree)
                .iter()
                .any(|node| node.role() == AxRole::Button && !node.data().is_busy())
        );
    }
}

#[test]
fn current_page_and_step_are_accesskit_locations_independent_of_selection() {
    let app = WindowApp::new();
    let page = app.app.scope().cage(Some(Current::Page));
    app.mount(
        view()
            .role(Role::Navigation)
            .name("Application navigation")
            .fill((
                link()
                    .target("/docs")
                    .label("Docs")
                    .semantics(app.app.scope().bond(move || {
                        Some(Box::new(Semantics {
                            current: page.get(),
                            ..Default::default()
                        }))
                    })),
                text()
                    .content("Step two")
                    .semantics(Some(Box::new(Semantics {
                        current: Some(Current::Step),
                        ..Default::default()
                    }))),
            )),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&app, true), true);
    let all = nodes(&tree);
    let page_node = all.iter().find(|node| node.role() == AxRole::Link).unwrap();
    assert_eq!(
        page_node.data().aria_current(),
        Some(accesskit::AriaCurrent::Page)
    );
    assert_eq!(page_node.data().is_selected(), None);
    assert!(
        all.iter()
            .any(|node| node.data().aria_current() == Some(accesskit::AriaCurrent::Step))
    );
    assert!(all.iter().any(|node| node.role() == AxRole::Navigation));
    drop(all);
    page.revise(|value| *value = Some(Current::Location));
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert!(nodes(&tree).iter().any(|node| node.role() == AxRole::Link
        && node.data().aria_current() == Some(accesskit::AriaCurrent::Location)
        && node.data().is_selected().is_none()));
    page.revise(|value| *value = None);
    tree.update_and_process_changes(build(&app, false), &mut Changes);
    assert!(
        nodes(&tree)
            .iter()
            .filter(|node| node.role() == AxRole::Link)
            .all(|node| node.data().aria_current().is_none())
    );
}

#[test]
fn grid_cells_expose_real_grid_selection_coordinates_and_today_independently() {
    use glory_render::{Direction, TableMetadata};
    let host = WindowApp::new();
    let selected = host.app.scope().cage(true);
    let multiple = host.app.scope().cage(true);
    host.mount(
        view()
            .role(Role::Grid)
            .name("February 2024")
            .semantics(host.app.scope().bond(move || {
                Some(Box::new(Semantics {
                    grid_multiple: multiple.get(),
                    grid_navigation: Some(Direction::Ltr),
                    table: Some(TableMetadata::Table {
                        rows: Some(6),
                        columns: Some(7),
                    }),
                    ..Default::default()
                }))
            }))
            .fill(
                view().role(Role::Row).fill(
                    view()
                        .role(Role::GridCell)
                        .semantics(host.app.scope().bond(move || {
                            Some(Box::new(Semantics {
                                selected: Some(selected.get()),
                                table: Some(TableMetadata::Cell {
                                    row: 4,
                                    column: 3,
                                    row_span: 1,
                                    column_span: 1,
                                    headers: vec![],
                                }),
                                ..Default::default()
                            }))
                        }))
                        .fill(
                            button()
                                .label("29")
                                .name("Thursday, February 29, 2024")
                                .semantics(Some(Box::new(Semantics {
                                    current: Some(Current::Date),
                                    ..Default::default()
                                }))),
                        ),
                ),
            ),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&host, true), true);
    let all = nodes(&tree);
    let grid = all.iter().find(|n| n.role() == AxRole::Grid).unwrap();
    assert!(grid.data().is_multiselectable());
    assert_eq!(grid.data().row_count(), Some(6));
    assert_eq!(grid.data().column_count(), Some(7));
    let cell = all.iter().find(|n| n.role() == AxRole::GridCell).unwrap();
    assert_eq!(cell.data().is_selected(), Some(true));
    assert_eq!(cell.data().row_index(), Some(4));
    assert_eq!(cell.data().column_index(), Some(3));
    assert!(all.iter().any(|n| n.role() == AxRole::Button
        && n.data().aria_current() == Some(accesskit::AriaCurrent::Date)
        && n.data().is_selected().is_none()));
    drop(all);
    multiple.revise(|v| *v = false);
    selected.revise(|v| *v = false);
    tree.update_and_process_changes(build(&host, false), &mut Changes);
    assert!(
        nodes(&tree)
            .iter()
            .any(|n| n.role() == AxRole::Grid && !n.data().is_multiselectable())
    );
    assert!(
        nodes(&tree)
            .iter()
            .any(|n| n.role() == AxRole::GridCell && n.data().is_selected() == Some(false))
    );
}

#[test]
fn clipped_zero_sized_text_remains_an_accessible_description_and_button_name() {
    use glory_render::{
        SemanticId,
        style::{Display, Edges, Overflow, Position},
    };
    let host = WindowApp::new();
    let detail = host.app.scope().cage("Hidden detail".to_string());
    let key = SemanticId::new("clipped-description").unwrap();
    let clipped = Style::new()
        .position(Position::Absolute)
        .display(Display::Block)
        .width(Length::Px(0.0))
        .height(Length::Px(0.0))
        .padding(Edges::all(0.0))
        .overflow_x(Overflow::Hidden)
        .overflow_y(Overflow::Hidden);
    host.mount(
        view().fill((
            text()
                .content(detail)
                .style(clipped.clone())
                .semantics(Some(Box::new(Semantics {
                    id: Some(key.clone()),
                    ..Default::default()
                }))),
            button().label("Save").semantics(Some(Box::new(Semantics {
                described_by: Some(key),
                ..Default::default()
            }))),
            button().fill(text().content("Invisible action name").style(clipped)),
        )),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&host, true), true);
    let all = nodes(&tree);
    let save = all
        .iter()
        .find(|node| node.role() == AxRole::Button && node.label().as_deref() == Some("Save"))
        .unwrap();
    assert_eq!(save.description().as_deref(), Some("Hidden detail"));
    assert!(all.iter().any(|node| node.role() == AxRole::Button
        && node.label().as_deref() == Some("Invisible action name")));
    drop(all);
    detail.revise(|value| *value = "New hidden detail".into());
    tree.update_and_process_changes(build(&host, false), &mut Changes);
    assert!(nodes(&tree).iter().any(|node| node.role() == AxRole::Button
        && node.description().as_deref() == Some("New hidden detail")));
}

#[test]
fn flat_tree_levels_lower_to_live_groups_with_container_sizes_and_nonrecycled_group_ids() {
    use glory_render::{FocusMode, TreeItem, TreeNavigation};
    let host = WindowApp::new();
    let open = host.app.scope().cage(true);
    host.mount(
        view()
            .role(Role::Tree)
            .name("Files")
            .semantics(Some(Box::new(Semantics {
                tree_navigation: Some(TreeNavigation {
                    multiple: true,
                    ..Default::default()
                }),
                ..Default::default()
            })))
            .fill((
                view()
                    .role(Role::TreeItem)
                    .name("Source")
                    .focus(FocusMode::TabStop)
                    .semantics(host.app.scope().bond(move || {
                        Some(Box::new(Semantics {
                            tree_item: Some(TreeItem {
                                level: 1,
                                position: 0,
                                count: 2,
                            }),
                            expanded: Some(open.get()),
                            selected: Some(false),
                            ..Default::default()
                        }))
                    })),
                glory_ui::when(open, || {
                    view()
                        .role(Role::TreeItem)
                        .name("Main")
                        .semantics(Some(Box::new(Semantics {
                            tree_item: Some(TreeItem {
                                level: 2,
                                position: 0,
                                count: 1,
                            }),
                            selected: Some(true),
                            ..Default::default()
                        })))
                }),
                view()
                    .role(Role::TreeItem)
                    .name("Tests")
                    .semantics(Some(Box::new(Semantics {
                        tree_item: Some(TreeItem {
                            level: 1,
                            position: 1,
                            count: 2,
                        }),
                        selected: Some(false),
                        ..Default::default()
                    }))),
            )),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&host, true), true);
    let all = nodes(&tree);
    let root = all.iter().find(|node| node.role() == AxRole::Tree).unwrap();
    assert_eq!(root.data().size_of_set(), Some(2));
    assert!(root.data().is_multiselectable());
    let source = all
        .iter()
        .find(|node| node.data().label() == Some("Source"))
        .unwrap();
    let source_id = source.id();
    let main = all
        .iter()
        .find(|node| node.data().label() == Some("Main"))
        .unwrap();
    assert_eq!(main.data().level(), Some(1));
    assert_eq!(main.data().position_in_set(), Some(0));
    assert_eq!(main.data().size_of_set(), None);
    assert_eq!(main.data().is_selected(), Some(true));
    let group = all
        .iter()
        .find(|node| node.role() == AxRole::Group && node.data().size_of_set() == Some(1))
        .unwrap();
    let group_id = group.id();
    assert_eq!(group.parent().map(|node| node.id()), Some(source_id));
    assert_eq!(main.parent().map(|node| node.id()), Some(group_id));
    drop(all);
    open.revise(|value| *value = false);
    tree.update_and_process_changes(build(&host, false), &mut Changes);
    assert!(!nodes(&tree).iter().any(|node| node.id() == group_id));
    assert!(nodes(&tree).iter().any(|node| node.id() == source_id));
    open.revise(|value| *value = true);
    tree.update_and_process_changes(build(&host, false), &mut Changes);
    let all = nodes(&tree);
    let group = all
        .iter()
        .find(|node| node.role() == AxRole::Group && node.data().size_of_set() == Some(1))
        .unwrap();
    assert_ne!(group.id(), group_id);
}
