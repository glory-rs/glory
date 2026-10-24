use super::*;
use glory_render::{Layer, Role, Semantics};
#[test]
fn busy_inert_content_and_retained_layer_survive_a_stream_patch_and_end() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let ready = resources
                .bind(
                    scope.owner(),
                    "inert",
                    "v1",
                    || 1u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            view()
                .class("stream-loading-region")
                .name("Streaming region")
                .semantics(ready.map(|state| {
                    Some(Box::new(Semantics {
                        busy: Some(matches!(state, ResourceState::Ready { .. })),
                        ..Default::default()
                    }))
                }))
                .fill(
                    view()
                        .class("stream-inert-content")
                        .semantics(ready.map(|state| {
                            Some(Box::new(Semantics {
                                inert: matches!(state, ResourceState::Ready { .. }),
                                ..Default::default()
                            }))
                        }))
                        .fill((
                            glory_ui::button().label("Retained action"),
                            view()
                                .class("stream-inert-layer")
                                .role(Role::Dialog)
                                .focus(glory_render::FocusMode::Programmatic)
                                .name("Retained dialog")
                                .semantics(Some(Box::new(Semantics {
                                    id: Some(
                                        glory_render::SemanticId::new("stream-inert-dialog")
                                            .unwrap(),
                                    ),
                                    layer: Some(Layer::default()),
                                    ..Default::default()
                                })))
                                .fill(glory_ui::button().label("Layer action")),
                        )),
                )
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("aria-busy=\"false\""));
    assert!(!shell.body().contains("data-gly-inert"));
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    assert!(
        patch.body().contains("aria-busy")
            && patch.body().contains("data-gly-inert")
            && patch.body().contains("\"name\":\"inert\"")
    );
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_INERT_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "inert-stream-root".into(),
            data_id: "inert-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\"><style>{}</style>{}</head><body><button id=\"outside-stream\">Outside</button>{}{}{}</body></html>",glory_html::BASE_CSS,bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
