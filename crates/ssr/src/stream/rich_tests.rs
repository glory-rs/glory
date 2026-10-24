use super::*;
use glory_render::{FocusMode, RichBlock, RichBlockKind, RichDocument, RichMarks, RichRun};
fn document(ready: bool) -> std::rc::Rc<RichDocument> {
    std::rc::Rc::new(
        RichDocument::new(vec![RichBlock {
            kind: if ready {
                RichBlockKind::Heading(2)
            } else {
                RichBlockKind::Paragraph
            },
            runs: vec![RichRun {
                text: if ready {
                    "Stream 😀 <literal>"
                } else {
                    "Initial document"
                }
                .into(),
                marks: if ready {
                    RichMarks {
                        bold: true,
                        italic: true,
                        code: true,
                        link: Some("https://example.test/?x=1&y=2".into()),
                    }
                } else {
                    RichMarks::default()
                },
            }],
        }])
        .unwrap(),
    )
}
#[test]
fn rich_shell_document_patch_and_end_share_the_finite_shape_and_format() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let ready = resources
                .bind(
                    scope.owner(),
                    "rich",
                    "v1",
                    || 1u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            let ready = ready.map(|state| matches!(state, ResourceState::Ready { .. }));
            view().fill((
                glory_ui::rich_editor()
                    .class("stream-rich")
                    .name("Stream document")
                    .focus(FocusMode::TabStop)
                    .document(scope.bond(move || document(ready.get()))),
                when(ready, || {
                    glory_ui::rich_editor()
                        .class("stream-created-rich")
                        .name("Created streamed document")
                        .focus(FocusMode::TabStop)
                        .document(document(true))
                }),
            ))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("<p>Initial document</p>"));
    assert!(
        shell.body().contains("contenteditable=\"true\"")
            && shell.body().contains("data-gly-kind=\"rich-editor\""),
        "{}",
        shell.body()
    );
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    for value in [
        "richDocument",
        "Stream 😀 <literal>",
        "https://example.test/?x=1&y=2",
        "\"kind\":2",
        "\"bold\":true",
        "\"italic\":true",
        "\"code\":true",
        "\"op\":\"create\"",
        "stream-created-rich",
    ] {
        assert!(patch.body().contains(value), "{value}: {}", patch.body());
    }
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    assert!(
        end.body()
            .contains(&format!("\"html_version\":{}", glory_html::FORMAT_VERSION))
    );
    if let Some(path) = std::env::var_os("GLORY_RICH_EDITOR_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "rich-stream-root".into(),
            data_id: "rich-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><meta charset=\"utf-8\"><link rel=\"icon\" href=\"data:,\"><style>{}</style>{}</head><body>{}{}{}</body></html>",glory_html::BASE_CSS,bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
