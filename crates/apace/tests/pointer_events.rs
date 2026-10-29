#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_render::{
    Query,
    style::{Length, PointerEvents, Position, Style},
};
use glory_ui::{App, view};
#[test]
fn real_native_hit_testing_passes_through_none_and_resets_to_auto_without_replacing_nodes() {
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::new(runtime);
        host.set_viewport(100, 100, 1.).unwrap();
        renderer = Some(host.clone());
        host
    });
    let renderer = renderer.unwrap();
    let mask = app.scope().cage(
        Style::new()
            .position(Position::Absolute)
            .top(Length::Px(0.))
            .left(Length::Px(0.))
            .width(Length::Px(40.))
            .height(Length::Px(40.))
            .pointer_events(PointerEvents::Auto),
    );
    app.mount(
        view()
            .style(
                Style::new()
                    .position(Position::Relative)
                    .width(Length::Px(40.))
                    .height(Length::Px(40.)),
            )
            .fill((
                view().style(Style::new().width(Length::Px(40.)).height(Length::Px(40.))),
                view().style(mask),
            )),
    )
    .unwrap();
    let root = app.tree().roots()[0];
    let base = app.tree().node(root).unwrap().children[0];
    let cover = app.tree().node(root).unwrap().children[1];
    let base_physical = renderer.engine_id(base, glory_html::Part::Main).unwrap();
    let cover_physical = renderer.engine_id(cover, glory_html::Part::Main).unwrap();
    for value in [
        Some(PointerEvents::Auto),
        Some(PointerEvents::None),
        Some(PointerEvents::Auto),
        None,
    ] {
        mask.revise(|style| style.pointer_events = value);
        app.request(Query::Bounds(root), |result| {
            result.unwrap();
        })
        .unwrap();
        app.pump().unwrap();
        let hit = renderer.inspect(|doc| doc.hit(10., 10.).unwrap().node_id);
        assert_eq!(
            hit,
            if value == Some(PointerEvents::None) {
                base_physical
            } else {
                cover_physical
            }
        );
        assert_eq!(
            renderer.engine_id(cover, glory_html::Part::Main).unwrap(),
            cover_physical
        );
    }
    app.clear().unwrap();
    assert_eq!(renderer.stats().nodes, 0);
}
