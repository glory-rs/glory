#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_render::style::{Display, Length, Style};
use glory_render::{Query, QueryValue};
use glory_ui::{App, view};
use std::{cell::RefCell, rc::Rc};
fn bounds(app: &App, id: glory_render::NodeId) -> glory_render::Rect {
    let result = Rc::new(RefCell::new(None));
    let reply = result.clone();
    app.request(Query::Bounds(id), move |value| {
        *reply.borrow_mut() = Some(value.unwrap())
    })
    .unwrap();
    app.pump().unwrap();
    let QueryValue::Bounds(rect) = result.borrow_mut().take().unwrap() else {
        panic!("bounds")
    };
    rect
}
#[test]
fn native_inline_flex_shrinks_to_content_and_block_flex_reset_preserves_identity() {
    let app = App::new(|runtime| {
        let host = Renderer::new(runtime);
        host.set_viewport(300, 100, 1.0).unwrap();
        host
    });
    let style = app.scope().cage(Style::new().display(Display::InlineFlex));
    app.mount(
        view().style(Style::new().width(Length::Px(240.0))).fill(
            view().style(style).fill(
                view().style(
                    Style::new()
                        .width(Length::Px(40.0))
                        .height(Length::Px(20.0)),
                ),
            ),
        ),
    )
    .unwrap();
    let parent = app.tree().roots()[0];
    let child = app.tree().node(parent).unwrap().children[0];
    let rect = bounds(&app, child);
    assert!(
        (rect.width - 40.0).abs() < 0.1 && (rect.height - 20.0).abs() < 0.1,
        "{rect:?}"
    );
    style.revise(|style| style.display = Some(Display::Flex));
    app.flush().unwrap();
    let full = bounds(&app, child);
    assert!((full.width - 240.0).abs() < 0.1, "{full:?}");
    assert_eq!(app.tree().node(parent).unwrap().children[0], child);
    style.revise(|style| style.display = Some(Display::InlineFlex));
    app.flush().unwrap();
    assert!((bounds(&app, child).width - 40.0).abs() < 0.1);
}
