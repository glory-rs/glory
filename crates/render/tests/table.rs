use glory_render::{memory::Memory, *};
fn part(part: DocumentPart) -> Descriptor {
    let mut d = Descriptor::new(Props::View);
    d.semantics = Some(Box::new(Semantics {
        document_part: Some(part),
        ..Default::default()
    }));
    d
}
#[test]
fn role_tables_accept_finite_coordinates_and_sort_only_on_real_column_headers() {
    for (role, table) in [
        (
            Role::Table,
            TableMetadata::Table {
                rows: Some(100_000_001),
                columns: Some(3),
            },
        ),
        (Role::Row, TableMetadata::Row { index: 100_000_000 }),
        (
            Role::Cell,
            TableMetadata::Cell {
                row: 10,
                column: 2,
                row_span: 1,
                column_span: 1,
                headers: vec![],
            },
        ),
    ] {
        let mut d = Descriptor::new(Props::View);
        d.role = Some(role);
        d.semantics = Some(Box::new(Semantics {
            table: Some(table),
            ..Default::default()
        }));
        assert!(d.validate().is_ok());
        d.semantics.as_mut().unwrap().sort = Some(SortDirection::Ascending);
        assert!(d.validate().is_err());
    }
    let mut d = Descriptor::new(Props::View);
    d.role = Some(Role::ColumnHeader);
    d.semantics = Some(Box::new(Semantics {
        sort: Some(SortDirection::Ascending),
        ..Default::default()
    }));
    assert!(d.validate().is_ok());
    d.semantics.as_mut().unwrap().table = Some(TableMetadata::Cell {
        row: 0,
        column: 0,
        row_span: 1,
        column_span: 1,
        headers: vec![SemanticId::new("header").unwrap()],
    });
    assert!(d.validate().is_err());
}
fn create(tree: &mut Memory, ids: &mut NodeIds, part: DocumentPart) -> NodeId {
    let id = ids.allocate();
    tree.apply(&[Mutation::Create {
        id,
        node: Box::new(self::part(part)),
    }])
    .unwrap();
    id
}
fn place(tree: &mut Memory, node: NodeId, parent: Option<NodeId>) -> Result<Receipt, Error> {
    tree.apply(&[Mutation::Place {
        id: node,
        parent,
        before: None,
    }])
}
#[test]
fn table_parts_validate_nearest_boxed_parent_including_transparent_groups() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let table = create(&mut tree, &mut ids, DocumentPart::Table);
    place(&mut tree, table, None).unwrap();
    let body = create(&mut tree, &mut ids, DocumentPart::TableBody);
    place(&mut tree, body, Some(table)).unwrap();
    let row = create(&mut tree, &mut ids, DocumentPart::TableRow);
    place(&mut tree, row, Some(body)).unwrap();
    let cell = create(&mut tree, &mut ids, DocumentPart::TableCell);
    place(&mut tree, cell, Some(row)).unwrap();
    assert!(place(&mut tree, cell, Some(table)).is_err());
    assert!(place(&mut tree, row, Some(table)).is_err());
    assert!(place(&mut tree, body, Some(row)).is_err());
    assert_eq!(tree.node(cell).unwrap().parent, Some(row));
    let group = ids.allocate();
    tree.apply(&[Mutation::Create {
        id: group,
        node: Box::new(Descriptor::new(Props::Group)),
    }])
    .unwrap();
    place(&mut tree, group, Some(row)).unwrap();
    place(&mut tree, cell, Some(group)).unwrap();
    assert!(place(&mut tree, group, Some(body)).is_err());
    assert_eq!(tree.node(group).unwrap().parent, Some(row));
}
#[test]
fn every_table_metadata_field_is_independently_bounded_and_cannot_annotate_other_nodes() {
    let cell = TableMetadata::Cell {
        row: 0,
        column: 0,
        row_span: 1,
        column_span: 1,
        headers: vec![SemanticId::new("header").unwrap()],
    };
    let mut d = part(DocumentPart::TableCell);
    d.semantics.as_mut().unwrap().table = Some(cell.clone());
    d.validate().unwrap();
    for index in 0..8 {
        let mut value = cell.clone();
        let TableMetadata::Cell {
            row,
            column,
            row_span,
            column_span,
            headers,
        } = &mut value
        else {
            unreachable!()
        };
        match index {
            0 => *row = u32::MAX,
            1 => *column = u16::MAX,
            2 => *row_span = 0,
            3 => *column_span = 0,
            4 => *row_span = 1001,
            5 => *column_span = 1001,
            6 => *headers = vec![SemanticId::new("same").unwrap(); 2],
            _ => {
                *headers = (0..33)
                    .map(|i| SemanticId::new(format!("h{i}")).unwrap())
                    .collect()
            }
        }
        d.semantics.as_mut().unwrap().table = Some(value);
        assert!(d.validate().is_err());
    }
    d.semantics.as_mut().unwrap().table = Some(cell);
    d.semantics.as_mut().unwrap().document_part = Some(DocumentPart::Blockquote);
    assert!(d.validate().is_err());
    let mut root = part(DocumentPart::Table);
    root.semantics.as_mut().unwrap().table = Some(TableMetadata::Table {
        rows: Some(0),
        columns: Some(3),
    });
    root.validate().unwrap();
    root.semantics.as_mut().unwrap().table = Some(TableMetadata::Row { index: 0 });
    assert!(root.validate().is_err());
}
#[test]
fn live_metadata_patches_rollback_invalid_fields_and_keep_document_structure_immutable() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let mut tree = Memory::new(runtime);
    let id = create(&mut tree, &mut ids, DocumentPart::TableCell);
    let before = tree.node(id).unwrap().descriptor.view().semantics.clone();
    let receipt = tree.receipt();
    let invalid = Patch::Semantics(Change::Set(Some(Box::new(Semantics {
        document_part: Some(DocumentPart::TableCell),
        table: Some(TableMetadata::Row { index: 0 }),
        ..Default::default()
    }))));
    assert!(tree.prepare_patch(id, &invalid).is_err());
    assert_eq!(tree.node(id).unwrap().descriptor.view().semantics, &before);
    assert_eq!(tree.receipt(), receipt);
    let change = Patch::Semantics(Change::Set(Some(Box::new(Semantics {
        document_part: Some(DocumentPart::ColumnHeader),
        ..Default::default()
    }))));
    assert!(tree.prepare_patch(id, &change).is_err());
}

#[test]
fn each_table_part_rejects_control_role_focus_and_form_impersonation() {
    for kind in [
        DocumentPart::Table,
        DocumentPart::TableCaption,
        DocumentPart::TableHead,
        DocumentPart::TableBody,
        DocumentPart::TableFoot,
        DocumentPart::TableRow,
        DocumentPart::ColumnHeader,
        DocumentPart::RowHeader,
        DocumentPart::TableCell,
    ] {
        part(kind).validate().unwrap();
        let mut d = part(kind);
        d.role = Some(Role::Group);
        assert!(d.validate().is_err());
        let mut d = part(kind);
        d.focus = FocusMode::TabStop;
        assert!(d.validate().is_err());
        let mut d = part(kind);
        d.props = Props::Input(Default::default());
        assert!(d.validate().is_err());
        let mut d = part(kind);
        d.semantics.as_mut().unwrap().field_part = Some(FieldPart::Form);
        assert!(d.validate().is_err());
    }
}
