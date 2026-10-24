use glory_render::{Props, Role, TableMetadata};
use glory_ui::{App, list, text, view};
use std::{cell::Cell, rc::Rc};

#[test]
fn virtual_table_rows_are_window_bounded_and_use_table_coordinates_without_list_metadata() {
    let app = App::memory();
    let calls = Rc::new(Cell::new(0));
    let keys = calls.clone();
    app.mount(
        view().role(Role::Table).fill(
            list(
                100_000_000usize,
                move |index| {
                    keys.set(keys.get() + 1);
                    index
                },
                |_, key| {
                    view()
                        .role(Role::Cell)
                        .fill(text().content(format!("Row {key}")))
                },
            )
            .table_rows(1u32)
            .height(120.)
            .row_height(24.)
            .overscan(1)
            .row_limit(8),
        ),
    )
    .unwrap();
    app.flush().unwrap();
    assert!(calls.get() <= 8);
    let tree = app.tree();
    let mut todo = tree.roots().to_vec();
    let mut rows = Vec::new();
    while let Some(id) = todo.pop() {
        let node = tree.node(id).unwrap();
        let d = node.descriptor.view();
        assert!(!matches!(d.role, Some(Role::List | Role::ListItem)));
        if d.role == Some(Role::Row) {
            assert!(d.item.is_none());
            let s = d.semantics.as_ref().unwrap();
            let Some(TableMetadata::Row { index }) = s.table else {
                panic!("row metadata")
            };
            rows.push(index);
        }
        todo.extend(&node.children);
    }
    rows.sort();
    assert_eq!(rows, (1..=6).collect::<Vec<_>>());
    drop(tree);
    app.clear().unwrap();
    assert_eq!(app.stats().nodes, 0);
}
#[test]
fn exhausted_future_table_coordinates_leave_accepted_rows_intact_without_binding_overflow() {
    let app = App::memory();
    let offset = app.scope().cage(1u32);
    app.mount(
        list(10usize, |index| index, |_, _| text().content("Cell"))
            .table_rows(offset)
            .height(120.)
            .row_height(24.)
            .overscan(1),
    )
    .unwrap();
    app.flush().unwrap();
    offset.revise(|value| *value = u32::MAX);
    assert!(app.flush().is_err());
    let tree = app.tree();
    let mut todo = tree.roots().to_vec();
    while let Some(id) = todo.pop() {
        let node = tree.node(id).unwrap();
        let d = node.descriptor.view();
        if let Props::View = d.props
            && let Some(TableMetadata::Row { index }) =
                d.semantics.as_ref().and_then(|s| s.table.as_ref())
        {
            assert!(*index < 8);
        }
        todo.extend(&node.children);
    }
}
