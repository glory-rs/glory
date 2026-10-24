use super::*;
use glory_render::{
    GraphicPoint, GraphicProps, GraphicStroke, GraphicTransition, PathCommand, StrokeCap,
};
#[test]
fn compatible_path_transition_config_survives_content_patch_and_classic_stream_end() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let ready = resources
                .bind(
                    scope.owner(),
                    "path-transition",
                    "v1",
                    || 1u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            glory_ui::graphic()
                .class("stream-path-transition")
                .drawing(ready.map(|state| {
                    let done = matches!(state, ResourceState::Ready { .. });
                    let y = if done { 20. } else { 4. };
                    GraphicProps {
                        commands: vec![
                            PathCommand::Move(GraphicPoint::new(2., y)),
                            PathCommand::Line(GraphicPoint::new(22., y)),
                        ],
                        stroke: Some(GraphicStroke {
                            color: glory_render::style::Color(0, 128, 0, 255),
                            width: 2.,
                            cap: StrokeCap::Round,
                        }),
                        transition: Some(GraphicTransition {
                            duration_millis: if done { 300 } else { 200 },
                        }),
                        ..Default::default()
                    }
                }))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("data-gly-path-transition"));
    assert!(shell.body().contains("transition-duration:200ms;"));
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    assert!(patch.body().contains("M2 20L22 20"));
    assert!(patch.body().contains("transition-duration:300ms;"));
    assert!(!patch.body().contains("replace"));
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_GRAPHIC_TRANSITION_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "graphic-transition-stream-root".into(),
            data_id: "graphic-transition-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\"><style>{}</style>{}</head><body>{}{}{}</body></html>",glory_html::BASE_CSS,bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
