#![cfg(not(target_arch = "wasm32"))]

use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use glory_apace::assets::{Bytes, NetHandler, NetProvider, Request};
use glory_apace::{Config, Renderer};
use glory_html::Part;
use glory_render::style::{Length, Style};
use glory_render::{Props, Query, QueryValue};
use glory_ui::{App, Widget, group, image, list, text, view};

struct Job {
    doc: usize,
    url: String,
    handler: Box<dyn NetHandler>,
}
#[derive(Default)]
struct Provider {
    jobs: Mutex<VecDeque<Job>>,
    cancelled: AtomicUsize,
    closed: AtomicUsize,
    fetched: AtomicUsize,
    maximum: AtomicUsize,
}
impl Provider {
    fn take_url(&self, suffix: &str) -> Job {
        let mut jobs = self.jobs.lock().unwrap();
        let index = jobs
            .iter()
            .position(|job| job.url.ends_with(suffix))
            .expect("pending image URL");
        jobs.remove(index).unwrap()
    }
    fn take(&self) -> Job {
        self.jobs
            .lock()
            .unwrap()
            .pop_front()
            .expect("queued request")
    }
    fn len(&self) -> usize {
        self.jobs.lock().unwrap().len()
    }
    fn complete(&self, bytes: &[u8]) {
        let job = self.take();
        job.handler.bytes(job.url, Bytes::copy_from_slice(bytes));
    }
    fn all(&self, bytes: &[u8]) {
        // A completed stylesheet may itself request a font/import. This helper
        // is used only for image batches; application callbacks never run here.
        while self.len() != 0 {
            self.complete(bytes);
        }
    }
}
impl NetProvider for Provider {
    fn fetch(&self, doc: usize, request: Request, handler: Box<dyn NetHandler>) {
        assert_eq!(request.method, apace_traits::net::Method::GET);
        let mut jobs = self.jobs.lock().unwrap();
        jobs.push_back(Job {
            doc,
            url: request.url.to_string(),
            handler,
        });
        self.fetched.fetch_add(1, Ordering::Relaxed);
        self.maximum.fetch_max(jobs.len(), Ordering::Relaxed);
    }
    fn cancel(&self, doc: usize, url: &str) {
        let removed = {
            let mut jobs = self.jobs.lock().unwrap();
            let mut retained = VecDeque::new();
            let mut removed = Vec::new();
            while let Some(job) = jobs.pop_front() {
                if job.doc == doc && job.url == url {
                    removed.push(job);
                } else {
                    retained.push_back(job);
                }
            }
            *jobs = retained;
            removed
        };
        self.cancelled.fetch_add(removed.len(), Ordering::Relaxed);
        drop(removed);
    }
    fn close(&self, doc: usize) {
        let removed = {
            let mut jobs = self.jobs.lock().unwrap();
            let mut removed = Vec::new();
            let mut retained = VecDeque::new();
            while let Some(job) = jobs.pop_front() {
                if job.doc == doc {
                    removed.push(job);
                } else {
                    retained.push_back(job);
                }
            }
            *jobs = retained;
            removed
        };
        self.closed.fetch_add(1, Ordering::Relaxed);
        drop(removed);
    }
}
fn fixture(limits: apace_dom::ResourceLimits) -> (App, Renderer, Arc<Provider>) {
    let provider = Arc::new(Provider::default());
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::with_config(
            runtime,
            Config {
                provider: Some(provider.clone()),
                resources: limits,
                ..Config::default()
            },
        );
        renderer = Some(host.clone());
        host
    });
    (app, renderer.unwrap(), provider)
}
fn png(color: [u8; 4], width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&color.repeat((width * height) as usize))
            .unwrap();
    }
    bytes
}
fn settle(app: &App) {
    for _ in 0..5 {
        app.pump().unwrap();
    }
}

fn bounds(app: &App, node: glory_render::NodeId) -> glory_render::Rect {
    let output = std::rc::Rc::new(std::cell::RefCell::new(None));
    let result = output.clone();
    app.request(Query::Bounds(node), move |value| {
        *result.borrow_mut() = Some(value)
    })
    .unwrap();
    app.pump().unwrap();
    let QueryValue::Bounds(value) = output.borrow_mut().take().unwrap().unwrap() else {
        panic!("bounds")
    };
    value
}

fn image_rows(app: &App) -> Vec<(usize, glory_render::NodeId)> {
    let tree = app.tree();
    let root = tree.roots()[0];
    let surface = tree.node(root).unwrap().children[0];
    tree.node(surface)
        .unwrap()
        .children
        .iter()
        .filter_map(|node| {
            tree.node(*node)
                .unwrap()
                .descriptor
                .view()
                .item
                .map(|item| (item.index, *node))
        })
        .collect()
}

#[test]
fn intrinsic_image_height_changes_preserve_a_huge_list_anchor() {
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    let (app, renderer, provider) = fixture(apace_dom::ResourceLimits {
        image_cache_entries: 8,
        image_cache_bytes: 64 * 1024,
        pending_images: 64,
        ..Default::default()
    });
    renderer.set_css(".photo{display:block;min-height:24px}");
    let position = app.scope().cage(24_000_007.);
    let replace = app.scope().cage(None::<usize>);
    let reads = Rc::new(Cell::new(0));
    let source = reads.clone();
    let latest = Rc::new(RefCell::new(None));
    let inspected = latest.clone();
    app.mount(
        list(
            100_000_000_usize,
            move |index| {
                source.set(source.get() + 1);
                index
            },
            move |_, key| {
                image()
                    .source(
                        replace
                            .map(move |changed| {
                                if *changed == Some(key) {
                                    format!("intrinsic/{key}-small.png")
                                } else {
                                    format!("intrinsic/{key}.png")
                                }
                            })
                            .with_partial_eq(),
                    )
                    .class("photo")
                    .alt("photo")
            },
        )
        .height(180.)
        .measure(24.)
        .measure_limit(64)
        .overscan(2)
        .row_limit(32)
        .position(position)
        .inspect(move |stats| *inspected.borrow_mut() = Some(stats)),
    )
    .unwrap();
    settle(&app);
    settle(&app);
    let root = app.tree().roots()[0];
    let outer = bounds(&app, root);
    let rows = image_rows(&app);
    let (key, anchor) = rows
        .iter()
        .copied()
        .find(|(_, node)| {
            let rect = bounds(&app, *node);
            rect.y <= outer.y + 0.03 && rect.y + rect.height > outer.y + 0.03
        })
        .unwrap();
    let before = bounds(&app, anchor);
    eprintln!(
        "intrinsic initial: key={key}, rect={before:?}, rows={rows:?}, stats={:?}",
        latest.borrow()
    );
    assert_eq!(key, 1_000_000);
    assert!((outer.y - before.y - 7.).abs() < 0.03);
    // Grow an overscan image before the visible anchor, with no CSS height.
    let previous = key - 1;
    let above = rows.iter().find(|(index, _)| *index == previous).unwrap().1;
    let old_height = bounds(&app, above).height;
    let old_logical = latest.borrow().as_ref().unwrap().logical;
    let job = provider.take_url(&format!("/{previous}.png"));
    job.handler
        .bytes(job.url, Bytes::from(png([1, 2, 3, 255], 8, 96)));
    settle(&app);
    settle(&app);
    assert_eq!(
        image_rows(&app)
            .iter()
            .find(|(index, _)| *index == key)
            .unwrap()
            .1,
        anchor
    );
    let after = bounds(&app, anchor);
    let grown = bounds(&app, above).height;
    let logical = latest.borrow().as_ref().unwrap().logical;
    eprintln!(
        "intrinsic grown: before={before:?}, after={after:?}, height={old_height}->{grown}, logical={old_logical}->{logical}"
    );
    assert!((grown - 96.).abs() < 0.03);
    assert!((after.y - before.y).abs() < 0.03);
    assert!((logical - old_logical - (grown - old_height)).abs() < 0.03);
    let &(retired_key, retired_row) = rows.last().unwrap();
    let retired = provider.take_url(&format!("/{retired_key}.png"));
    let job = provider.take_url(&format!("/{key}.png"));
    job.handler
        .bytes(job.url, Bytes::from(png([4, 5, 6, 255], 8, 128)));
    settle(&app);
    settle(&app);
    assert!((bounds(&app, anchor).height - 128.).abs() < 0.03);
    assert!((bounds(&app, anchor).y - before.y).abs() < 0.03);
    assert!(app.tree().node(retired_row).is_err());
    let cached = renderer.resource_stats().cached_images;
    retired
        .handler
        .bytes(retired.url, Bytes::from(png([99, 98, 97, 255], 8, 88)));
    settle(&app);
    assert_eq!(renderer.resource_stats().cached_images, cached);
    assert!(
        (bounds(&app, anchor).y - before.y).abs() < 0.03,
        "stale image must not move the anchor"
    );
    replace.revise(|value| *value = Some(key));
    app.flush().unwrap();
    settle(&app);
    assert!((bounds(&app, anchor).height - 24.).abs() < 0.03);
    assert!((bounds(&app, anchor).y - before.y).abs() < 0.03);
    let job = provider.take_url(&format!("/{key}-small.png"));
    job.handler
        .bytes(job.url, Bytes::from(png([7, 8, 9, 255], 8, 32)));
    settle(&app);
    settle(&app);
    assert!((bounds(&app, anchor).height - 32.).abs() < 0.03);
    assert!((bounds(&app, anchor).y - before.y).abs() < 0.03);
    for step in 0..40 {
        position.revise(|value| *value = 100_000_007. + step as f64 * 19_123_000.);
        settle(&app);
        let stats = latest.borrow().clone().unwrap();
        assert!(
            stats.rows <= 16 && stats.measured <= 64 && stats.height_capacity < 2000,
            "{stats:?}"
        );
        assert!(
            provider.len() <= 16 && app.stats().nodes < 64 && app.runtime_stats().node_slots < 256
        );
    }
    position.revise(|value| *value = 24_000_007.);
    settle(&app);
    settle(&app);
    let restored = image_rows(&app)
        .iter()
        .find(|(index, _)| *index == key)
        .unwrap()
        .1;
    assert_ne!(
        restored, anchor,
        "offscreen rows release their logical and engine identities"
    );
    assert!((bounds(&app, restored).y - before.y).abs() < 0.03);
    assert!(reads.get() < 4000, "indexed source reads={}", reads.get());
    eprintln!(
        "intrinsic anchor: key_reads={}, stats={:?}, resources={:?}",
        reads.get(),
        latest.borrow(),
        renderer.resource_stats()
    );
    app.clear().unwrap();
    assert_eq!(provider.len(), 0);
    renderer.poll_resources();
    assert_eq!(renderer.resource_stats().requests, 0);
}

#[cfg(all(
    feature = "shell",
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]
#[test]
fn navigation_reveals_an_already_measured_row_after_its_intrinsic_image_arrives() {
    use glory_apace::shell::WindowApp;
    use glory_render::{Action, Event, Navigation};
    let provider = Arc::new(Provider::default());
    let window = WindowApp::with_config(Config {
        provider: Some(provider.clone()),
        ..Default::default()
    });
    let app = window.app();
    let renderer = window.renderer();
    let position = app.scope().cage(0.);
    renderer.set_css(".photo{display:block;min-height:24px}");
    app.mount(
        list(
            100_000_000_usize,
            |index| index,
            |_, key| {
                image()
                    .source(format!("navigation/{key}.png"))
                    .class("photo")
                    .alt("photo")
            },
        )
        .height(180.)
        .measure(24.)
        .measure_limit(64)
        .overscan(2)
        .row_limit(32)
        .position(position)
        .on_scroll(move |value| position.revise(|p| *p = value)),
    )
    .unwrap();
    settle(app);
    settle(app);
    let root = app.tree().roots()[0];
    app.scope().action(Action::Focus(root)).unwrap();
    app.flush().unwrap();
    settle(app);
    app.dispatch(
        root,
        Event::Navigate {
            from: root,
            command: Navigation::Last,
        },
    )
    .unwrap();
    settle(app);
    settle(app);
    let last = image_rows(app)
        .iter()
        .find(|(key, _)| *key == 99_999_999)
        .unwrap()
        .1;
    let outer = bounds(app, root);
    let before = bounds(app, last);
    assert!((before.height - 24.).abs() < 0.03);
    assert!(before.y + before.height <= outer.y + outer.height + 0.03);
    let job = provider.take_url("/99999999.png");
    job.handler
        .bytes(job.url, Bytes::from(png([1, 2, 3, 255], 8, 96)));
    settle(app);
    settle(app);
    let after = bounds(app, last);
    eprintln!("intrinsic End navigation: before={before:?}, after={after:?}, viewport={outer:?}");
    assert!((after.height - 96.).abs() < 0.03);
    assert!(
        after.y >= outer.y - 0.03 && after.y + after.height <= outer.y + outer.height + 0.03,
        "late image must remain visible after keyboard navigation: {after:?}"
    );
    // Real user movement cancels the retained reveal intent. A later intrinsic
    // width/height change must not drag the viewport back to the focused row.
    let physical = std::rc::Rc::new(std::cell::RefCell::new(None));
    let result = physical.clone();
    app.request(Query::Scroll(root), move |value| {
        *result.borrow_mut() = Some(value)
    })
    .unwrap();
    settle(app);
    let QueryValue::Scroll { y, .. } = physical.borrow_mut().take().unwrap().unwrap() else {
        panic!("scroll")
    };
    app.scope()
        .action(Action::ScrollTo {
            node: root,
            x: 0.,
            y: y - 100.,
        })
        .unwrap();
    app.flush().unwrap();
    settle(app);
    let user_position = position.get();
    renderer.set_css(".photo{display:block;min-height:24px;width:16px}");
    settle(app);
    settle(app);
    assert!(
        (position.get() - user_position).abs() < 0.03,
        "user movement must retire the old navigation intent"
    );
    // A different program position also wins, even with the row still focused.
    position.revise(|p| *p = 0.);
    app.flush().unwrap();
    settle(app);
    renderer.set_css(".photo{display:block;min-height:24px;width:32px}");
    settle(app);
    settle(app);
    assert!(position.get().abs() < 0.03);
    app.scope().action(Action::Focus(root)).unwrap();
    app.flush().unwrap();
    settle(app);
    app.dispatch(
        root,
        Event::Navigate {
            from: root,
            command: Navigation::Last,
        },
    )
    .unwrap();
    settle(app);
    settle(app);
    let tall = image_rows(app)
        .iter()
        .find(|(key, _)| *key == 99_999_999)
        .unwrap()
        .1;
    let rect = bounds(app, tall);
    assert!(
        (rect.height - 384.).abs() < 0.03 && (rect.y - outer.y).abs() < 0.03,
        "oversized row must align at its start: {rect:?}"
    );
}
fn pixel(renderer: &Renderer, id: glory_render::NodeId) -> Option<[u8; 4]> {
    let node = renderer.engine_id(id, Part::Main).unwrap();
    renderer.inspect(|doc| {
        doc.get_node(node)
            .and_then(|node| node.element_data())
            .and_then(|element| element.raster_image_data())
            .map(|image| image.data.as_ref()[0..4].try_into().unwrap())
    })
}

#[test]
fn delayed_images_invalidate_geometry_and_pixels_without_manual_invalidation() {
    let (app, renderer, provider) = fixture(Default::default());
    app.mount(image().source("picture.png").alt("picture"))
        .unwrap();
    settle(&app);
    let node = app.tree().roots()[0];
    assert_eq!(provider.len(), 1);
    assert_eq!(renderer.resource_stats().pending_images, 1);
    #[cfg(feature = "raster")]
    let mut raster = glory_apace::Raster::new();
    #[cfg(feature = "raster")]
    {
        renderer.set_viewport(32, 32, 1.).unwrap();
        assert!(raster.draw(&renderer).unwrap().rendered);
        assert!(!raster.draw(&renderer).unwrap().rendered);
        assert!(
            !raster
                .pixels()
                .as_chunks::<4>()
                .0
                .contains(&[12, 180, 45, 255])
        );
    }
    let before = renderer.stats().visual_revision;
    let encoded = png([12, 180, 45, 255], 8, 8);
    provider.complete(&encoded);
    #[cfg(feature = "raster")]
    {
        let frame = raster.draw(&renderer).unwrap();
        assert!(
            frame.rendered,
            "a resource completion must bypass the unchanged-pixel fast path"
        );
        assert_eq!(frame.info.revision, renderer.stats().visual_revision);
        assert!(!raster.draw(&renderer).unwrap().rendered);
    }
    settle(&app);
    assert!(renderer.stats().visual_revision > before);
    assert_eq!(pixel(&renderer, node), Some([12, 180, 45, 255]));
    let dimensions = std::rc::Rc::new(std::cell::RefCell::new(None));
    let result = dimensions.clone();
    app.request(Query::Bounds(node), move |value| {
        *result.borrow_mut() = Some(value)
    })
    .unwrap();
    settle(&app);
    let QueryValue::Bounds(bounds) = dimensions.borrow_mut().take().unwrap().unwrap() else {
        panic!("bounds")
    };
    assert_eq!((bounds.width, bounds.height), (8., 8.));
    #[cfg(feature = "raster")]
    {
        assert!(
            raster
                .pixels()
                .as_chunks::<4>()
                .0
                .contains(&[12, 180, 45, 255])
        );
    }
    app.clear().unwrap();
    assert_eq!(renderer.resource_stats().pending_images, 0);
}

#[test]
fn unique_images_in_huge_fixed_and_measured_lists_bound_cache_pending_jobs_and_history() {
    for measured in [false, true] {
        let (app, renderer, provider) = fixture(apace_dom::ResourceLimits {
            image_cache_entries: 8,
            image_cache_bytes: 1024,
            pending_images: 48,
            ..Default::default()
        });
        let position = app.scope().cage(0.0);
        let reads = std::rc::Rc::new(std::cell::Cell::new(0));
        let source_reads = reads.clone();
        let widget = list(
            100_000_000_usize,
            move |index| {
                source_reads.set(source_reads.get() + 1);
                index
            },
            move |_, key| {
                image()
                    .source(format!("rows/{key}.png"))
                    .alt("row")
                    .style(Style::new().height(Length::Px(if measured {
                        [24., 28., 32.][key % 3]
                    } else {
                        24.
                    })))
            },
        )
        .height(180.)
        .overscan(2)
        .row_limit(20)
        .position(position);
        app.mount(if measured {
            widget.measure(24.).measure_limit(64)
        } else {
            widget.row_height(24.)
        })
        .unwrap();
        let bytes = png([1, 2, 3, 255], 8, 8);
        for step in 0..80 {
            position.revise(|value| *value = (step * 19_123_000) as f64);
            settle(&app);
            // Abandon one entire window every other turn: it must be cancelled
            // before the next window's responses arrive, not kept as history.
            if step % 2 == 0 {
                provider.all(&bytes);
                settle(&app);
            }
            let resources = renderer.resource_stats();
            assert!(
                resources.cached_images <= 8 && resources.cached_image_bytes <= 1024,
                "{resources:?}"
            );
            assert!(
                resources.pending_images <= 16 && resources.image_waiters <= 16,
                "{resources:?}"
            );
            assert!(provider.len() <= 16);
            assert!(app.stats().nodes < 64);
            assert!(app.runtime_stats().node_slots < 160);
        }
        assert!(renderer.resource_stats().image_evictions > 100);
        assert!(provider.cancelled.load(Ordering::Relaxed) > 100);
        assert!(provider.maximum.load(Ordering::Relaxed) <= 32);
        assert!(reads.get() < 4000, "no source scan: {}", reads.get());
        app.clear().unwrap();
        assert_eq!(provider.len(), 0);
        assert_eq!(renderer.resource_stats().pending_images, 0);
        assert_eq!(renderer.resource_stats().image_waiters, 0);
        drop(app);
        drop(renderer);
        assert_eq!(provider.closed.load(Ordering::Relaxed), 1);
    }
}

#[test]
fn stale_response_for_an_abandoned_url_cannot_fill_a_new_waiter_or_reused_slot() {
    let (app, renderer, provider) = fixture(Default::default());
    let source = app.scope().cage(String::from("same.png"));
    app.mount(image().source(source).alt("dynamic")).unwrap();
    let node = app.tree().roots()[0];
    let old = provider.take();
    source.revise(|value| *value = "next.png".into());
    settle(&app);
    source.revise(|value| *value = "same.png".into());
    settle(&app);
    assert_eq!(provider.len(), 1);
    old.handler
        .bytes(old.url, Bytes::from(png([255, 0, 0, 255], 8, 8)));
    settle(&app);
    assert_eq!(pixel(&renderer, node), None);
    assert_eq!(renderer.resource_stats().pending_images, 1);
    provider.complete(&png([0, 0, 255, 255], 8, 8));
    settle(&app);
    assert_eq!(pixel(&renderer, node), Some([0, 0, 255, 255]));
    source.revise(|value| *value = String::new());
    settle(&app);
    assert_eq!(
        pixel(&renderer, node),
        None,
        "clearing src releases the node's decoded image"
    );
    source.revise(|value| *value = "reuse.png".into());
    settle(&app);
    let retired = provider.take();
    let retired_physical = renderer.engine_id(node, Part::Main).unwrap();
    app.clear().unwrap();
    app.mount(image().source("reuse.png").alt("new generation"))
        .unwrap();
    settle(&app);
    let next = app.tree().roots()[0];
    assert_ne!(next, node);
    assert_ne!(
        renderer.engine_id(next, Part::Main).unwrap(),
        retired_physical
    );
    retired
        .handler
        .bytes(retired.url, Bytes::from(png([255, 0, 0, 255], 8, 8)));
    settle(&app);
    assert_eq!(
        pixel(&renderer, next),
        None,
        "the old request cannot populate a recycled logical/engine slot"
    );
    assert_eq!(renderer.resource_stats().pending_images, 1);
    provider.complete(&png([0, 0, 255, 255], 8, 8));
    settle(&app);
    assert_eq!(pixel(&renderer, next), Some([0, 0, 255, 255]));
    app.clear().unwrap();
    app.mount(image().source("reuse.png").alt("cache hit"))
        .unwrap();
    settle(&app);
    assert!(
        provider.len() == 0,
        "a valid bounded reuse-cache hit needs no fetch"
    );
}

#[test]
fn pending_admission_decode_dimensions_and_invalid_urls_are_explicit_and_bounded() {
    let (app, renderer, provider) = fixture(apace_dom::ResourceLimits {
        pending_images: 2,
        image_waiters: 3,
        image_decoded_bytes: 128,
        ..Default::default()
    });
    app.mount(
        group().fill(
            (0..10)
                .map(|index| {
                    image()
                        .source(if index < 5 {
                            "same.png".into()
                        } else {
                            format!("{index}.png")
                        })
                        .alt("image")
                })
                .collect::<Vec<_>>(),
        ),
    )
    .unwrap();
    settle(&app);
    let stats = renderer.resource_stats();
    assert_eq!(
        (provider.len(), stats.pending_images, stats.image_waiters),
        (2, 2, 4)
    );
    assert_eq!(stats.rejected, 6);
    provider.all(&png([1, 2, 3, 255], 8, 8));
    settle(&app);
    assert_eq!(renderer.resource_stats().cached_images, 0);
    assert_eq!(renderer.resource_stats().failures, 2);
    assert_eq!(renderer.resource_stats().pending_images, 0);
    app.clear().unwrap();
    app.mount(image().source("http://[").alt("invalid URL"))
        .unwrap();
    settle(&app);
    assert_eq!(provider.len(), 0);
    assert!(renderer.resource_stats().failures >= 3);
}

#[test]
fn suspended_image_and_background_waiters_cancel_and_resume_without_accepting_stale_data() {
    let (app, renderer, provider) = fixture(Default::default());
    renderer.set_css(".picture{width:16px;height:16px;background-image:url('shared.png')}");
    let mounted = view()
        .class("picture")
        .fill(image().source("shared.png").alt("shared"))
        .mount(app.scope())
        .unwrap();
    settle(&app);
    assert_eq!(provider.len(), 1);
    assert_eq!(renderer.resource_stats().image_waiters, 2);
    let old = provider.take();
    mounted.suspend().unwrap();
    app.flush().unwrap();
    assert_eq!(renderer.resource_stats().pending_images, 0);
    mounted.place_before(None).unwrap();
    app.flush().unwrap();
    settle(&app);
    assert_eq!(provider.len(), 1);
    assert_eq!(renderer.resource_stats().image_waiters, 2);
    old.handler
        .bytes(old.url, Bytes::from(png([255, 0, 0, 255], 8, 8)));
    settle(&app);
    assert_eq!(renderer.resource_stats().pending_images, 1);
    provider.complete(&png([0, 0, 255, 255], 8, 8));
    settle(&app);
    let root = mounted.roots()[0];
    let image = app.tree().node(root).unwrap().children[0];
    assert_eq!(pixel(&renderer, image), Some([0, 0, 255, 255]));
    let physical = renderer.engine_id(root, Part::Main).unwrap();
    renderer.inspect(|doc| {
        let background = doc
            .get_node(physical)
            .unwrap()
            .element_data()
            .unwrap()
            .background_images[0]
            .as_ref()
            .unwrap();
        assert_eq!(background.status, apace_dom::node::Status::Ok);
    });
    drop(mounted);
    app.flush().unwrap();
    assert_eq!(renderer.resource_stats().image_waiters, 0);
}

#[test]
fn external_import_font_and_background_complete_through_the_same_document_pipeline() {
    let (app, renderer, provider) = fixture(Default::default());
    renderer.set_css("@import url('theme.css'); .box{height:20px}");
    app.mount(view().class("box").fill(text().content("font fallback")))
        .unwrap();
    settle(&app);
    let job = provider.take();
    assert!(job.url.ends_with("theme.css"));
    job.handler.bytes(job.url,Bytes::from_static(b"@font-face{font-family:ResourceAlias;src:url('font.otf')} .box{width:73px;font-family:ResourceAlias;background-image:url('background.png')}"));
    settle(&app);
    while provider.len() != 0 {
        let job = provider.take();
        let bytes = if job.url.ends_with("font.otf") {
            apace_dom::BULLET_FONT.to_vec()
        } else {
            png([13, 41, 89, 255], 8, 8)
        };
        job.handler.bytes(job.url, Bytes::from(bytes));
        settle(&app);
    }
    assert_eq!(renderer.resource_stats().registered_fonts, 1);
    assert_eq!(
        renderer.resource_stats().registered_font_bytes,
        apace_dom::BULLET_FONT.len()
    );
    assert_eq!(renderer.resource_stats().cached_images, 1);
    let node = app.tree().roots()[0];
    assert!(matches!(
        app.tree().node(node).unwrap().descriptor.view().props,
        Props::View
    ));
    let physical = renderer.engine_id(node, Part::Main).unwrap();
    assert_eq!(
        renderer.inspect(|doc| doc.get_client_bounding_rect(physical).unwrap().width),
        73.0
    );
}

fn linked_document(provider: Arc<Provider>) -> apace_dom::BaseDocument {
    let mut doc = apace_dom::BaseDocument::new(apace_dom::DocumentConfig {
        base_url: Some("glory://app/".into()),
        net_provider: Some(provider),
        ..Default::default()
    });
    let tag = |name: &str| {
        apace_dom::QualName::new(
            None,
            apace_dom::Namespace::from("http://www.w3.org/1999/xhtml"),
            name.into(),
        )
    };
    let root = doc.root_node().id;
    {
        let mut m = doc.mutate();
        let html = m.create_element(tag("html"), Vec::new());
        let head = m.create_element(tag("head"), Vec::new());
        let link = m.create_element(
            tag("link"),
            vec![
                apace_dom::Attribute {
                    name: apace_dom::QualName::new(None, "".into(), "rel".into()),
                    value: "stylesheet".into(),
                },
                apace_dom::Attribute {
                    name: apace_dom::QualName::new(None, "".into(), "href".into()),
                    value: "missing.css".into(),
                },
            ],
        );
        m.append_children(root, &[html]);
        m.append_children(html, &[head]);
        m.append_children(head, &[link]);
    }
    doc
}

#[test]
fn linked_stylesheet_redirect_resolves_imports_against_the_final_url() {
    let provider = Arc::new(Provider::default());
    let mut doc = linked_document(provider.clone());
    let job = provider.take();
    job.handler.bytes(
        "glory://cdn/theme/final.css".into(),
        Bytes::from_static(b"@import url('child.css');"),
    );
    doc.handle_messages();
    assert!(!doc.has_pending_critical_resources());
    assert_eq!(provider.take().url, "glory://cdn/theme/child.css");
}

#[test]
fn rejected_stylesheet_admission_never_leaves_an_unfulfillable_critical_request() {
    let provider = Arc::new(Provider::default());
    let mut doc = linked_document(provider.clone());
    doc.set_resource_limits(apace_dom::ResourceLimits {
        requests: 1,
        ..Default::default()
    })
    .unwrap();
    let head = doc.root_element().children[0];
    {
        let mut m = doc.mutate();
        let link = m.create_element(
            apace_dom::QualName::new(None, "http://www.w3.org/1999/xhtml".into(), "link".into()),
            vec![
                apace_dom::Attribute {
                    name: apace_dom::QualName::new(None, "".into(), "rel".into()),
                    value: "stylesheet".into(),
                },
                apace_dom::Attribute {
                    name: apace_dom::QualName::new(None, "".into(), "href".into()),
                    value: "second.css".into(),
                },
            ],
        );
        m.append_children(head, &[link]);
    }
    assert_eq!(provider.len(), 1);
    assert_eq!(doc.resource_stats().requests, 1);
    assert_eq!(doc.resource_stats().rejected, 1);
    let job = provider.take();
    job.handler.error(job.url, "first failed".into());
    assert!(doc.handle_messages());
    assert!(!doc.has_pending_critical_resources());
    assert_eq!(doc.resource_stats().requests, 0);
    assert_eq!(doc.resource_stats().completions, 0);
}

#[test]
fn stylesheet_failure_unblocks_rendering_and_document_drop_closes_pending_requests() {
    let provider = Arc::new(Provider::default());
    let mut doc = linked_document(provider.clone());
    let tag = |name: &str| {
        apace_dom::QualName::new(
            None,
            apace_dom::Namespace::from("http://www.w3.org/1999/xhtml"),
            name.into(),
        )
    };
    assert!(doc.has_pending_critical_resources());
    let job = provider.take();
    job.handler.error(job.url, "fixture: missing asset".into());
    assert!(doc.handle_messages());
    assert!(!doc.has_pending_critical_resources());
    assert_eq!(doc.resource_stats().failures, 1);
    let html = doc.root_element().id;
    {
        let mut m = doc.mutate();
        let body = m.create_element(tag("body"), Vec::new());
        let image = m.create_element(
            tag("img"),
            vec![apace_dom::Attribute {
                name: apace_dom::QualName::new(None, "".into(), "src".into()),
                value: "pending.png".into(),
            }],
        );
        m.append_children(html, &[body]);
        m.append_children(body, &[image]);
    }
    assert_eq!(provider.len(), 1);
    drop(doc);
    assert_eq!(provider.closed.load(Ordering::Relaxed), 1);
    assert_eq!(provider.len(), 0);
}

#[test]
fn registered_font_bytes_and_entries_have_a_document_lifetime_budget() {
    let (app, renderer, provider) = fixture(apace_dom::ResourceLimits {
        font_entries: 1,
        font_bytes: apace_dom::BULLET_FONT.len(),
        ..Default::default()
    });
    renderer.set_css("@font-face{font-family:Invalid;src:url('invalid.otf')}");
    app.mount(text().content("font budgets")).unwrap();
    settle(&app);
    provider.complete(b"OTTO invalid font body");
    settle(&app);
    assert_eq!(renderer.resource_stats().registered_fonts, 0);
    assert_eq!(renderer.resource_stats().failures, 1);
    renderer.set_css("@font-face{font-family:First;src:url('first.otf')} @font-face{font-family:Second;src:url('second.otf')}");
    settle(&app);
    assert_eq!(provider.len(), 2);
    provider.all(apace_dom::BULLET_FONT);
    settle(&app);
    let stats = renderer.resource_stats();
    assert_eq!(stats.registered_fonts, 1);
    assert_eq!(stats.registered_font_bytes, apace_dom::BULLET_FONT.len());
    assert_eq!(stats.rejected, 1);
}

#[test]
fn resource_completions_wake_the_current_host_once_and_late_close_does_not_wake() {
    struct Wake(AtomicUsize);
    impl std::task::Wake for Wake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    let (app, renderer, provider) = fixture(Default::default());
    app.mount(group().fill((
        image().source("a.png").alt("a"),
        image().source("b.png").alt("b"),
    )))
    .unwrap();
    let wake = Arc::new(Wake(AtomicUsize::new(0)));
    let waker = std::task::Waker::from(wake.clone());
    renderer.observe_resources(&waker);
    let first = provider.take();
    let second = provider.take();
    // Handlers were created before any window shell existed. Their stable relay
    // observes the current host, instead of the DummyShell captured at fetch.
    std::thread::spawn(move || {
        for job in [first, second] {
            job.handler
                .bytes(job.url, Bytes::from(png([1, 2, 3, 255], 8, 8)));
        }
    })
    .join()
    .unwrap();
    assert_eq!(wake.0.load(Ordering::Relaxed), 1);
    settle(&app);
    assert_eq!(renderer.resource_stats().pending_images, 0);
    renderer.observe_resources(&waker);
    app.clear().unwrap();
    app.mount(image().source("late.png").alt("late")).unwrap();
    let late = provider.take();
    drop(app);
    drop(renderer);
    let before = wake.0.load(Ordering::Relaxed);
    late.handler
        .bytes(late.url, Bytes::from(png([255, 0, 0, 255], 8, 8)));
    assert_eq!(wake.0.load(Ordering::Relaxed), before);
    assert_eq!(provider.closed.load(Ordering::Relaxed), 1);
}
