#![cfg(all(not(target_arch = "wasm32"), feature = "paint"))]

use glory_blitz::Renderer;
use glory_ui::{App, view};

fn fixture(css: &str, width: u32, height: u32) -> (App, Renderer) {
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::new(runtime);
        host.set_viewport(width, height, 1.0).unwrap();
        host.set_css(css);
        renderer = Some(host.clone());
        host
    });
    (app, renderer.unwrap())
}

#[test]
fn complete_frame_recording_resets_commands_and_reuses_unchanged_layout() {
    let (app, renderer) = fixture(".box{width:40px;height:20px;background:blue}", 80, 40);
    app.mount(view().class("box")).unwrap();
    let mut scene = anyrender::Scene::new();
    let info = renderer.paint(&mut scene);
    let count = scene.commands.len();
    assert!(count > 0);
    let layouts = renderer.stats().layouts;
    for _ in 0..20 {
        assert_eq!(renderer.paint(&mut scene), info);
        assert_eq!(scene.commands.len(), count);
    }
    assert_eq!(renderer.stats().layouts, layouts);
    renderer.set_viewport(0, 40, 1.0).unwrap();
    assert_eq!(renderer.paint(&mut scene).width, 0);
    assert!(scene.commands.is_empty());
}

#[cfg(feature = "raster")]
mod raster {
    use super::*;
    use glory_blitz::{Raster, RasterError};
    use glory_render::style::{Color, Length, Style};
    use glory_render::{Change, Patch};
    use glory_ui::{list, text};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    fn pixel(raster: &Raster, x: usize, y: usize) -> &[u8] {
        let width = raster.frame().unwrap().width as usize;
        &raster.pixels()[(y * width + x) * 4..(y * width + x) * 4 + 4]
    }

    #[test]
    fn actual_pixels_follow_text_and_style_updates_and_clear_without_ghosts() {
        let (app, renderer) = fixture(
            ".box{width:200px;height:120px;color:white;padding:8px}",
            200,
            120,
        );
        let style = app.scope().cage(Style {
            background: Some(Color(0, 0, 255, 255)),
            ..Style::new()
        });
        let label = app.scope().cage(String::from("Hello"));
        app.mount(view().class("box").style(style).fill(text().content(label)))
            .unwrap();
        let mut raster = Raster::new();
        assert!(raster.draw(&renderer).unwrap().rendered);
        assert_eq!(pixel(&raster, 150, 100), [0, 0, 255, 255]);
        assert!(
            raster
                .pixels()
                .as_chunks::<4>()
                .0
                .iter()
                .any(|p| p[0] > 0 && p[1] > 0 && p[2] == 255),
            "text produced actual light pixels"
        );
        let original = raster.pixels().to_vec();
        let pointer = raster.pixels().as_ptr();
        let layouts = renderer.stats().layouts;
        assert!(!raster.draw(&renderer).unwrap().rendered);
        assert_eq!(renderer.stats().layouts, layouts);
        assert_eq!(raster.pixels().as_ptr(), pointer);
        label.revise(|value| *value = "Changed".into());
        // Drawing sees committed renderer state, never implicitly runs the App.
        assert!(!raster.draw(&renderer).unwrap().rendered);
        app.flush().unwrap();
        assert!(raster.draw(&renderer).unwrap().rendered);
        assert_ne!(raster.pixels(), original);
        assert_eq!(raster.pixels().as_ptr(), pointer);
        style.revise(|value| value.background = Some(Color(255, 0, 0, 255)));
        app.flush().unwrap();
        assert!(raster.draw(&renderer).unwrap().rendered);
        assert_eq!(pixel(&raster, 150, 100), [255, 0, 0, 255]);
        app.clear().unwrap();
        assert!(raster.draw(&renderer).unwrap().rendered);
        assert!(
            raster.pixels().iter().all(|byte| *byte == 0),
            "old pixels were cleared"
        );
        raster.clear();
        assert_eq!(raster.buffer_capacity(), 0);
        assert!(raster.frame().is_none());
    }

    #[test]
    fn dpi_css_and_runtime_identity_are_part_of_the_pixel_cache_key() {
        let (app, renderer) = fixture(".box{width:40px;height:20px;background:blue}", 80, 40);
        app.mount(view().class("box")).unwrap();
        let root = app.tree().roots()[0];
        let mut raster = Raster::new();
        let first = raster.draw(&renderer).unwrap();
        assert_eq!(pixel(&raster, 60, 30), [0, 0, 0, 0]);
        app.scope()
            .patch(root, Patch::Classes(Change::Keep))
            .unwrap();
        app.flush().unwrap();
        let noop = raster.draw(&renderer).unwrap();
        assert!(!noop.rendered);
        assert!(noop.info.applied > first.info.applied);
        assert_eq!(noop.info.revision, first.info.revision);
        renderer.set_viewport(80, 40, 1.0).unwrap();
        renderer.set_css(".box{width:40px;height:20px;background:blue}");
        assert!(!raster.draw(&renderer).unwrap().rendered);
        renderer.set_viewport(80, 40, 2.0).unwrap();
        assert!(raster.draw(&renderer).unwrap().rendered);
        assert_eq!(pixel(&raster, 60, 30), [0, 0, 255, 255]);
        renderer.set_css(".box{width:40px;height:20px;background:lime}");
        // A read can resolve layout before the host paints. That must not
        // consume the independent visual invalidation or reuse blue pixels.
        app.request(glory_render::Query::Bounds(root), |value| {
            assert!(value.is_ok())
        })
        .unwrap();
        app.pump().unwrap();
        assert!(raster.draw(&renderer).unwrap().rendered);
        assert_eq!(pixel(&raster, 60, 30), [0, 255, 0, 255]);
        renderer.invalidate();
        assert!(raster.draw(&renderer).unwrap().rendered);
        let (other, host) = fixture(".box{width:40px;height:20px;background:red}", 80, 40);
        other.mount(view().class("box")).unwrap();
        assert_ne!(renderer.frame_info().runtime, host.frame_info().runtime);
        assert!(raster.draw(&host).unwrap().rendered);
        assert_eq!(pixel(&raster, 10, 10), [255, 0, 0, 255]);
    }

    #[test]
    fn translucent_backbuffer_pixels_are_premultiplied_rgba() {
        let (app, renderer) = fixture(
            ".box{width:20px;height:10px;background:rgba(255,0,0,.5)}",
            20,
            10,
        );
        app.mount(view().class("box")).unwrap();
        let mut raster = Raster::new();
        raster.draw(&renderer).unwrap();
        let p = pixel(&raster, 10, 5);
        assert_eq!(p[0], p[3]);
        assert_eq!(&p[1..3], &[0, 0]);
        assert!((127..=128).contains(&p[3]));
    }

    #[test]
    fn typed_border_radius_and_group_opacity_paint_composited_pixels() {
        use glory_render::style::{BorderStyle, Edges};
        let (app, renderer) = fixture("", 80, 80);
        let style = app.scope().cage(Style {
            background: Some(Color(255, 0, 0, 255)),
            ..Style::new()
                .width(Length::Px(60.0))
                .height(Length::Px(60.0))
                .border_width(Edges::all(4.0))
                .border_color(Color(0, 0, 255, 255))
                .border_style(BorderStyle::Solid)
                .radius(16.0)
                .opacity(0.5)
        });
        app.mount(
            view().style(style).fill(
                view().style(Style {
                    background: Some(Color(0, 255, 0, 255)),
                    ..Style::new()
                        .width(Length::Px(20.0))
                        .height(Length::Px(20.0))
                }),
            ),
        )
        .unwrap();
        let mut raster = Raster::new();
        raster.draw(&renderer).unwrap();
        assert_eq!(pixel(&raster, 0, 0), [0, 0, 0, 0], "rounded corner");
        for (x, y, channel) in [(30, 2, 2), (30, 30, 0), (14, 14, 1)] {
            let p = pixel(&raster, x, y);
            assert!((127..=128).contains(&p[3]), "{x},{y}: {p:?}");
            assert_eq!(
                p[channel], p[3],
                "group opacity covers border, background and child"
            );
            assert_eq!(
                p.iter().map(|v| u32::from(*v)).sum::<u32>(),
                u32::from(p[3]) * 2
            );
        }
        style.revise(|s| {
            s.opacity = Some(1.0);
            s.radius = Some(0.0);
            s.border_style = Some(BorderStyle::None);
        });
        app.flush().unwrap();
        raster.draw(&renderer).unwrap();
        assert_eq!(
            pixel(&raster, 0, 0),
            [0, 255, 0, 255],
            "removing the border restores child placement"
        );
        app.clear().unwrap();
        raster.draw(&renderer).unwrap();
        assert!(raster.pixels().iter().all(|v| *v == 0));
    }

    #[test]
    fn distinct_documents_for_one_runtime_never_reuse_each_others_pixels() {
        use glory_render::{Descriptor, Mutation, NodeIds, Props, Render, RuntimeId};
        let runtime = RuntimeId::new();
        let node = NodeIds::new(runtime).allocate();
        let mut blue = Renderer::new(runtime);
        let mut red = Renderer::new(runtime);
        let mut descriptor = Descriptor::new(Props::View);
        descriptor.classes = vec!["box".into()];
        let commands = [
            Mutation::Create {
                id: node,
                node: Box::new(descriptor),
            },
            Mutation::Place {
                id: node,
                parent: None,
                before: None,
            },
        ];
        for (host, color) in [(&mut blue, "blue"), (&mut red, "red")] {
            host.set_viewport(8, 8, 1.0).unwrap();
            host.set_css(&format!(".box{{width:8px;height:8px;background:{color}}}"));
            host.apply(&commands).unwrap();
        }
        let a = blue.frame_info();
        let b = red.frame_info();
        assert_eq!(a.runtime, b.runtime);
        assert_eq!(a.revision, b.revision);
        assert_eq!(a.applied, b.applied);
        assert_ne!(a.document, b.document);
        let mut raster = Raster::new();
        raster.draw(&blue).unwrap();
        assert_eq!(pixel(&raster, 4, 4), [0, 0, 255, 255]);
        assert!(raster.draw(&red).unwrap().rendered);
        assert_eq!(pixel(&raster, 4, 4), [255, 0, 0, 255]);
        assert!(!raster.draw(&red.clone()).unwrap().rendered);
    }

    #[test]
    fn dimensions_and_allocation_budget_fail_before_replacing_the_last_good_frame() {
        let (app, renderer) = fixture(".box{width:100%;height:100%;background:blue}", 8, 8);
        app.mount(view().class("box")).unwrap();
        let mut raster = Raster::with_limit(256);
        raster.draw(&renderer).unwrap();
        let saved = raster.pixels().to_vec();
        let frame = raster.frame();
        renderer.set_viewport(9, 8, 1.0).unwrap();
        assert_eq!(
            raster.draw(&renderer),
            Err(RasterError::Limit {
                bytes: 288,
                limit: 256
            })
        );
        assert_eq!(raster.pixels(), saved);
        assert_eq!(raster.frame(), frame);
        renderer.set_viewport(65536, 1, 1.0).unwrap();
        assert_eq!(
            raster.draw(&renderer),
            Err(RasterError::Dimensions {
                width: 65536,
                height: 1
            })
        );
        assert_eq!(raster.frame(), frame);
        assert!(renderer.set_viewport(800, 600, f32::from_bits(1)).is_err());
        renderer.set_viewport(0, 40, 1.0).unwrap();
        assert!(!raster.draw(&renderer).unwrap().rendered);
        assert!(raster.pixels().is_empty());
        assert_eq!(raster.buffer_capacity(), 0);
        renderer.set_viewport(8, 8, 1.0).unwrap();
        assert!(raster.draw(&renderer).unwrap().rendered);
        assert_eq!(raster.renders(), 2);
    }

    #[test]
    fn hundred_million_rows_paint_visible_stripes_with_one_bounded_buffer() {
        for adaptive in [false, true] {
            let (app, renderer) = fixture("", 160, 120);
            let position = app.scope().cage(0.0);
            let reads = Rc::new(Cell::new(0));
            let source = reads.clone();
            let latest = Rc::new(RefCell::new(None));
            let inspect = latest.clone();
            let widget = list(
                100_000_000usize,
                move |i| {
                    source.set(source.get() + 1);
                    i
                },
                move |_, key| {
                    view()
                        .style(Style {
                            background: Some(if key % 2 == 0 {
                                Color(0, 0, 255, 255)
                            } else {
                                Color(255, 0, 0, 255)
                            }),
                            ..Style::new().height(Length::Px(if adaptive {
                                24.0 + (key % 3) as f64 * 4.0
                            } else {
                                24.0
                            }))
                        })
                        .fill(text().content(format!("row {key}")))
                },
            )
            .height(120.0)
            .overscan(2)
            .position(position)
            .inspect(move |value| *inspect.borrow_mut() = Some(value));
            app.mount(if adaptive {
                widget.measure(24.0).measure_limit(64)
            } else {
                widget.row_height(24.0)
            })
            .unwrap();
            let root = app.tree().roots()[0];
            let mut raster = Raster::new();
            let mut scene = anyrender::Scene::new();
            let mut max_commands = 0;
            let mut max_nodes = 0;
            let mut max_rows = 0;
            for _ in 0..4 {
                app.pump().unwrap();
            }
            raster.draw(&renderer).unwrap();
            let buffer = raster.pixels().as_ptr();
            for step in 0..30 {
                let index = 1_000_000 + step * 7919;
                position.revise(|offset| *offset = index as f64 * 24.0);
                app.flush().unwrap();
                for _ in 0..4 {
                    app.pump().unwrap();
                }
                assert!(raster.draw(&renderer).unwrap().rendered);
                assert_eq!(raster.pixels().as_ptr(), buffer);
                assert_eq!(raster.buffer_capacity(), 160 * 120 * 4);
                if !adaptive {
                    let expected = if index % 2 == 0 {
                        [0, 0, 255, 255]
                    } else {
                        [255, 0, 0, 255]
                    };
                    assert_eq!(pixel(&raster, 150, 22), expected);
                }
                // Pick a complete physical row after paint/layout, including the
                // measured-height path. The pixel must match that semantic row's
                // color at its actual coordinates, not just an estimated index.
                let surface = app.tree().node(root).unwrap().children[0];
                let rows = app.tree().node(surface).unwrap().children.clone();
                let sample = rows
                    .into_iter()
                    .find_map(|row| {
                        let native = renderer.engine_id(row, glory_html::Part::Main).unwrap();
                        let rect = renderer
                            .inspect(|doc| doc.get_client_bounding_rect(native))
                            .unwrap();
                        if rect.y < 0.0 || rect.y + rect.height > 120.0 {
                            return None;
                        }
                        let child = app.tree().node(row).unwrap().children[0];
                        let color = app
                            .tree()
                            .node(child)
                            .unwrap()
                            .descriptor
                            .style
                            .background
                            .unwrap();
                        Some((
                            (rect.y + rect.height - 2.0) as usize,
                            [color.0, color.1, color.2, color.3],
                        ))
                    })
                    .expect("fully visible row");
                assert_eq!(pixel(&raster, 150, sample.0), sample.1);
                renderer.paint(&mut scene);
                max_commands = max_commands.max(scene.commands.len());
                max_nodes = max_nodes.max(renderer.stats().engine_nodes);
                assert!(scene.commands.len() < 512);
                let rows = latest.borrow().as_ref().unwrap().rows;
                max_rows = max_rows.max(rows);
                // A non-aligned measured window can cover two partial rows:
                // ceil(120/24) + one partial row + two overscan rows per side.
                assert!(rows <= if adaptive { 10 } else { 9 });
                assert!(renderer.stats().engine_nodes < 64);
            }
            assert!(reads.get() < 4000);
            assert_eq!(raster.renders(), 31);
            app.clear().unwrap();
            raster.draw(&renderer).unwrap();
            assert!(raster.pixels().iter().all(|byte| *byte == 0));
            assert_eq!(renderer.stats().nodes, 0);
            eprintln!(
                "Blitz raster 100M adaptive={adaptive}: rows={max_rows}, engine_nodes={max_nodes}, commands={max_commands}, buffer_bytes={}, key_reads={}",
                raster.pixels().len(),
                reads.get()
            );
            raster.clear();
            assert_eq!(raster.buffer_capacity(), 0);
        }
    }
}
