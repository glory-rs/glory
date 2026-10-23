#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::style::{Align, Display, FlexDirection, FlexWrap, Length, Style};
use glory_render::{Query, QueryValue};
use glory_ui::{App, view};
use std::{cell::RefCell, rc::Rc};
fn bounds(app: &App, node: glory_render::NodeId) -> glory_render::Rect {
    let result = Rc::new(RefCell::new(None));
    let reply = result.clone();
    app.request(Query::Bounds(node), move |value| {
        *reply.borrow_mut() = Some(value.unwrap())
    })
    .unwrap();
    app.pump().unwrap();
    let QueryValue::Bounds(value) = result.borrow_mut().take().unwrap() else {
        panic!("bounds")
    };
    value
}
#[test]
fn native_wrap_moves_only_the_overflowing_child_to_a_new_line_and_reset_recovers() {
    let app = App::new(|runtime| {
        let host = Renderer::new(runtime);
        host.set_viewport(200, 100, 1.0).unwrap();
        host
    });
    let style = app.scope().cage(Style {
        align: Some(Align::Start),
        ..Style::new()
            .display(Display::Flex)
            .flex_direction(FlexDirection::Row)
            .flex_wrap(FlexWrap::Wrap)
            .width(Length::Px(60.0))
    });
    let child = || {
        view().style(Style {
            shrink: Some(0.0),
            ..Style::new()
                .width(Length::Px(40.0))
                .height(Length::Px(20.0))
        })
    };
    app.mount(view().style(style).fill((child(), child())))
        .unwrap();
    let root = app.tree().roots()[0];
    let children = app.tree().node(root).unwrap().children.clone();
    let first = bounds(&app, children[0]);
    let wrapped = bounds(&app, children[1]);
    assert!((wrapped.x - first.x).abs() < 0.1);
    assert!((wrapped.y - first.y - 20.0).abs() < 0.1);
    style.revise(|value| value.flex_wrap = Some(FlexWrap::NoWrap));
    app.flush().unwrap();
    let unwrapped = bounds(&app, children[1]);
    assert!((unwrapped.y - first.y).abs() < 0.1);
    assert!((unwrapped.x - first.x - 40.0).abs() < 0.1);
    assert_eq!(app.tree().node(root).unwrap().children, children);
}
