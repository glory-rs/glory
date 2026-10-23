use super::*;
use glory_render::{DocumentPart, TextKind};

fn document() -> impl Widget {
    view().class("stream-document").fill((
        text().kind(TextKind::Code).content("code <&>"),
        text().kind(TextKind::Kbd).content("Control"),
        text().kind(TextKind::Mark).content("marked"),
        view()
            .document_part(DocumentPart::Blockquote)
            .fill(text().content("quote")),
        view().document_part(DocumentPart::OrderedList).fill(
            view()
                .document_part(DocumentPart::ListItem)
                .fill(text().content("ordered")),
        ),
        view().document_part(DocumentPart::UnorderedList).fill(
            view()
                .document_part(DocumentPart::ListItem)
                .fill(text().content("unordered")),
        ),
        view().document_part(DocumentPart::DescriptionList).fill((
            view()
                .document_part(DocumentPart::Term)
                .fill(text().content("term")),
            view()
                .document_part(DocumentPart::Definition)
                .fill(text().content("definition")),
        )),
    ))
}
#[test]
fn document_tags_survive_stream_patch_construction_and_end_handoff() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let resource = resources
                .bind(
                    scope.owner(),
                    "document",
                    "v1",
                    || 1_u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            when(
                resource.map(|state| matches!(state, ResourceState::Ready { .. })),
                document,
            )
            .otherwise(|| text().content("pending document"))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("pending document"));
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    for tag in [
        "code",
        "kbd",
        "mark",
        "blockquote",
        "ol",
        "ul",
        "li",
        "dl",
        "dt",
        "dd",
    ] {
        assert!(
            patch.body().contains(&format!("\"tag\":\"{tag}\"")),
            "missing {tag}"
        );
    }
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_DOCUMENT_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "document-stream-root".into(),
            data_id: "document-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        let body = format!(
            "<!doctype html><html><head>{}</head><body>{}{}{}</body></html>",
            bootstrap.head_script().unwrap(),
            shell.to_html(&bootstrap).unwrap(),
            patch.to_html(&bootstrap).unwrap(),
            end.to_html(&bootstrap).unwrap()
        );
        std::fs::write(path, body).unwrap();
    }
}
