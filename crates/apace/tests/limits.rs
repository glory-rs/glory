#![cfg(not(target_arch = "wasm32"))]

use glory_apace::Renderer;
use glory_html::Part;
use glory_render::style::{Length, Limit, Style};
use glory_render::*;
use glory_ui::{App, Error as UiError, view};
use std::cell::RefCell;
use std::rc::Rc;

fn bounds(app: &App, node: NodeId) -> Rect {
    let result = Rc::new(RefCell::new(None));
    let reply = result.clone();
    app.request(Query::Bounds(node), move |value| {
        *reply.borrow_mut() = Some(value.unwrap())
    })
    .unwrap();
    app.pump().unwrap();
    let Some(QueryValue::Bounds(rect)) = result.borrow_mut().take() else {
        panic!("bounds")
    };
    rect
}

#[test]
fn explicit_unbounded_pixel_and_percent_limits_override_stylesheets_without_remounting() {
    let mut output = None;
    let app = App::new(|runtime| {
        let renderer = Renderer::new(runtime);
        output = Some(renderer.clone());
        renderer
    });
    let renderer = output.unwrap();
    renderer.set_css(".size{width:200px;height:100px;max-width:80px;max-height:40px}");
    let preferred = || {
        Style::new()
            .width(Length::Px(200.0))
            .height(Length::Px(100.0))
    };
    app.mount(
        view()
            .style(
                Style::new()
                    .width(Length::Px(400.0))
                    .height(Length::Px(300.0)),
            )
            .fill(
                view()
                    .class("size")
                    .style(preferred().max_width(Limit::None).max_height(Limit::None)),
            ),
    )
    .unwrap();
    let root = app.tree().roots()[0];
    let node = app.tree().node(root).unwrap().children[0];
    let physical = renderer.engine_id(node, Part::Main).unwrap();
    for (style, width, height) in [
        (
            preferred().max_width(Limit::None).max_height(Limit::None),
            200.0,
            100.0,
        ),
        (
            preferred()
                .max_width(Limit::Px(60.0))
                .max_height(Limit::Px(30.0)),
            60.0,
            30.0,
        ),
        (
            preferred()
                .max_width(Limit::Percent(25.0))
                .max_height(Limit::Percent(20.0)),
            100.0,
            60.0,
        ),
        (
            preferred()
                .min_width(Length::Px(140.0))
                .min_height(Length::Px(70.0))
                .max_width(Limit::Px(60.0))
                .max_height(Limit::Px(30.0)),
            140.0,
            70.0,
        ),
        (preferred(), 80.0, 40.0),
    ] {
        app.scope()
            .patch(node, Patch::Style(Change::Set(Box::new(style))))
            .unwrap();
        let rect = bounds(&app, node);
        assert!(
            (rect.width - width).abs() < 0.03 && (rect.height - height).abs() < 0.03,
            "{rect:?} expected {width}x{height}"
        );
        assert_eq!(renderer.engine_id(node, Part::Main).unwrap(), physical);
    }
    app.scope()
        .patch(node, Patch::Style(Change::Clear))
        .unwrap();
    let rect = bounds(&app, node);
    assert!((rect.width - 80.0).abs() < 0.03 && (rect.height - 40.0).abs() < 0.03);
    for limit in [
        Limit::Px(-1.0),
        Limit::Px(f64::NAN),
        Limit::Percent(f64::INFINITY),
    ] {
        let before = (app.stats(), app.tree().stats(), renderer.stats());
        assert!(matches!(
            app.scope().patch(
                node,
                Patch::Style(Change::Set(Box::new(preferred().max_width(limit))))
            ),
            Err(UiError::Render(Error::InvalidProps(_)))
        ));
        assert_eq!((app.stats(), app.tree().stats(), renderer.stats()), before);
        assert_eq!(renderer.engine_id(node, Part::Main).unwrap(), physical);
    }
    app.scope()
        .patch(
            node,
            Patch::Style(Change::Set(Box::new(
                preferred().max_width(Limit::None).max_height(Limit::None),
            ))),
        )
        .unwrap();
    let rect = bounds(&app, node);
    assert!((rect.width - 200.0).abs() < 0.03 && (rect.height - 100.0).abs() < 0.03);
}
