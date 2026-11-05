#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_render::{
    DocumentPart, NodeId, Query, QueryValue, Rect,
    style::{CaptionSide, Length, Style},
};
use glory_ui::{App, text, view};
use std::{cell::RefCell, rc::Rc};
fn bounds(app: &App, node: NodeId) -> Rect {
    let result = Rc::new(RefCell::new(None));
    let saved = result.clone();
    app.request(Query::Bounds(node), move |reply| {
        *saved.borrow_mut() = Some(reply.unwrap())
    })
    .unwrap();
    app.pump().unwrap();
    let QueryValue::Bounds(rect) = result.borrow_mut().take().unwrap() else {
        panic!("bounds")
    };
    rect
}
#[test]
fn caption_side_changes_actual_table_formatting_and_preserves_caption_identity() {
    let mut host = None;
    let app = App::new(|runtime| {
        let renderer = Renderer::new(runtime);
        renderer.set_viewport(300, 200, 1.).unwrap();
        host = Some(renderer.clone());
        renderer
    });
    let host = host.unwrap();
    let style = app.scope().cage(
        Style::new()
            .width(Length::Px(240.))
            .caption_side(CaptionSide::Bottom),
    );
    let caption = app.scope().cage(None);
    let body = app.scope().cage(None);
    app.mount(
        view()
            .document_part(DocumentPart::Table)
            .style(style)
            .fill((
                view()
                    .document_part(DocumentPart::TableBody)
                    .node_ref(body)
                    .fill(
                        view().document_part(DocumentPart::TableRow).fill(
                            view()
                                .document_part(DocumentPart::TableCell)
                                .style(Style::new().height(Length::Px(40.)))
                                .fill(text().content("Data")),
                        ),
                    ),
                view()
                    .document_part(DocumentPart::TableCaption)
                    .node_ref(caption)
                    .fill(text().content("Table caption")),
            )),
    )
    .unwrap();
    app.settle(12).unwrap();
    let caption = caption.get().unwrap();
    let body = body.get().unwrap();
    let physical = host.engine_id(caption, glory_html::Part::Main).unwrap();
    let lower = bounds(&app, caption);
    let rows = bounds(&app, body);
    assert!(lower.y >= rows.y + rows.height - 0.5, "{lower:?} {rows:?}");
    style.revise(|s| s.caption_side = Some(CaptionSide::Top));
    app.settle(12).unwrap();
    let upper = bounds(&app, caption);
    let rows = bounds(&app, body);
    assert!(upper.y + upper.height <= rows.y + 0.5, "{upper:?} {rows:?}");
    assert_eq!(
        host.engine_id(caption, glory_html::Part::Main).unwrap(),
        physical
    );
    style.revise(|s| s.caption_side = None);
    app.settle(12).unwrap();
    let reset = bounds(&app, caption);
    assert!((reset.y - upper.y).abs() < 0.5);
    app.clear().unwrap();
    assert_eq!(app.stats().nodes, 0);
}
#[cfg(feature = "raster")]
#[test]
fn dashed_and_dotted_borders_paint_real_gaps_on_the_same_physical_node() {
    use glory_apace::Raster;
    use glory_render::style::{BorderStyle, Color, Edges};
    let mut host = None;
    let app = App::new(|runtime| {
        let renderer = Renderer::new(runtime);
        renderer.set_viewport(140, 30, 1.).unwrap();
        host = Some(renderer.clone());
        renderer
    });
    let host = host.unwrap();
    let style = app.scope().cage(Style {
        background: Some(Color(255, 255, 255, 255)),
        border_color: Some(Color(255, 0, 0, 255)),
        ..Style::new()
            .width(Length::Px(120.))
            .height(Length::Px(20.))
            .border_width(Edges {
                top: 4.,
                right: 0.,
                bottom: 0.,
                left: 0.,
            })
            .border_style(BorderStyle::Solid)
    });
    let root = app.scope().cage(None);
    app.mount(view().style(style).node_ref(root)).unwrap();
    app.settle(12).unwrap();
    let root = root.get().unwrap();
    let physical = host.engine_id(root, glory_html::Part::Main).unwrap();
    for variant in [BorderStyle::Solid, BorderStyle::Dashed, BorderStyle::Dotted] {
        style.revise(|s| s.border_style = Some(variant));
        app.settle(12).unwrap();
        let mut raster = Raster::new();
        raster.draw(&host).unwrap();
        let width = raster.frame().unwrap().width as usize;
        let pixels = raster.pixels();
        let red = (0..120)
            .filter(|x| {
                let p = &pixels[(2 * width + x) * 4..][..4];
                p[0] > 180 && p[1] < 80 && p[2] < 80
            })
            .count();
        if variant == BorderStyle::Solid {
            assert!(red >= 119, "{red}");
        } else {
            assert!(red > 8 && red < 112, "{variant:?}: {red}");
        }
        assert_eq!(
            host.engine_id(root, glory_html::Part::Main).unwrap(),
            physical
        );
    }
}
