use super::*;
use glory_render::{
    GraphicAnimation, GraphicPoint, GraphicProps, GraphicStroke, PathCommand, StrokeCap,
};
#[test]
fn finite_rotation_survives_stream_content_attribute_patch_and_end() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let ready = resources
                .bind(
                    scope.owner(),
                    "motion",
                    "v1",
                    || 1u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            glory_ui::graphic()
                .class("stream-spinner")
                .drawing(ready.map(|state| {
                    GraphicProps {
                        commands: vec![
                            PathCommand::Move(GraphicPoint::new(12., 12.)),
                            PathCommand::Line(GraphicPoint::new(22., 12.)),
                        ],
                        stroke: Some(GraphicStroke {
                            color: glory_render::style::Color(0, 128, 0, 255),
                            width: 3.,
                            cap: StrokeCap::Round,
                        }),
                        animation: matches!(state, ResourceState::Ready { .. })
                            .then_some(GraphicAnimation::Rotate { period_millis: 500 }),
                        ..Default::default()
                    }
                }))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(!shell.body().contains("data-gly-rotate"));
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    assert!(
        patch.body().contains("data-gly-rotate")
            && patch.body().contains("animation-duration:500ms;")
            && patch.body().contains("\"part\":\"content\"")
    );
    assert!(!patch.body().contains("replace"));
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_SPINNER_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "spinner-stream-root".into(),
            data_id: "spinner-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\"><style>{}</style>{}</head><body>{}{}{}</body></html>",glory_html::BASE_CSS,bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
