#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_html::Part;
use glory_render::{
    InputAssist, InputAutofill, InputMode, NodeId, Purpose, Query, QueryValue, Rect, Semantics,
    style::{Edges, Family, Length, Style},
};
use glory_ui::{App, input};
use std::{cell::RefCell, rc::Rc};
fn bounds(app: &App, id: NodeId) -> Rect {
    let reply = Rc::new(RefCell::new(None));
    let out = reply.clone();
    app.request(Query::Bounds(id), move |v| {
        *out.borrow_mut() = Some(v.unwrap())
    })
    .unwrap();
    app.pump().unwrap();
    let QueryValue::Bounds(rect) = reply.borrow_mut().take().unwrap() else {
        panic!("bounds")
    };
    rect
}

#[test]
fn typed_ch_dimensions_minimums_and_signed_insets_follow_the_same_live_font_metrics() {
    use glory_render::style::Position;
    use glory_ui::view;
    let mut output = None;
    let app = App::new(|runtime| {
        let renderer = Renderer::new(runtime);
        output = Some(renderer.clone());
        renderer
    });
    let host = output.unwrap();
    host.set_viewport(700, 700, 1.).unwrap();
    let root = app.scope().cage(None);
    let zero = app.scope().cage(None);
    let start = app.scope().cage(None);
    let end = app.scope().cage(None);
    let minimum = app.scope().cage(None);
    app.mount(
        view()
            .style(
                Style {
                    font_size: Some(20.),
                    ..Style::new()
                }
                .font_family([Family::Mono])
                .position(Position::Relative)
                .width(Length::Px(400.))
                .height(Length::Px(400.)),
            )
            .node_ref(root)
            .fill((
                view()
                    .style(Style::new().width(Length::Ch(1.)).height(Length::Px(1.)))
                    .node_ref(zero),
                view()
                    .style(
                        Style::new()
                            .position(Position::Absolute)
                            .left(Length::Ch(-1.))
                            .top(Length::Ch(2.))
                            .width(Length::Ch(2.))
                            .height(Length::Ch(3.)),
                    )
                    .node_ref(start),
                view()
                    .style(
                        Style::new()
                            .position(Position::Absolute)
                            .right(Length::Ch(1.))
                            .bottom(Length::Ch(-2.))
                            .width(Length::Ch(2.))
                            .height(Length::Ch(3.)),
                    )
                    .node_ref(end),
                view()
                    .style(
                        Style::new()
                            .width(Length::Px(0.))
                            .height(Length::Px(0.))
                            .min_width(Length::Ch(2.))
                            .min_height(Length::Ch(3.)),
                    )
                    .node_ref(minimum),
            )),
    )
    .unwrap();
    let root = bounds(&app, root.get().unwrap());
    let unit = bounds(&app, zero.get().unwrap()).width;
    assert!(unit > 10.);
    let a = bounds(&app, start.get().unwrap());
    let b = bounds(&app, end.get().unwrap());
    let min = bounds(&app, minimum.get().unwrap());
    for (actual, expected) in [
        (a.x, root.x - unit),
        (a.y, root.y + unit * 2.),
        (a.width, unit * 2.),
        (a.height, unit * 3.),
        (b.x, root.x + 400. - unit * 3.),
        (b.y, root.y + 400. - unit),
        (b.width, unit * 2.),
        (b.height, unit * 3.),
        (min.width, unit * 2.),
        (min.height, unit * 3.),
    ] {
        assert!(
            (actual - expected).abs() < 0.2,
            "typed ch declaration must reach actual layout"
        );
    }
    app.clear().unwrap();
    assert_eq!(app.stats().queries, 0);
}
#[test]
fn typed_ch_resolves_real_zero_and_password_projection_advances_across_fonts_without_author_css() {
    assert_eq!(glory_apace::STYLE_PROFILE.version, 11);
    for font in [14., 24., 36.] {
        for masked in [false, true] {
            let mut output = None;
            let app = App::new(|runtime| {
                let host = Renderer::new(runtime);
                output = Some(host.clone());
                host
            });
            let host = output.unwrap();
            host.set_viewport(600, 150, 1.).unwrap();
            let reference = app.scope().cage(None);
            let style = app.scope().cage(
                Style {
                    font_size: Some(font),
                    ..Style::new()
                }
                .width(Length::Ch(6.))
                .height(Length::Px(64.))
                .font_family([Family::Mono])
                .padding(Edges::all(0.))
                .border_width(Edges::all(0.)),
            );
            app.mount(
                input()
                    .value("000000")
                    .purpose(if masked {
                        Purpose::Password
                    } else {
                        Purpose::Text
                    })
                    .style(style)
                    .node_ref(reference)
                    .semantics(Some(Box::new(Semantics {
                        input_assist: Some(InputAssist {
                            autofill: Some(InputAutofill::OneTimeCode),
                            mode: Some(InputMode::Numeric),
                        }),
                        ..Default::default()
                    }))),
            )
            .unwrap();
            app.settle(12).unwrap();
            let id = reference.get().unwrap();
            let physical = host.engine_id(id, Part::Main).unwrap();
            let rect = bounds(&app, id);
            let advance = host.inspect(|dom| {
                let editor = dom
                    .get_node(physical)
                    .unwrap()
                    .element_data()
                    .unwrap()
                    .text_input_data()
                    .unwrap();
                assert_eq!(editor.is_password(), masked);
                editor.display_editor().try_layout().unwrap().width()
            });
            assert!(
                (rect.width - f64::from(advance)).abs() < 0.2,
                "font geometry and display projection must agree"
            );
            style.revise(|s| s.width = Some(Length::Ch(3.)));
            app.flush().unwrap();
            let half = bounds(&app, id);
            assert!((half.width * 2. - rect.width).abs() < 0.1);
            assert_eq!(host.engine_id(id, Part::Main).unwrap(), physical);
            app.clear().unwrap();
            assert_eq!(app.stats().queries, 0);
        }
    }
}
