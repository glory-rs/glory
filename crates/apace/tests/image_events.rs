#![cfg(not(target_arch = "wasm32"))]
use glory_apace::assets::{Bytes, NetHandler, NetProvider, Request};
use glory_apace::{Config, Renderer};
use glory_render::{Event, EventKind, ImageStatus};
use glory_ui::{App, Widget, image};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

struct Job {
    url: String,
    handler: Box<dyn NetHandler>,
}
#[derive(Default)]
struct Provider(Mutex<Vec<Job>>);
impl Provider {
    fn take(&self, suffix: &str) -> Job {
        let mut jobs = self.0.lock().unwrap();
        let i = jobs
            .iter()
            .position(|job| job.url.ends_with(suffix))
            .expect("pending image");
        jobs.remove(i)
    }
}
impl NetProvider for Provider {
    fn fetch(&self, _: usize, request: Request, handler: Box<dyn NetHandler>) {
        self.0.lock().unwrap().push(Job {
            url: request.url.to_string(),
            handler,
        });
    }
}
fn fixture(config: Config) -> (App, Renderer) {
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::with_config(runtime, config);
        renderer = Some(host.clone());
        host
    });
    (app, renderer.unwrap())
}
fn png() -> Bytes {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[255, 0, 0, 255, 0, 0, 255, 255])
            .unwrap();
    }
    Bytes::from(bytes)
}
fn finish(job: Job) {
    job.handler.bytes(job.url, png());
}
fn settle(app: &App) {
    for _ in 0..4 {
        app.pump().unwrap();
    }
}
#[test]
fn actual_decode_completion_is_queued_after_engine_consumption_and_aba_and_cache_use_their_revision()
 {
    let provider = Arc::new(Provider::default());
    let (app, renderer) = fixture(Config {
        provider: Some(provider.clone()),
        ..Default::default()
    });
    let source = app.scope().cage("a.png".to_string());
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();
    app.mount(
        image()
            .source(source)
            .alt("Photo")
            .on(EventKind::ImageStatus, move |event| {
                seen.borrow_mut().push(event)
            }),
    )
    .unwrap();
    settle(&app);
    let id = app.tree().roots()[0];
    let physical = renderer.engine_id(id, glory_html::Part::Main).unwrap();
    let old_a = provider.take("a.png");
    source.revise(|value| *value = "b.png".into());
    app.flush().unwrap();
    let old_b = provider.take("b.png");
    source.revise(|value| *value = "a.png".into());
    app.flush().unwrap();
    let current = provider.take("a.png");
    finish(old_a);
    old_b.handler.error(old_b.url, "old failure".into());
    settle(&app);
    assert!(events.borrow().is_empty());
    finish(current);
    assert!(events.borrow().is_empty());
    settle(&app);
    assert_eq!(
        &*events.borrow(),
        &[Event::ImageStatus {
            revision: 3,
            status: ImageStatus::Loaded
        }]
    );
    app.scope().set_image_source(id, "a.png").unwrap();
    app.flush().unwrap();
    settle(&app);
    assert_eq!(events.borrow().len(), 2);
    assert_eq!(
        events.borrow()[1],
        Event::ImageStatus {
            revision: 4,
            status: ImageStatus::Loaded
        }
    );
    assert!(provider.0.lock().unwrap().is_empty());
    assert_eq!(
        renderer.engine_id(id, glory_html::Part::Main).unwrap(),
        physical
    );
    settle(&app);
    assert_eq!(events.borrow().len(), 2);
    assert_eq!(source.get(), "a.png");
}
#[test]
fn default_provider_admission_denial_and_empty_source_produce_finite_failure() {
    for config in [
        Config::default(),
        Config {
            provider: Some(Arc::new(Provider::default())),
            resources: apace_dom::ResourceLimits {
                pending_images: 0,
                ..Default::default()
            },
            ..Default::default()
        },
    ] {
        let (app, _renderer) = fixture(config);
        let events = Rc::new(RefCell::new(Vec::new()));
        let seen = events.clone();
        app.mount(
            image()
                .source("missing.png")
                .alt("Photo")
                .on(EventKind::ImageStatus, move |event| {
                    seen.borrow_mut().push(event)
                }),
        )
        .unwrap();
        settle(&app);
        assert_eq!(
            &*events.borrow(),
            &[Event::ImageStatus {
                revision: 1,
                status: ImageStatus::Failed
            }]
        );
        let id = app.tree().roots()[0];
        app.scope().set_image_source(id, "").unwrap();
        app.flush().unwrap();
        settle(&app);
        assert_eq!(
            events.borrow()[1],
            Event::ImageStatus {
                revision: 2,
                status: ImageStatus::Failed
            }
        );
    }
}
#[test]
fn suspended_consumer_restarts_and_owner_disposal_prevents_late_delivery() {
    let provider = Arc::new(Provider::default());
    let (app, renderer) = fixture(Config {
        provider: Some(provider.clone()),
        ..Default::default()
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();
    let mounted = image()
        .source("a.png")
        .alt("Photo")
        .on(EventKind::ImageStatus, move |event| {
            seen.borrow_mut().push(event)
        })
        .mount(app.scope())
        .unwrap();
    settle(&app);
    let old = provider.take("a.png");
    mounted.suspend().unwrap();
    app.flush().unwrap();
    finish(old);
    settle(&app);
    assert!(events.borrow().is_empty());
    mounted.place_before(None).unwrap();
    app.flush().unwrap();
    finish(provider.take("a.png"));
    settle(&app);
    assert_eq!(
        &*events.borrow(),
        &[Event::ImageStatus {
            revision: 1,
            status: ImageStatus::Loaded
        }]
    );
    mounted.place_before(None).unwrap();
    app.flush().unwrap();
    settle(&app);
    assert_eq!(events.borrow().len(), 1);
    let id = mounted.roots()[0];
    mounted.scope().set_image_source(id, "late.png").unwrap();
    app.flush().unwrap();
    let late = provider.take("late.png");
    drop(mounted);
    app.flush().unwrap();
    finish(late);
    settle(&app);
    assert_eq!(events.borrow().len(), 1);
    assert_eq!(app.stats().nodes, 0);
    assert_eq!(renderer.resource_stats().pending_images, 0);
}
