#![cfg(all(not(target_arch = "wasm32"), feature = "raster"))]
use glory_blitz::{Raster, Renderer};
use glory_render::style::{Color, Display, Edges, Length, Overflow, Style};
use glory_render::{GraphicPoint as Point, GraphicProps, PathCommand as Path, Props};
use glory_ui::{App, graphic, view};

fn square(color: Color) -> GraphicProps {
    GraphicProps {
        width: 24.0,
        height: 24.0,
        commands: vec![
            Path::Move(Point::new(0.0, 0.0)),
            Path::Line(Point::new(24.0, 0.0)),
            Path::Line(Point::new(24.0, 24.0)),
            Path::Line(Point::new(0.0, 24.0)),
            Path::Close,
        ],
        fill: Some(color),
        ..Default::default()
    }
}
fn pixel(raster: &Raster, x: usize, y: usize) -> &[u8] {
    let width = raster.frame().unwrap().width as usize;
    &raster.pixels()[(y * width + x) * 4..(y * width + x) * 4 + 4]
}
#[test]
fn actual_custom_widget_paths_obey_native_ancestor_clip_opacity_and_lifecycle() {
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::new(runtime);
        host.set_viewport(100, 100, 1.0).unwrap();
        renderer = Some(host.clone());
        host
    });
    let renderer = renderer.unwrap();
    let drawing = app.scope().cage(square(Color(255, 0, 0, 255)));
    app.mount(
        view()
            .style(
                Style::new()
                    .width(Length::Px(40.0))
                    .height(Length::Px(40.0))
                    .overflow_x(Overflow::Hidden)
                    .overflow_y(Overflow::Hidden)
                    .opacity(0.5),
            )
            .fill(
                graphic().drawing(drawing).style(
                    Style::new()
                        .display(Display::Block)
                        .width(Length::Px(80.0))
                        .height(Length::Px(80.0)),
                ),
            ),
    )
    .unwrap();
    let root = app.tree().roots()[0];
    let id = app.tree().node(root).unwrap().children[0];
    let engine = renderer.engine_id(id, glory_html::Part::Main).unwrap();
    let mut raster = Raster::new();
    raster.draw(&renderer).unwrap();
    let red = pixel(&raster, 20, 20);
    // Raster exposes premultiplied RGBA8: half-opacity red has R == alpha.
    assert_eq!(&red[1..3], &[0, 0]);
    assert!((127..=128).contains(&red[3]), "{red:?}");
    assert_eq!(red[0], red[3]);
    assert_eq!(pixel(&raster, 50, 20), [0, 0, 0, 0]);
    drawing.revise(|value| *value = square(Color(0, 0, 255, 255)));
    app.flush().unwrap();
    raster.draw(&renderer).unwrap();
    let blue = pixel(&raster, 20, 20);
    assert_eq!(&blue[..2], &[0, 0]);
    assert_eq!(blue[2], blue[3]);
    assert!((127..=128).contains(&blue[3]));
    assert_eq!(
        renderer.engine_id(id, glory_html::Part::Main).unwrap(),
        engine
    );
    renderer.set_viewport(200, 200, 2.0).unwrap();
    raster.draw(&renderer).unwrap();
    let blue = pixel(&raster, 60, 60);
    assert_eq!(&blue[..2], &[0, 0]);
    assert_eq!(blue[2], blue[3]);
    assert!((127..=128).contains(&blue[3]));
    assert_eq!(pixel(&raster, 90, 40), [0, 0, 0, 0]);
    app.clear().unwrap();
    raster.draw(&renderer).unwrap();
    assert_eq!(pixel(&raster, 20, 20), [0, 0, 0, 0]);
    assert_eq!(renderer.stats().nodes, 0);
}
#[test]
fn stroke_caps_and_cubic_segments_have_real_scaled_native_pixels() {
    use glory_render::{GraphicStroke, StrokeCap};
    for cap in [StrokeCap::Butt, StrokeCap::Round, StrokeCap::Square] {
        let mut renderer = None;
        let app = App::new(|runtime| {
            let host = Renderer::new(runtime);
            host.set_viewport(100, 100, 1.0).unwrap();
            renderer = Some(host.clone());
            host
        });
        let renderer = renderer.unwrap();
        let drawing = GraphicProps {
            width: 100.0,
            height: 100.0,
            commands: vec![
                Path::Move(Point::new(20.0, 50.0)),
                Path::Cubic {
                    first: Point::new(40.0, 50.0),
                    second: Point::new(60.0, 50.0),
                    end: Point::new(80.0, 50.0),
                },
            ],
            stroke: Some(GraphicStroke {
                color: Color(0, 255, 0, 255),
                width: 20.0,
                cap,
            }),
            ..Default::default()
        };
        app.mount(
            graphic().drawing(drawing).style(
                Style::new()
                    .width(Length::Px(100.0))
                    .height(Length::Px(100.0)),
            ),
        )
        .unwrap();
        let mut raster = Raster::new();
        raster.draw(&renderer).unwrap();
        assert_eq!(pixel(&raster, 50, 50), [0, 255, 0, 255]);
        if cap == StrokeCap::Butt {
            assert_eq!(pixel(&raster, 12, 50), [0, 0, 0, 0]);
        } else {
            assert_eq!(pixel(&raster, 12, 50), [0, 255, 0, 255]);
        }
        assert_eq!(pixel(&raster, 7, 50), [0, 0, 0, 0]);
        renderer.set_viewport(200, 200, 2.0).unwrap();
        raster.draw(&renderer).unwrap();
        assert_eq!(pixel(&raster, 100, 100), [0, 255, 0, 255]);
        assert_eq!(pixel(&raster, 100, 75), [0, 0, 0, 0]);
        if cap == StrokeCap::Butt {
            assert_eq!(pixel(&raster, 24, 100), [0, 0, 0, 0]);
        } else {
            assert_eq!(pixel(&raster, 24, 100), [0, 255, 0, 255]);
        }
    }
}
#[test]
fn unsupported_graphic_box_declarations_reject_before_ui_allocation() {
    for style in [
        Style::new().padding(Edges::all(1.0)),
        Style::new().border_width(Edges::all(1.0)),
        Style::new().object_fit(glory_render::style::ObjectFit::Fill),
    ] {
        let app = App::new(Renderer::new);
        assert!(
            app.mount(
                graphic()
                    .drawing(square(Color(255, 0, 0, 255)))
                    .style(style)
            )
            .is_err()
        );
        assert!(app.tree().roots().is_empty());
    }
    // Declared zero edges agree with the native content/border box contract.
    let app = App::new(Renderer::new);
    app.mount(
        graphic().drawing(square(Color(255, 0, 0, 255))).style(
            Style::new()
                .padding(Edges::all(0.0))
                .border_width(Edges::all(0.0)),
        ),
    )
    .unwrap();
    assert!(matches!(
        app.tree()
            .node(app.tree().roots()[0])
            .unwrap()
            .descriptor
            .view()
            .props,
        Props::Graphic(_)
    ));
}
