#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_render::{
    Action, Query, QueryValue,
    style::{Edges, Length, Overflow, Style},
};
use glory_ui::{App, view};
use std::{cell::RefCell, rc::Rc};

#[test]
fn metrics_read_real_client_and_overflow_extents_and_report_actual_scroll_assignment() {
    let app = App::new(Renderer::new);
    let reference = app.scope().cage(None);
    app.mount(
        view()
            .node_ref(reference)
            .style(
                Style::new()
                    .width(Length::Px(200.0))
                    .height(Length::Px(120.0))
                    .border_width(Edges::all(2.0))
                    .overflow_x(Overflow::Auto)
                    .overflow_y(Overflow::Auto),
            )
            .fill(
                view().style(
                    Style::new()
                        .width(Length::Px(600.0))
                        .height(Length::Px(800.0)),
                ),
            ),
    )
    .unwrap();
    let id = reference.get().unwrap();
    let get = || {
        let output = Rc::new(RefCell::new(None));
        let saved = output.clone();
        app.request(Query::ScrollMetrics(id), move |result| {
            *saved.borrow_mut() = Some(result.unwrap())
        })
        .unwrap();
        app.pump().unwrap();
        let QueryValue::ScrollMetrics(metrics) = output.borrow_mut().take().unwrap() else {
            panic!("metrics response")
        };
        metrics
    };
    let initial = get();
    initial.validate().unwrap();
    assert!(
        initial.viewport_width > 180.0 && initial.viewport_width <= 200.0,
        "{initial:?}"
    );
    assert!(
        initial.viewport_height > 100.0 && initial.viewport_height <= 120.0,
        "{initial:?}"
    );
    assert!(
        initial.content_width >= 600.0 && initial.content_height >= 800.0,
        "{initial:?}"
    );
    app.scope()
        .action(Action::ScrollTo {
            node: id,
            x: 50.0,
            y: 250.0,
        })
        .unwrap();
    app.flush().unwrap();
    let scrolled = get();
    assert_eq!((scrolled.x, scrolled.y), (50.0, 250.0));
    assert_eq!(
        (scrolled.content_width, scrolled.content_height),
        (initial.content_width, initial.content_height)
    );
}
