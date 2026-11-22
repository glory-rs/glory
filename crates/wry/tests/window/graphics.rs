use glory_render::{Event, EventKind, GraphicAnimation, GraphicPoint, GraphicProps, PathCommand};
use glory_ui::{graphic, group, text, view};
use glory_wry::shell::WindowApp;

pub(super) fn mount(app: &WindowApp) -> glory_ui::Result {
    let observation = app.app().scope().cage(String::new());
    app.mount(
        group().fill((
            view()
                .class("plot-viewport")
                .style(
                    glory_render::style::Style::new()
                        .width(glory_render::style::Length::Px(120.))
                        .height(glory_render::style::Length::Px(48.)),
                )
                .on(EventKind::PlotPointer, move |event| {
                    if let Event::PlotPointer { pointer, .. } = event {
                        observation.revise(|text| {
                            *text = format!("{:?}:{:.1}:{:.1}", pointer.phase, pointer.x, pointer.y)
                        });
                    }
                })
                .fill(graphic().drawing(GraphicProps {
                    commands: vec![
                        PathCommand::Move(GraphicPoint::new(2., 2.)),
                        PathCommand::Line(GraphicPoint::new(18., 12.)),
                        PathCommand::Line(GraphicPoint::new(2., 22.)),
                        PathCommand::Close,
                    ],
                    fill: Some(glory_render::style::Color(30, 100, 200, 255)),
                    animation: Some(GraphicAnimation::Rotate {
                        period_millis: 1000,
                    }),
                    ..GraphicProps::default()
                })),
            text().class("plot-observation").content(observation),
        )),
    )
}
