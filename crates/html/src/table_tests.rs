use crate::*;
use glory_render::*;
#[derive(Default)]
struct Record(Vec<Op>);
impl Sink for Record {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        self.0.push(op);
        Ok(())
    }
}
fn document(part: DocumentPart, table: Option<TableMetadata>) -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.semantics = Some(Box::new(Semantics {
        document_part: Some(part),
        table,
        ..Default::default()
    }));
    d
}
#[test]
fn role_cell_coordinates_and_sort_direction_resets_match_the_full_shape_oracle() {
    for role in [Role::ColumnHeader, Role::Cell] {
        let mut base = Descriptor::new(Props::View);
        base.role = Some(role);
        base.semantics = Some(Box::new(Semantics {
            sort: (role == Role::ColumnHeader).then_some(SortDirection::Ascending),
            table: Some(TableMetadata::Cell {
                row: 2,
                column: 3,
                row_span: 1,
                column_span: 1,
                headers: vec![],
            }),
            ..Default::default()
        }));
        for sort in [
            Some(SortDirection::Descending),
            Some(SortDirection::None),
            None,
        ] {
            let mut next = base.clone();
            next.semantics.as_mut().unwrap().sort = if role == Role::ColumnHeader {
                sort
            } else {
                None
            };
            next.semantics.as_mut().unwrap().table = None;
            let expected = crate::difference(
                &shape(&base).unwrap(),
                &shape(&next).unwrap(),
                false,
                false,
                false,
            )
            .unwrap();
            let runtime = RuntimeId::new();
            let mut ids = NodeIds::new(runtime);
            let id = ids.allocate();
            let mut mapper = Mapper::new(runtime);
            let mut sink = Record::default();
            mapper
                .apply(
                    &[
                        Mutation::Create {
                            id,
                            node: Box::new(base.clone()),
                        },
                        Mutation::Place {
                            id,
                            parent: None,
                            before: None,
                        },
                    ],
                    &mut sink,
                )
                .unwrap();
            sink.0.clear();
            mapper
                .apply(
                    &[Mutation::Patch {
                        id,
                        patch: Patch::Semantics(Change::Set(next.semantics.clone())),
                    }],
                    &mut sink,
                )
                .unwrap();
            assert!(matches!(&sink.0[0],Op::Patch{edits,..} if edits==&expected));
        }
        let mut attrs = std::collections::BTreeMap::new();
        attrs.extend(crate::semantics::attributes(
            base.semantics.as_deref(),
            base.role,
        ));
        assert_eq!(attrs.get("aria-colspan").map(String::as_str), Some("1"));
        let runtime = RuntimeId::new();
        let mut mapper = Mapper::new(runtime);
        let mut sink = Record::default();
        let mut ids = NodeIds::new(runtime);
        let id = ids.allocate();
        mapper
            .apply(
                &[
                    Mutation::Create {
                        id,
                        node: Box::new(base.clone()),
                    },
                    Mutation::Place {
                        id,
                        parent: None,
                        before: None,
                    },
                ],
                &mut sink,
            )
            .unwrap();
        let html = mapper.html().unwrap();
        assert!(html.contains("aria-colspan=\"1\"") && html.contains("aria-rowindex=\"3\""));
        assert!(!html.contains(" headers="));
    }
}
#[test]
fn table_coordinates_headers_and_resets_match_full_shape_without_replacing_nodes() {
    for part in [
        DocumentPart::Table,
        DocumentPart::TableRow,
        DocumentPart::TableCell,
        DocumentPart::ColumnHeader,
        DocumentPart::RowHeader,
    ] {
        let values = match part {
            DocumentPart::Table => vec![
                Some(TableMetadata::Table {
                    rows: Some(3),
                    columns: Some(2),
                }),
                Some(TableMetadata::Table {
                    rows: Some(0),
                    columns: Some(5),
                }),
                None,
            ],
            DocumentPart::TableRow => vec![
                Some(TableMetadata::Row { index: 1 }),
                Some(TableMetadata::Row { index: 5 }),
                None,
            ],
            _ => vec![
                Some(TableMetadata::Cell {
                    row: 1,
                    column: 2,
                    row_span: 1,
                    column_span: 2,
                    headers: vec![
                        SemanticId::new("column").unwrap(),
                        SemanticId::new("row").unwrap(),
                    ],
                }),
                Some(TableMetadata::Cell {
                    row: 4,
                    column: 0,
                    row_span: 3,
                    column_span: 1,
                    headers: vec![],
                }),
                None,
            ],
        };
        let base = document(part, values[0].clone());
        for value in values.into_iter().skip(1) {
            let next = document(part, value);
            let patch = Patch::Semantics(Change::Set(next.semantics.clone()));
            let expected = crate::difference(
                &shape(&base).unwrap(),
                &shape(&next).unwrap(),
                false,
                false,
                false,
            )
            .unwrap();
            let runtime = RuntimeId::new();
            let mut ids = NodeIds::new(runtime);
            let table = ids.allocate();
            let body = ids.allocate();
            let row = ids.allocate();
            let cell = ids.allocate();
            let target = match part {
                DocumentPart::Table => table,
                DocumentPart::TableRow => row,
                _ => cell,
            };
            let mut mapper = Mapper::new(runtime);
            let mut sink = Record::default();
            let mut ops = vec![];
            for (id, node, parent) in [
                (
                    table,
                    if target == table {
                        base.clone()
                    } else {
                        document(DocumentPart::Table, None)
                    },
                    None,
                ),
                (body, document(DocumentPart::TableBody, None), Some(table)),
                (
                    row,
                    if target == row {
                        base.clone()
                    } else {
                        document(DocumentPart::TableRow, None)
                    },
                    Some(body),
                ),
                (
                    cell,
                    if target == cell {
                        base.clone()
                    } else {
                        document(DocumentPart::TableCell, None)
                    },
                    Some(row),
                ),
            ] {
                ops.push(Mutation::Create {
                    id,
                    node: Box::new(node),
                });
                ops.push(Mutation::Place {
                    id,
                    parent,
                    before: None,
                });
            }
            mapper.apply(&ops, &mut sink).unwrap();
            sink.0.clear();
            mapper
                .apply(&[Mutation::Patch { id: target, patch }], &mut sink)
                .unwrap();
            assert!(matches!(&sink.0[0],Op::Patch{edits,..}if edits==&expected));
            let html = mapper.html().unwrap();
            assert!(html.contains("<table") && html.contains("<tbody") && html.contains("<tr"));
            if part == DocumentPart::ColumnHeader {
                assert!(html.contains("scope=\"col\""));
            }
            if part == DocumentPart::RowHeader {
                assert!(html.contains("scope=\"row\""));
            }
        }
    }
}
