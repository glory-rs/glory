use super::*;
use glory_render::{
    AnchorAlign, AnchorPlacement, AnchorSide, FocusMode, Layer, LayerPlacement, Role, SemanticId,
    Semantics,
};

#[test]
fn finite_anchor_policy_fields_survive_classic_patch_and_versioned_start_end() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let resource = resources
                .bind(
                    scope.owner(),
                    "anchor",
                    "v1",
                    || 1_u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            let ready = resource.map(|state| matches!(state, ResourceState::Ready { .. }));
            view().fill((
                glory_ui::button()
                    .label("Stream anchor")
                    .semantics(Some(Box::new(Semantics {
                        id: Some(SemanticId::new("stream-anchor").unwrap()),
                        ..Default::default()
                    }))),
                view()
                    .role(Role::Dialog)
                    .focus(FocusMode::Programmatic)
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(SemanticId::new("stream-anchor-panel").unwrap()),
                            layer: Some(Layer {
                                modal: false,
                                placement: LayerPlacement::Anchor(if ready.get() {
                                    AnchorPlacement {
                                        side: AnchorSide::Top,
                                        align: AnchorAlign::End,
                                        gap: 17.,
                                        viewport_padding: 23.,
                                        flip: false,
                                        shift: false,
                                    }
                                } else {
                                    AnchorPlacement::default()
                                }),
                                anchor: Some(SemanticId::new("stream-anchor").unwrap()),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }))
                    }))
                    .fill(text().content("Finite placement")),
                view()
                    .role(Role::Dialog)
                    .focus(FocusMode::Programmatic)
                    .name("Stream viewport window")
                    .semantics(scope.bond(move || {
                        Some(Box::new(Semantics {
                            id: Some(SemanticId::new("stream-viewport-window").unwrap()),
                            layer: Some(Layer {
                                modal: false,
                                placement: LayerPlacement::Viewport(if ready.get() {
                                    glory_render::ViewportPlacement {
                                        x: 175.5,
                                        y: -23.25,
                                        viewport_padding: 43.,
                                        constrain: false,
                                    }
                                } else {
                                    glory_render::ViewportPlacement::default()
                                }),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }))
                    })),
            ))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("data-gly-anchor-side=\"bottom\""));
    assert!(shell.body().contains("data-gly-viewport-x=\"48\""));
    assert!(poll(&mut stream).is_pending());
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    for field in [
        "data-gly-anchor-side",
        "data-gly-anchor-align",
        "data-gly-anchor-gap",
        "data-gly-anchor-padding",
        "data-gly-anchor-flip",
        "data-gly-anchor-shift",
        "data-gly-viewport-x",
        "data-gly-viewport-y",
        "data-gly-viewport-padding",
        "data-gly-viewport-constrain",
    ] {
        assert!(patch.body().contains(field), "{field}");
    }
    let end = frame(&mut stream);
    let bootstrap = Bootstrap {
        root_id: "anchor-stream-root".into(),
        data_id: "anchor-stream-data".into(),
        script_url: "glory-stream.js".into(),
        nonce: None,
    };
    let shell_html = shell.to_html(&bootstrap).unwrap();
    assert!(shell_html.contains(&format!("\"html_version\":{}", glory_html::FORMAT_VERSION)));
    assert!(
        end.body()
            .contains(&format!("\"html_version\":{}", glory_html::FORMAT_VERSION))
    );
    if let Some(path) = std::env::var_os("GLORY_ANCHOR_STREAM_FIXTURE") {
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\">{}</head><body>{}{}{}</body></html>",bootstrap.head_script().unwrap(),shell_html,patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
