use super::*;
use glory_render::{Layer, LayerFocus, LayerPlacement, Role, SemanticId, Semantics};

#[test]
fn tooltip_stream_preserves_description_relation_and_passive_layer_through_end() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let resource = resources
                .bind(
                    scope.owner(),
                    "hint",
                    "v1",
                    || 1_u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            let ready = resource.map(|state| matches!(state, ResourceState::Ready { .. }));
            let tip = SemanticId::new("stream-tip").unwrap();
            let target = SemanticId::new("stream-tip-target").unwrap();
            let anchor = target.clone();
            view().fill((
                glory_ui::button()
                    .label("Stream help")
                    .semantics(Some(Box::new(Semantics {
                        id: Some(target),
                        described_by: Some(tip.clone()),
                        ..Default::default()
                    }))),
                view()
                    .role(Role::Tooltip)
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(tip.clone()),
                            hidden: !ready.get(),
                            layer: Some(Layer {
                                modal: false,
                                focus: LayerFocus::Preserve,
                                placement: LayerPlacement::Anchor(
                                    glory_render::AnchorPlacement::new(
                                        glory_render::AnchorSide::Bottom,
                                        glory_render::AnchorAlign::Start,
                                    ),
                                ),
                                anchor: Some(anchor.clone()),
                                initial_focus: None,
                                return_focus: None,
                                ..Default::default()
                            }),
                            ..Default::default()
                        }))
                    }))
                    .fill(text().content(scope.bond(move || {
                        if ready.get() {
                            "Loaded tooltip description".to_owned()
                        } else {
                            "Waiting description".to_owned()
                        }
                    }))),
            ))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("role=\"tooltip\""));
    assert!(shell.body().contains("aria-describedby=\"stream-tip\""));
    assert!(
        !shell.body().contains("aria-haspopup")
            && !shell.body().contains("aria-expanded")
            && !shell.body().contains("aria-modal")
    );
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    assert!(patch.body().contains("Loaded tooltip description") && patch.body().contains("hidden"));
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_HINTS_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "hints-stream-root".into(),
            data_id: "hints-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\">{}</head><body>{}{}{}</body></html>",
            bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
