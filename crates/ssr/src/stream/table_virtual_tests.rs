use super::*;
use glory_render::{Role, Semantics, SortDirection, TableMetadata};
fn table() -> impl Widget {
    view()
        .role(Role::Table)
        .name("Stream records")
        .semantics(Some(Box::new(Semantics {
            table: Some(TableMetadata::Table {
                rows: Some(51),
                columns: Some(1),
            }),
            ..Default::default()
        })))
        .fill((
            view()
                .role(Role::Row)
                .semantics(Some(Box::new(Semantics {
                    table: Some(TableMetadata::Row { index: 0 }),
                    ..Default::default()
                })))
                .fill(
                    view()
                        .role(Role::ColumnHeader)
                        .semantics(Some(Box::new(Semantics {
                            sort: Some(SortDirection::Descending),
                            table: Some(TableMetadata::Cell {
                                row: 0,
                                column: 0,
                                row_span: 1,
                                column_span: 1,
                                headers: vec![],
                            }),
                            ..Default::default()
                        })))
                        .fill(text().content("Record")),
                ),
            list(
                50usize,
                |index| index,
                |index, key| {
                    view()
                        .role(Role::Cell)
                        .semantics(index.map(|index| {
                            Some(Box::new(Semantics {
                                table: Some(TableMetadata::Cell {
                                    row: *index as u32 + 1,
                                    column: 0,
                                    row_span: 1,
                                    column_span: 1,
                                    headers: vec![],
                                }),
                                ..Default::default()
                            }))
                        }))
                        .fill(text().content(format!("Record <&> {key}")))
                },
            )
            .table_rows(1u32)
            .height(72.)
            .row_height(24.)
            .overscan(1),
        ))
}
#[test]
fn windowed_table_coordinates_sort_and_finite_rows_survive_classic_patch_and_end() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let resource = resources
                .bind(
                    scope.owner(),
                    "data-table",
                    "v1",
                    || 1_u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            when(
                resource.map(|state| matches!(state, ResourceState::Ready { .. })),
                table,
            )
            .otherwise(|| text().content("pending records"))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("pending records"));
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    assert!(
        patch.body().contains("aria-sort")
            && patch.body().contains("descending")
            && patch.body().contains("aria-rowcount")
    );
    assert!(!patch.body().contains("Record <&> 49"));
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_DATA_TABLE_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "data-table-stream-root".into(),
            data_id: "data-table-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(
            path,
            format!(
                "<!doctype html><html><head>{}</head><body>{}{}{}</body></html>",
                bootstrap.head_script().unwrap(),
                shell.to_html(&bootstrap).unwrap(),
                patch.to_html(&bootstrap).unwrap(),
                end.to_html(&bootstrap).unwrap()
            ),
        )
        .unwrap();
    }
}
