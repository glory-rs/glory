use super::*;
use glory_render::{FocusMode, Layer, LayerPlacement, Role, SemanticId, Semantics, SheetConfig};
#[test]
fn bounded_sheet_config_and_revision_survive_stream_patch_and_end_without_input_driver() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let ready = resources
                .bind(
                    scope.owner(),
                    "sheet",
                    "v1",
                    || 1u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            view()
                .class("stream-sheet")
                .role(Role::Dialog)
                .focus(FocusMode::Programmatic)
                .name("Streaming sheet")
                .semantics(ready.map(|state| {
                    let done = matches!(state, ResourceState::Ready { .. });
                    Some(Box::new(Semantics {
                        id: Some(SemanticId::new("stream-sheet").unwrap()),
                        layer: Some(Layer {
                            placement: LayerPlacement::Bottom,
                            sheet: Some(SheetConfig {
                                fractions: vec![0.25, 0.5, 1.0],
                                current: if done { 2 } else { 0 },
                                max_extent: 640.0,
                                settle_millis: 180,
                                dismissible: true,
                                revision: u64::from(done),
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .fill((
                    view()
                        .class("stream-sheet-grip")
                        .semantics(Some(Box::new(Semantics {
                            sheet_grip: true,
                            ..Default::default()
                        })))
                        .style(
                            glory_render::style::Style::new()
                                .height(glory_render::style::Length::Px(24.0)),
                        ),
                    glory_ui::button().label("Stream action"),
                ))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("data-gly-sheet=\"0.25,0.5,1\""));
    assert!(shell.body().contains("data-gly-sheet-revision=\"0\""));
    assert!(shell.body().contains("data-gly-sheet-max=\"640\""));
    assert!(shell.body().contains("data-gly-sheet-settle=\"180\""));
    assert!(shell.body().contains("data-gly-sheet-dismissible=\"true\""));
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    assert!(patch.body().contains("data-gly-sheet-current"));
    assert!(patch.body().contains("data-gly-sheet-revision"));
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_SHEET_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "sheet-stream-root".into(),
            data_id: "sheet-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\"><style>{}</style>{}</head><body>{}{}{}</body></html>",glory_html::BASE_CSS,bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
