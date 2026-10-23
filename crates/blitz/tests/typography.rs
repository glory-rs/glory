#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_html::Part;
use glory_render::style::{BorderStyle, Color, Edges, Family, Length, Style, WhiteSpace};
use glory_render::{NodeId, Query, QueryValue, Rect, TextKind};
use glory_ui::{App, input, list, text, view};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn fixture() -> (App, Renderer) {
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::new(runtime);
        renderer = Some(host.clone());
        host
    });
    (app, renderer.unwrap())
}
fn settle(app: &App) {
    for _ in 0..8 {
        app.pump().unwrap();
    }
}
fn bounds(app: &App, id: NodeId) -> Rect {
    let out = Rc::new(RefCell::new(None));
    let copy = out.clone();
    app.request(Query::Bounds(id), move |r| *copy.borrow_mut() = Some(r))
        .unwrap();
    app.pump().unwrap();
    let QueryValue::Bounds(rect) = out.borrow_mut().take().unwrap().unwrap() else {
        panic!("bounds")
    };
    rect
}
fn type_style(mode: WhiteSpace) -> Style {
    Style {
        font_size: Some(16.0),
        line_height: Some(20.0),
        ..Style::new().width(Length::Px(160.0))
    }
    .font_family([
        Family::Named("Glory deliberately missing face".into()),
        Family::Mono,
    ])
    .font_weight(450.5)
    .white_space(mode)
    .border_width(Edges::all(2.0))
    .border_style(BorderStyle::Solid)
    .border_color(Color(1, 2, 3, 255))
    .radius(8.0)
    .opacity(0.5)
}
const WORDS: &str = "alpha beta gamma delta epsilon zeta eta theta";

#[test]
fn typed_whitespace_reflows_inherited_text_without_replacing_control_parts() {
    let (app, renderer) = fixture();
    let style = app.scope().cage(type_style(WhiteSpace::NoWrap));
    let content = app.scope().cage(WORDS.to_string());
    app.mount(view().style(style).fill((
        text().kind(TextKind::Paragraph).content(content),
        input().value("draft🙂"),
    )))
    .unwrap();
    let root = app.tree().roots()[0];
    let ids = app.tree().node(root).unwrap().children.clone();
    let original = renderer.engine_id(ids[1], Part::Main).unwrap();
    let first = bounds(&app, ids[0]);
    assert!((first.height - 20.0).abs() < 0.03, "{first:?}");
    for mode in [
        WhiteSpace::Normal,
        WhiteSpace::Pre,
        WhiteSpace::PreWrap,
        WhiteSpace::NoWrap,
    ] {
        style.revise(|s| s.white_space = Some(mode));
        app.flush().unwrap();
        let rect = bounds(&app, ids[0]);
        if matches!(mode, WhiteSpace::Normal | WhiteSpace::PreWrap) {
            assert!(rect.height >= 60.0, "{mode:?}: {rect:?}");
        } else {
            assert!((rect.height - 20.0).abs() < 0.03, "{mode:?}: {rect:?}");
        }
        assert_eq!(renderer.engine_id(ids[1], Part::Main).unwrap(), original);
    }
    assert!(
        (bounds(&app, ids[0]).x - bounds(&app, root).x - 2.0).abs() < 0.03,
        "border affects content geometry"
    );
    content.revise(|s| *s = "alpha\nbeta".into());
    for mode in [
        WhiteSpace::Pre,
        WhiteSpace::Normal,
        WhiteSpace::PreWrap,
        WhiteSpace::NoWrap,
    ] {
        style.revise(|s| s.white_space = Some(mode));
        app.flush().unwrap();
        let expected = if matches!(mode, WhiteSpace::Pre | WhiteSpace::PreWrap) {
            40.0
        } else {
            20.0
        };
        assert!(
            (bounds(&app, ids[0]).height - expected).abs() < 0.03,
            "{mode:?}: newlines must follow the whitespace contract"
        );
    }
    app.clear().unwrap();
    assert_eq!(renderer.stats().nodes, 0);
}

fn rows(app: &App) -> Vec<(usize, NodeId)> {
    let tree = app.tree();
    let root = tree.roots()[0];
    let surface = tree.node(root).unwrap().children[0];
    tree.node(surface)
        .unwrap()
        .children
        .iter()
        .filter_map(|id| {
            tree.node(*id)
                .unwrap()
                .descriptor
                .item
                .map(|item| (item.index, *id))
        })
        .collect()
}

#[cfg(feature = "css")]
#[test]
fn parsed_typography_and_decoration_sheets_reflow_the_live_native_document() {
    use glory_render::style::Sheet;
    let (app, renderer) = fixture();
    app.mount(
        view()
            .class("type")
            .fill(text().kind(TextKind::Paragraph).content(WORDS)),
    )
    .unwrap();
    let root = app.tree().roots()[0];
    let paragraph = app.tree().node(root).unwrap().children[0];
    let engine = renderer.engine_id(paragraph, Part::Main).unwrap();
    for wrap in [false, true, false] {
        let sheet=Sheet::parse(&format!(".type{{width:160px;font-family:'Glory deliberately missing face',monospace;font-weight:450.5;font-size:16px;line-height:20px;white-space:{};border-width:2px;border-style:solid;border-color:#123;border-radius:8px;opacity:.5}}",if wrap {"normal"} else {"nowrap"})).unwrap();
        renderer.set_sheet(&sheet).unwrap();
        let rect = bounds(&app, paragraph);
        assert!(
            if wrap {
                rect.height >= 60.0
            } else {
                (rect.height - 20.0).abs() < 0.03
            },
            "{rect:?}"
        );
        assert_eq!(renderer.engine_id(paragraph, Part::Main).unwrap(), engine);
    }
    app.clear().unwrap();
}

#[test]
fn typography_growth_above_a_hundred_million_row_anchor_stays_bounded() {
    let (app, renderer) = fixture();
    let baseline = renderer.stats().engine_nodes;
    let position = app.scope().cage(20_000_007.0);
    let changed = app.scope().cage(None::<usize>);
    let reads = Rc::new(Cell::new(0));
    let copy = reads.clone();
    let stats = Rc::new(RefCell::new(None));
    let latest = stats.clone();
    app.mount(
        list(
            100_000_000_usize,
            move |i| {
                copy.set(copy.get() + 1);
                i
            },
            move |_, key| {
                view()
                    .style(
                        changed
                            .map(move |changed| {
                                let mut s = type_style(if *changed == Some(key) {
                                    WhiteSpace::PreWrap
                                } else {
                                    WhiteSpace::NoWrap
                                });
                                // Keep the initial estimate exact; decorations are tested separately.
                                s.border_width = Some(Edges::all(0.0));
                                s
                            })
                            .with_partial_eq(),
                    )
                    .fill(text().kind(TextKind::Paragraph).content(WORDS))
            },
        )
        .height(180.0)
        .measure(20.0)
        .measure_limit(64)
        .overscan(2)
        .row_limit(32)
        .position(position)
        .inspect(move |s| *latest.borrow_mut() = Some(s)),
    )
    .unwrap();
    settle(&app);
    let root = app.tree().roots()[0];
    let outer = bounds(&app, root);
    let live = rows(&app);
    let (key, anchor) = live
        .iter()
        .copied()
        .find(|(_, id)| {
            let r = bounds(&app, *id);
            r.y <= outer.y + 0.03 && r.y + r.height > outer.y + 0.03
        })
        .unwrap();
    assert_eq!(key, 1_000_000);
    let before = bounds(&app, anchor);
    let previous = live.iter().find(|(i, _)| *i == key - 1).unwrap().1;
    let old = bounds(&app, previous).height;
    let logical = stats.borrow().as_ref().unwrap().logical;
    changed.revise(|v| *v = Some(key - 1));
    app.flush().unwrap();
    settle(&app);
    let grown = bounds(&app, previous).height;
    assert!(grown >= 60.0 && old == 20.0, "{old}->{grown}");
    assert_eq!(
        rows(&app).iter().find(|(i, _)| *i == key).unwrap().1,
        anchor
    );
    assert!((bounds(&app, anchor).y - before.y).abs() < 0.03);
    assert!((stats.borrow().as_ref().unwrap().logical - logical - (grown - old)).abs() < 0.03);
    for i in 0..25 {
        position.revise(|v| {
            *v = if i % 2 == 0 {
                1_900_000_007.0
            } else {
                i as f64 * 7919.0 * 200.0
            }
        });
        app.flush().unwrap();
        settle(&app);
        let stats = stats.borrow();
        let stats = stats.as_ref().unwrap();
        assert!(
            stats.rows <= 16 && stats.measured <= 64 && stats.height_capacity < 4096,
            "{stats:?}"
        );
        assert!(renderer.stats().engine_nodes < 200 && app.runtime_stats().node_slots < 256);
    }
    assert!(reads.get() < 4000, "{} key reads", reads.get());
    eprintln!(
        "typography anchor: key={key}, height={old}->{grown}, key_reads={}, final={:?}",
        reads.get(),
        stats.borrow()
    );
    app.clear().unwrap();
    settle(&app);
    assert_eq!(renderer.stats().engine_nodes, baseline);
    assert_eq!(app.runtime_stats().tasks, 0);
}
