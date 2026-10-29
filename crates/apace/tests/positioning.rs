#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_render::{
    NodeId, Query, QueryValue, Rect,
    style::{Length, Position, Style},
};
use glory_ui::{App, view};
use std::{cell::RefCell, rc::Rc};
fn bounds(app: &App, node: NodeId) -> Rect {
    let output = Rc::new(RefCell::new(None));
    let saved = output.clone();
    app.request(Query::Bounds(node), move |value| {
        *saved.borrow_mut() = Some(value.unwrap())
    })
    .unwrap();
    app.pump().unwrap();
    let QueryValue::Bounds(rect) = output.borrow_mut().take().unwrap() else {
        panic!("bounds")
    };
    rect
}
fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.5, "{actual} != {expected}");
}
#[test]
fn fixed_percent_signed_pixel_and_auto_insets_reach_actual_native_layout_without_replacing_identity()
 {
    let renderer = Rc::new(RefCell::new(None));
    let saved = renderer.clone();
    let app = App::new(move |runtime| {
        let output = Renderer::new(runtime);
        *saved.borrow_mut() = Some(output.clone());
        output
    });
    let renderer = renderer.borrow().as_ref().unwrap().clone();
    renderer.set_viewport(800, 600, 1.0).unwrap();
    let base = Style::new()
        .position(Position::Fixed)
        .width(Length::Px(100.0))
        .height(Length::Px(40.0));
    let source = app.scope().cage(
        base.clone()
            .right(Length::Percent(10.0))
            .bottom(Length::Percent(20.0)),
    );
    let reference = app.scope().cage(None);
    app.mount(view().style(source).node_ref(reference)).unwrap();
    let id = reference.get().unwrap();
    let identity = renderer.engine_id(id, glory_html::Part::Main).unwrap();
    let percent = bounds(&app, id);
    close(percent.x, 620.0);
    close(percent.y, 440.0);
    source.revise(|value| {
        *value = base
            .clone()
            .right(Length::Px(-5.0))
            .bottom(Length::Px(-10.0))
    });
    app.flush().unwrap();
    let signed = bounds(&app, id);
    close(signed.x, 705.0);
    close(signed.y, 570.0);
    source.revise(|value| {
        *value = base
            .clone()
            .right(Length::Auto)
            .bottom(Length::Auto)
            .left(Length::Px(12.0))
            .top(Length::Px(13.0))
    });
    app.flush().unwrap();
    let reset = bounds(&app, id);
    close(reset.x, 12.0);
    close(reset.y, 13.0);
    assert_eq!(
        renderer.engine_id(id, glory_html::Part::Main).unwrap(),
        identity
    );
}
