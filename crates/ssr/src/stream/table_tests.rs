use super::*;
use glory_render::{DocumentPart, SemanticId, Semantics, TableMetadata};

fn cell(part: DocumentPart, column: u16, name: &'static str) -> impl Widget {
    view()
        .semantics(Some(Box::new(Semantics {
            id: Some(SemanticId::new(name).unwrap()),
            document_part: Some(part),
            table: Some(TableMetadata::Cell {
                row: u32::from(part == DocumentPart::TableCell),
                column,
                row_span: 1,
                column_span: 1,
                headers: if part == DocumentPart::TableCell {
                    vec![SemanticId::new("stream-heading").unwrap()]
                } else {
                    vec![]
                },
            }),
            ..Default::default()
        })))
        .fill(text().content(name))
}
fn table() -> impl Widget {
    view().document_part(DocumentPart::Table).fill((
        view()
            .document_part(DocumentPart::TableCaption)
            .fill(text().content("Stream <&> caption")),
        view().document_part(DocumentPart::TableHead).fill(
            view().document_part(DocumentPart::TableRow).fill(cell(
                DocumentPart::ColumnHeader,
                0,
                "stream-heading",
            )),
        ),
        view().document_part(DocumentPart::TableBody).fill(
            view().document_part(DocumentPart::TableRow).fill(cell(
                DocumentPart::TableCell,
                0,
                "stream-value",
            )),
        ),
        view().document_part(DocumentPart::TableFoot).fill(
            view().document_part(DocumentPart::TableRow).fill(cell(
                DocumentPart::RowHeader,
                0,
                "stream-total",
            )),
        ),
    ))
}
#[test]
fn table_intrinsics_header_associations_and_caption_survive_classic_patch_and_end() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let resource = resources
                .bind(
                    scope.owner(),
                    "table",
                    "v1",
                    || 1_u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            when(
                resource.map(|state| matches!(state, ResourceState::Ready { .. })),
                table,
            )
            .otherwise(|| text().content("pending table"))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("pending table"));
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    for tag in [
        "table", "caption", "thead", "tbody", "tfoot", "tr", "th", "td",
    ] {
        assert!(
            patch.body().contains(&format!("\"tag\":\"{tag}\"")),
            "missing {tag}"
        );
    }
    assert!(patch.body().contains("headers"));
    assert!(patch.body().contains("stream-heading"));
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_TABLE_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "table-stream-root".into(),
            data_id: "table-stream-data".into(),
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
