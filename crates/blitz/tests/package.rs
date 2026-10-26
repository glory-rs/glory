#![cfg(all(feature = "package", not(target_arch = "wasm32")))]

use glory_blitz::assets::{
    Bytes, LoadConfig, LoadLimits, Loader, NetHandler, NetProvider, NetReady, Request, Url,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

type Reply = Result<Bytes, String>;
struct Handler(mpsc::Sender<Reply>);
impl NetHandler for Handler {
    fn bytes(self: Box<Self>, _: String, bytes: Bytes) {
        let _ = self.0.send(Ok(bytes));
    }
    fn error(self: Box<Self>, _: String, error: String) {
        let _ = self.0.send(Err(error));
    }
}
fn wait(mut condition: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < until, "package fixture timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn config() -> LoadConfig {
    LoadConfig {
        package: BTreeMap::from([("/theme.css".into(), Bytes::from_static(b"body{color:red}"))]),
        ..Default::default()
    }
}
fn fetch(loader: &Loader, doc: usize, request: Request) -> mpsc::Receiver<Reply> {
    let (tx, rx) = mpsc::channel();
    loader.fetch(doc, request, Box::new(Handler(tx)));
    rx
}
fn request(url: &str) -> Request {
    Request::get(Url::parse(url).unwrap())
}
fn stop(loader: &Loader) {
    loader.shutdown();
    wait(|| loader.stats().stopped);
    assert_eq!((loader.stats().queued, loader.stats().active), (0, 0));
}

#[test]
fn package_get_is_document_owned_and_has_no_ambient_authority() {
    let loader = Loader::new(config()).unwrap();
    assert_eq!(
        fetch(&loader, 1, request("asset://app/theme.css?v=1#part"))
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap(),
        b"body{color:red}"[..]
    );
    for url in [
        "https://example.com/theme.css",
        "file:///theme.css",
        "data:text/css,x",
        "asset://other/theme.css",
        "asset://user@app/theme.css",
        "asset://app/missing.css",
    ] {
        assert!(
            fetch(&loader, 1, request(url))
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .is_err(),
            "{url}"
        );
    }
    let mut post = request("asset://app/theme.css");
    post.method = glory_blitz::assets::Method::POST;
    assert!(
        fetch(&loader, 1, post)
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .is_err()
    );
    assert!(
        fetch(&loader, 2, request("asset://app/theme.css"))
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap_err()
            .contains("another document")
    );
    loader.close(2);
    assert!(
        !loader.stats().closed,
        "foreign close cannot retire the owner"
    );
    loader.close(1);
    wait(|| loader.stats().stopped);
    assert!(
        fetch(&loader, 1, request("asset://app/theme.css"))
            .recv_timeout(Duration::from_millis(20))
            .is_err()
    );
    assert_eq!(
        (loader.stats().file_active, loader.stats().file_peak_active),
        (0, 0)
    );
}

#[test]
fn package_paths_and_each_encoded_budget_are_checked_at_construction() {
    for path in [
        "theme.css",
        "/a/../theme.css",
        "//other/theme.css",
        "/a?query",
        "/a#fragment",
        "/space here",
    ] {
        let mut invalid = config();
        invalid.package = BTreeMap::from([(path.into(), Bytes::from_static(b"x"))]);
        assert!(Loader::new(invalid).is_err(), "{path}");
    }
    for limits in [
        LoadLimits {
            package_entries: 0,
            ..Default::default()
        },
        LoadLimits {
            package_bytes: 1,
            ..Default::default()
        },
        LoadLimits {
            body_bytes: 1,
            ..Default::default()
        },
        LoadLimits {
            queued: 0,
            ..Default::default()
        },
        LoadLimits {
            workers: 0,
            ..Default::default()
        },
    ] {
        assert!(Loader::new(LoadConfig { limits, ..config() }).is_err());
    }
    let loader = Loader::new(LoadConfig {
        package: BTreeMap::new(),
        limits: LoadLimits {
            package_bytes: 0,
            package_entries: 0,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    stop(&loader);
}

#[cfg(not(all(
    feature = "assets",
    any(target_os = "windows", target_os = "linux", target_os = "macos")
)))]
#[test]
fn package_only_build_rejects_http_and_mounted_file_configuration() {
    for config in [
        LoadConfig {
            origins: vec![Url::parse("https://example.com/").unwrap()],
            ..config()
        },
        LoadConfig {
            mounts: BTreeMap::from([("media".into(), std::path::PathBuf::from("/explicit/root"))]),
            ..config()
        },
    ] {
        assert_eq!(
            Loader::new(config).err().unwrap().kind(),
            std::io::ErrorKind::Unsupported
        );
    }
}

struct Gated {
    entered: mpsc::Sender<std::thread::ThreadId>,
    gate: Arc<tokio::sync::Notify>,
    reply: mpsc::Sender<Reply>,
    drops: Arc<AtomicUsize>,
}
impl NetHandler for Gated {
    fn prepare<'a>(&'a mut self, _: &'a str, _: &'a Bytes) -> NetReady<'a> {
        Box::pin(async move {
            let ready = self.gate.notified();
            self.entered.send(std::thread::current().id()).unwrap();
            ready.await;
            Ok(())
        })
    }
    fn bytes(self: Box<Self>, _: String, bytes: Bytes) {
        let _ = self.reply.send(Ok(bytes));
    }
    fn error(self: Box<Self>, _: String, error: String) {
        let _ = self.reply.send(Err(error));
    }
}
impl Drop for Gated {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn package_waits_for_decode_admission_and_cancel_releases_bounded_work() {
    let loader = Loader::new(LoadConfig {
        limits: LoadLimits {
            workers: 1,
            queued: 1,
            ..Default::default()
        },
        ..config()
    })
    .unwrap();
    let (entered, started) = mpsc::channel();
    let (reply, result) = mpsc::channel();
    let gate = Arc::new(tokio::sync::Notify::new());
    let drops = Arc::new(AtomicUsize::new(0));
    loader.fetch(
        7,
        request("asset://app/theme.css"),
        Box::new(Gated {
            entered,
            gate: gate.clone(),
            reply,
            drops: drops.clone(),
        }),
    );
    assert_ne!(
        started.recv_timeout(Duration::from_secs(5)).unwrap(),
        std::thread::current().id()
    );
    assert!(
        matches!(result.try_recv(), Err(mpsc::TryRecvError::Empty)),
        "bytes must wait for prepare"
    );
    let queued = fetch(&loader, 7, request("asset://app/theme.css"));
    let rejected = fetch(&loader, 7, request("asset://app/theme.css"));
    assert!(
        rejected
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap_err()
            .contains("queue")
    );
    loader.cancel(8, "asset://app/theme.css");
    assert_eq!(loader.stats().active, 1);
    loader.cancel(7, "asset://app/theme.css");
    wait(|| drops.load(Ordering::SeqCst) == 1 && loader.stats().active == 0);
    gate.notify_one();
    assert!(result.recv_timeout(Duration::from_millis(20)).is_err());
    assert!(queued.recv_timeout(Duration::from_millis(20)).is_err());
    assert!(loader.stats().peak_active <= 1 && loader.stats().peak_queued <= 1);
    assert_eq!(
        fetch(&loader, 7, request("asset://app/theme.css"))
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap(),
        b"body{color:red}"[..]
    );
    stop(&loader);
}

#[test]
fn close_drops_a_pending_prepare_without_late_decode_or_publication() {
    let loader = Loader::new(config()).unwrap();
    let (entered, started) = mpsc::channel();
    let (reply, result) = mpsc::channel();
    let gate = Arc::new(tokio::sync::Notify::new());
    let drops = Arc::new(AtomicUsize::new(0));
    loader.fetch(
        1,
        request("asset://app/theme.css"),
        Box::new(Gated {
            entered,
            gate: gate.clone(),
            reply,
            drops: drops.clone(),
        }),
    );
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    loader.close(1);
    wait(|| loader.stats().stopped && drops.load(Ordering::SeqCst) == 1);
    gate.notify_one();
    assert!(result.recv_timeout(Duration::from_millis(20)).is_err());
    assert_eq!(loader.stats().completed, 0);
}

#[cfg(feature = "raster")]
#[test]
fn package_css_and_image_reach_real_layout_and_cpu_pixels() {
    use glory_blitz::{Config, Raster, Renderer};
    use glory_ui::{App, image};
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[20, 180, 90, 255])
            .unwrap();
    }
    let loader = Arc::new(
        Loader::new(LoadConfig {
            package: BTreeMap::from([
                (
                    "/theme.css".into(),
                    Bytes::from_static(
                        b"body{margin:0}.badge{display:block;width:32px;height:32px}",
                    ),
                ),
                ("/badge.png".into(), png.into()),
            ]),
            ..Default::default()
        })
        .unwrap(),
    );
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::with_config(
            runtime,
            Config {
                base_url: Url::parse("asset://app/").unwrap(),
                provider: Some(loader.clone()),
                ..Default::default()
            },
        );
        host.set_viewport(64, 64, 1.).unwrap();
        renderer = Some(host.clone());
        host
    });
    let renderer = renderer.unwrap();
    renderer.set_css("@import url('theme.css');");
    app.mount(image().class("badge").source("badge.png").decorative())
        .unwrap();
    wait(|| {
        app.pump().unwrap();
        renderer.resource_stats().cached_images == 1 && loader.stats().completed == 2
    });
    let root = app.tree().roots()[0];
    let engine = renderer.engine_id(root, glory_html::Part::Main).unwrap();
    assert_eq!(
        renderer.inspect(|doc| doc.get_client_bounding_rect(engine).unwrap().width),
        32.
    );
    let mut raster = Raster::new();
    raster.draw(&renderer).unwrap();
    let offset = (16 * 64 + 16) * 4;
    assert_eq!(&raster.pixels()[offset..offset + 4], &[20, 180, 90, 255]);
    drop(app);
    drop(renderer);
    wait(|| loader.stats().closed && loader.stats().stopped);
}
