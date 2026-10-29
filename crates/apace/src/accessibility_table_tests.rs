use super::*;
use glory_render::{Change, DocumentPart, Patch, Semantics, TableMetadata};
fn metadata(part: DocumentPart, table: Option<TableMetadata>) -> Option<Box<Semantics>> {
    Some(Box::new(Semantics {
        document_part: Some(part),
        table,
        ..Default::default()
    }))
}
#[test]
fn virtual_role_table_exports_sort_and_finite_coordinates_without_list_positions() {
    let host = WindowApp::new();
    let sorted = host
        .app
        .scope()
        .cage(glory_render::SortDirection::Ascending);
    host.mount(
        view()
            .role(glory_render::Role::Table)
            .name("Virtual data")
            .semantics(Some(Box::new(Semantics {
                table: Some(TableMetadata::Table {
                    rows: Some(101),
                    columns: Some(1),
                }),
                ..Default::default()
            })))
            .fill(
                view()
                    .role(glory_render::Role::Row)
                    .semantics(Some(Box::new(Semantics {
                        table: Some(TableMetadata::Row { index: 0 }),
                        ..Default::default()
                    })))
                    .fill(
                        view()
                            .role(glory_render::Role::ColumnHeader)
                            .semantics(sorted.map(|sort| {
                                Some(Box::new(Semantics {
                                    sort: Some(*sort),
                                    table: Some(TableMetadata::Cell {
                                        row: 0,
                                        column: 0,
                                        row_span: 1,
                                        column_span: 1,
                                        headers: vec![],
                                    }),
                                    ..Default::default()
                                }))
                            }))
                            .fill(text().content("Name")),
                    ),
            ),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&host, true), true);
    let id = {
        let all = nodes(&tree);
        let header = all
            .iter()
            .find(|n| n.role() == AxRole::ColumnHeader)
            .unwrap();
        assert_eq!(header.label().as_deref(), Some("Name"));
        assert_eq!(
            header.data().sort_direction(),
            Some(accesskit::SortDirection::Ascending)
        );
        assert_eq!(header.data().column_index(), Some(0));
        let table = all.iter().find(|n| n.role() == AxRole::Table).unwrap();
        assert_eq!(table.data().row_count(), Some(101));
        header.id()
    };
    sorted.revise(|sort| *sort = glory_render::SortDirection::None);
    host.app.flush().unwrap();
    tree.update_and_process_changes(build(&host, false), &mut Changes);
    let all = nodes(&tree);
    let header = all.iter().find(|node| node.id() == id).unwrap();
    assert_eq!(header.data().sort_direction(), None);
}
#[test]
fn intrinsic_table_exports_caption_header_cells_coordinates_and_restores_caption_after_name_clear()
{
    let host = WindowApp::new();
    let root = host.app.scope().cage(None);
    let caption = host.app.scope().cage("Live caption".to_owned());
    host.mount(
        view()
            .node_ref(root)
            .semantics(metadata(
                DocumentPart::Table,
                Some(TableMetadata::Table {
                    rows: Some(2),
                    columns: Some(1),
                }),
            ))
            .fill((
                view()
                    .document_part(DocumentPart::TableCaption)
                    .fill(text().content(caption)),
                view().document_part(DocumentPart::TableHead).fill(
                    view()
                        .semantics(metadata(
                            DocumentPart::TableRow,
                            Some(TableMetadata::Row { index: 0 }),
                        ))
                        .fill(
                            view()
                                .semantics(metadata(
                                    DocumentPart::ColumnHeader,
                                    Some(TableMetadata::Cell {
                                        row: 0,
                                        column: 0,
                                        row_span: 1,
                                        column_span: 1,
                                        headers: vec![],
                                    }),
                                ))
                                .fill(text().content("Heading")),
                        ),
                ),
                view().document_part(DocumentPart::TableBody).fill(
                    view()
                        .semantics(metadata(
                            DocumentPart::TableRow,
                            Some(TableMetadata::Row { index: 1 }),
                        ))
                        .fill(
                            view()
                                .semantics(metadata(
                                    DocumentPart::TableCell,
                                    Some(TableMetadata::Cell {
                                        row: 1,
                                        column: 0,
                                        row_span: 1,
                                        column_span: 1,
                                        headers: vec![],
                                    }),
                                ))
                                .fill(text().content("Value")),
                        ),
                ),
            )),
    )
    .unwrap();
    let mut tree = Consumer::new(build(&host, true), true);
    let table_id = {
        let all = nodes(&tree);
        let table = all.iter().find(|n| n.role() == AxRole::Table).unwrap();
        assert_eq!(table.label().as_deref(), Some("Live caption"));
        assert_eq!(table.data().row_count(), Some(2));
        assert_eq!(table.data().column_count(), Some(1));
        let header = all
            .iter()
            .find(|n| n.role() == AxRole::ColumnHeader)
            .unwrap();
        assert_eq!(header.label().as_deref(), Some("Heading"));
        assert_eq!(header.data().row_index(), Some(0));
        assert_eq!(header.data().column_index(), Some(0));
        let cell = all.iter().find(|n| n.role() == AxRole::Cell).unwrap();
        assert_eq!(cell.label().as_deref(), Some("Value"));
        assert_eq!(cell.data().row_index(), Some(1));
        table.id()
    };
    host.app
        .scope()
        .patch(
            root.get().unwrap(),
            Patch::Name(Change::Set(Some("Explicit".into()))),
        )
        .unwrap();
    host.app.flush().unwrap();
    tree.update_and_process_changes(build(&host, false), &mut Changes);
    assert_eq!(
        nodes(&tree)
            .into_iter()
            .find(|node| node.id() == table_id)
            .unwrap()
            .label()
            .as_deref(),
        Some("Explicit")
    );
    host.app
        .scope()
        .patch(root.get().unwrap(), Patch::Name(Change::Clear))
        .unwrap();
    caption.revise(|v| *v = "Caption revised".into());
    host.app.flush().unwrap();
    tree.update_and_process_changes(build(&host, false), &mut Changes);
    assert_eq!(
        nodes(&tree)
            .into_iter()
            .find(|node| node.id() == table_id)
            .unwrap()
            .label()
            .as_deref(),
        Some("Caption revised")
    );
}
