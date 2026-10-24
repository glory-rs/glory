use super::*;
use glory_render::{Current, Semantics};

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
    host.mount(
        view()
            .role(Role::Grid)
            .name("February 2024")
            .semantics(Some(Box::new(Semantics {
                grid_navigation: Some(Direction::Ltr),
                table: Some(TableMetadata::Table {
                    rows: Some(6),
                    columns: Some(7),
                }),
                ..Default::default()
            })))
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
    selected.revise(|v| *v = false);
    tree.update_and_process_changes(build(&host, false), &mut Changes);
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
