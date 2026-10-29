#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_render::style::{
    AspectRatio, Display, GridPlacement, GridTrack, GridTracks, Length, Limit, Style,
};
use glory_render::{NodeId, Query, QueryValue, Rect};
use glory_ui::{App, view};
use std::{cell::RefCell, rc::Rc};
fn app() -> (App, Renderer) {
    let mut result = None;
    let app = App::new(|runtime| {
        let renderer = Renderer::new(runtime);
        renderer.set_viewport(640, 480, 1.0).unwrap();
        result = Some(renderer.clone());
        renderer
    });
    (app, result.unwrap())
}
fn bounds(app: &App, id: NodeId) -> Rect {
    let value = Rc::new(RefCell::new(None));
    let output = value.clone();
    app.request(Query::Bounds(id), move |value| {
        *output.borrow_mut() = Some(value.unwrap())
    })
    .unwrap();
    app.pump().unwrap();
    let QueryValue::Bounds(rect) = value.borrow_mut().take().unwrap() else {
        panic!("bounds")
    };
    rect
}
fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.1,
        "expected {expected}, got {actual}"
    );
}
fn cell() -> impl glory_ui::Widget {
    view().style(
        Style::new()
            .height(Length::Px(20.0))
            .min_width(Length::Px(0.0)),
    )
}
#[test]
fn native_grid_resolves_px_percentage_and_fractional_tracks_into_real_boxes() {
    let (app, _) = app();
    app.mount(
        view()
            .style(
                Style::new()
                    .display(Display::Grid)
                    .width(Length::Px(400.0))
                    .grid_columns(GridTracks::Tracks(vec![
                        GridTrack::Px(80.0),
                        GridTrack::Percent(25.0),
                        GridTrack::Fr(1.0),
                        GridTrack::Fr(2.0),
                    ])),
            )
            .fill((cell(), cell(), cell(), cell())),
    )
    .unwrap();
    let parent = app.tree().roots()[0];
    let children = app.tree().node(parent).unwrap().children.clone();
    for (id, x, width) in [
        (children[0], 0.0, 80.0),
        (children[1], 80.0, 100.0),
        (children[2], 180.0, 220.0 / 3.0),
        (children[3], 180.0 + 220.0 / 3.0, 440.0 / 3.0),
    ] {
        let rect = bounds(&app, id);
        near(rect.x, x);
        near(rect.width, width);
        near(rect.height, 20.0);
    }
}
#[test]
fn native_auto_track_uses_content_before_fractional_remainder() {
    let (app, _) = app();
    app.mount(
        view()
            .style(
                Style::new()
                    .display(Display::Grid)
                    .width(Length::Px(200.0))
                    .grid_columns(GridTracks::Tracks(vec![
                        GridTrack::Auto,
                        GridTrack::Fr(1.0),
                    ])),
            )
            .fill((
                view().style(
                    Style::new()
                        .width(Length::Px(40.0))
                        .height(Length::Px(20.0)),
                ),
                cell(),
            )),
    )
    .unwrap();
    let root = app.tree().roots()[0];
    let children = app.tree().node(root).unwrap().children.clone();
    near(bounds(&app, children[0]).width, 40.0);
    near(bounds(&app, children[1]).x, 40.0);
    near(bounds(&app, children[1]).width, 160.0);
}
#[test]
fn native_grid_at_and_auto_span_include_gaps_and_row_positions() {
    let (app, _) = app();
    app.mount(
        view()
            .style(
                Style::new()
                    .display(Display::Grid)
                    .width(Length::Px(200.0))
                    .gap(10.0)
                    .grid_columns(GridTracks::Tracks(vec![GridTrack::Px(60.0); 3]))
                    .grid_rows(GridTracks::Tracks(vec![GridTrack::Px(20.0); 2])),
            )
            .fill((
                view().style(
                    Style::new()
                        .grid_column(GridPlacement::Span(2))
                        .height(Length::Px(20.0)),
                ),
                view().style(
                    Style::new()
                        .grid_column(GridPlacement::At { start: 2, span: 2 })
                        .grid_row(GridPlacement::At { start: 2, span: 1 })
                        .height(Length::Px(20.0)),
                ),
            )),
    )
    .unwrap();
    let root = app.tree().roots()[0];
    let children = app.tree().node(root).unwrap().children.clone();
    let a = bounds(&app, children[0]);
    let b = bounds(&app, children[1]);
    near(a.x, 0.0);
    near(a.width, 130.0);
    near(a.y, 0.0);
    near(b.x, 70.0);
    near(b.width, 130.0);
    near(b.y, 30.0);
}
#[test]
fn grid_track_and_placement_resets_relayout_without_replacing_engine_nodes() {
    let (app, renderer) = app();
    let style = app.scope().cage(
        Style::new()
            .display(Display::Grid)
            .width(Length::Px(200.0))
            .grid_columns(GridTracks::Tracks(vec![GridTrack::Px(100.0); 2])),
    );
    app.mount(view().style(style).fill((cell(), cell())))
        .unwrap();
    let root = app.tree().roots()[0];
    let children = app.tree().node(root).unwrap().children.clone();
    let physical = renderer
        .engine_id(children[1], glory_html::Part::Main)
        .unwrap();
    near(bounds(&app, children[1]).x, 100.0);
    style.revise(|style| style.grid_columns = Some(GridTracks::None));
    app.flush().unwrap();
    near(bounds(&app, children[1]).x, 0.0);
    near(bounds(&app, children[1]).y, 20.0);
    assert_eq!(
        renderer
            .engine_id(children[1], glory_html::Part::Main)
            .unwrap(),
        physical
    );
    style
        .revise(|style| style.grid_columns = Some(GridTracks::Tracks(vec![GridTrack::Fr(1.0); 2])));
    app.flush().unwrap();
    near(bounds(&app, children[1]).x, 100.0);
    assert_eq!(app.tree().node(root).unwrap().children, children);
}
#[test]
fn native_ratio_changes_geometry_and_respects_definite_axes_and_min_over_max() {
    let (app, renderer) = app();
    let style = app.scope().cage(
        Style::new()
            .width(Length::Px(200.0))
            .aspect_ratio(AspectRatio::Ratio(2.0)),
    );
    app.mount(view().style(style)).unwrap();
    let root = app.tree().roots()[0];
    let physical = renderer.engine_id(root, glory_html::Part::Main).unwrap();
    near(bounds(&app, root).height, 100.0);
    style.revise(|style| style.aspect_ratio = Some(AspectRatio::Ratio(1.0)));
    app.flush().unwrap();
    near(bounds(&app, root).height, 200.0);
    style.revise(|style| {
        *style = Style::new()
            .width(Length::Px(100.0))
            .height(Length::Px(30.0))
            .aspect_ratio(AspectRatio::Ratio(2.0))
    });
    app.flush().unwrap();
    near(bounds(&app, root).height, 30.0);
    style.revise(|style| {
        *style = Style::new()
            .width(Length::Px(100.0))
            .aspect_ratio(AspectRatio::Ratio(2.0))
            .min_height(Length::Px(80.0))
            .max_height(Limit::Px(40.0))
    });
    app.flush().unwrap();
    near(bounds(&app, root).height, 80.0);
    style.revise(|style| {
        *style = Style::new()
            .width(Length::Px(100.0))
            .aspect_ratio(AspectRatio::Auto)
    });
    app.flush().unwrap();
    near(bounds(&app, root).height, 0.0);
    assert_eq!(
        renderer.engine_id(root, glory_html::Part::Main).unwrap(),
        physical
    );
}
