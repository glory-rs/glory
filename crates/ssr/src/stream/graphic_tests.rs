use super::*;
use glory_render::{GraphicPoint, GraphicProps, GraphicStroke, PathCommand, StrokeCap};
#[test]
fn finite_graphic_construction_and_field_patch_survive_stream_end() {
    let gate = Gate::default();
    let loader = gate.clone();
    let mut stream = stream_with(
        Resources::default(),
        Config::default(),
        move |scope, resources| {
            let ready = resources
                .bind(
                    scope.owner(),
                    "graphic-drawing",
                    "v1",
                    || 1u64,
                    move |_| loader.waiting(),
                )
                .unwrap();
            let drawing = ready.map(|state| GraphicProps {
                width: 24.0,
                height: 24.0,
                commands: vec![
                    PathCommand::Move(GraphicPoint::new(2.0, 2.0)),
                    PathCommand::Line(GraphicPoint::new(
                        if matches!(state, ResourceState::Ready { .. }) {
                            22.0
                        } else {
                            12.0
                        },
                        22.0,
                    )),
                ],
                stroke: Some(GraphicStroke {
                    color: glory_render::style::Color(255, 0, 0, 255),
                    width: 2.0,
                    cap: StrokeCap::Round,
                }),
                alternative: Some("Progress illustration".into()),
                ..Default::default()
            });
            let visible = ready.map(|state| matches!(state, ResourceState::Ready { .. }));
            view().fill((
                glory_ui::graphic()
                    .class("stream-current-graphic")
                    .drawing(drawing),
                when(visible, || {
                    glory_ui::graphic()
                        .class("stream-created-graphic")
                        .drawing(GraphicProps {
                            commands: vec![
                                PathCommand::Move(GraphicPoint::new(0.0, 0.0)),
                                PathCommand::Line(GraphicPoint::new(24.0, 0.0)),
                                PathCommand::Line(GraphicPoint::new(12.0, 24.0)),
                                PathCommand::Close,
                            ],
                            fill: Some(glory_render::style::Color(0, 128, 0, 255)),
                            alternative: Some("Created graphic".into()),
                            ..Default::default()
                        })
                }),
            ))
        },
    )
    .unwrap();
    let shell = frame(&mut stream);
    assert!(shell.body().contains("<svg"));
    assert!(shell.body().contains("<path"));
    assert!(shell.body().contains("M2 2L12 22"));
    assert!(shell.body().contains("role=\"img\""));
    gate.resolve(Ok("ready".into()));
    let patch = frame(&mut stream);
    assert_eq!(patch.kind(), FrameKind::Patch);
    assert!(patch.body().contains("\"name\":\"d\""));
    assert!(patch.body().contains("M2 2L22 22"));
    assert!(patch.body().contains("\"tag\":\"svg\""));
    assert!(patch.body().contains("\"tag\":\"path\""));
    assert!(!patch.body().contains("replace"));
    let end = frame(&mut stream);
    assert_eq!(end.kind(), FrameKind::End);
    if let Some(path) = std::env::var_os("GLORY_GRAPHICS_STREAM_FIXTURE") {
        let bootstrap = Bootstrap {
            root_id: "graphics-stream-root".into(),
            data_id: "graphics-stream-data".into(),
            script_url: "glory-stream.js".into(),
            nonce: None,
        };
        std::fs::write(path,format!("<!doctype html><html><head><link rel=\"icon\" href=\"data:,\">{}</head><body>{}{}{}</body></html>",bootstrap.head_script().unwrap(),shell.to_html(&bootstrap).unwrap(),patch.to_html(&bootstrap).unwrap(),end.to_html(&bootstrap).unwrap())).unwrap();
    }
}
