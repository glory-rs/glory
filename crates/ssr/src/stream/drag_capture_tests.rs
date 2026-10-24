use super::*;
use glory_render::{DragCapture, FocusMode, Semantics};

#[test]
fn press_capture_policy_survives_semantic_patch_and_classic_end() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let ready = resources
                .bind(
                    scope.owner(),
                    "drag-policy",
                    "v1",
                    || 1u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            view()
                .class("stream-drag-plane")
                .focus(FocusMode::TabStop)
                .semantics(ready.map(|state| {
                    Some(Box::new(Semantics {
                        drag_capture: Some(if matches!(state, ResourceState::Ready { .. }) {
                            DragCapture::PressFocus
                        } else {
                            DragCapture::Press
                        }),
                        ..Default::default()
                    }))
                }))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("data-gly-drag-capture=\"press\""));
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    assert!(patch.body().contains("data-gly-drag-capture") && patch.body().contains("press-focus"));
    assert!(!patch.body().contains("replace"));
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    assert!(
        end.body()
            .contains(&format!("\"html_version\":{}", glory_html::FORMAT_VERSION))
    );
    if let Some(path) = std::env::var_os("GLORY_DRAG_CAPTURE_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "drag-stream-root".into(),
            data_id: "drag-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path, format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\"><style>{}</style>{}</head><body>{}{}{}</body></html>",glory_html::BASE_CSS,bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
