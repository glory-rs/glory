//! Opt-in actual AssetManager acceptance, run on the Activity's Rust thread.
use glory::apace::assets::{
    AndroidAssets, Bytes, LoadConfig, LoadLimits, Loader, NetHandler, NetProvider, NetReady,
    Request, Url,
};
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

type Reply = Result<Bytes, String>;
struct Handler {
    reply: mpsc::Sender<Reply>,
    prepared: Arc<AtomicUsize>,
}
impl NetHandler for Handler {
    fn prepare<'a>(&'a mut self, _: &'a str, _: &'a Bytes) -> NetReady<'a> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(Ok(())))
    }
    fn bytes(self: Box<Self>, _: String, bytes: Bytes) {
        let _ = self.reply.send(Ok(bytes));
    }
    fn error(self: Box<Self>, _: String, error: String) {
        let _ = self.reply.send(Err(error));
    }
}
fn config(assets: AndroidAssets) -> LoadConfig {
    LoadConfig {
        android: Some(assets),
        ..Default::default()
    }
}
fn wait(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < end, "APK asset acceptance timeout");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn fetch(loader: &Loader, url: &str) -> (Reply, usize) {
    let (reply, result) = mpsc::channel();
    let prepared = Arc::new(AtomicUsize::new(0));
    loader.fetch(
        7,
        Request::get(Url::parse(url).unwrap()),
        Box::new(Handler {
            reply,
            prepared: prepared.clone(),
        }),
    );
    let result = result.recv_timeout(Duration::from_secs(5)).unwrap();
    (result, prepared.load(Ordering::SeqCst))
}
fn stop(loader: &Loader) {
    loader.shutdown();
    wait(|| loader.stats().stopped && loader.stats().file_active == 0);
    assert_eq!((loader.stats().active, loader.stats().queued), (0, 0));
}

struct Pending {
    entered: mpsc::Sender<()>,
    result: mpsc::Sender<Reply>,
    dropped: Arc<AtomicUsize>,
}
impl NetHandler for Pending {
    fn prepare<'a>(&'a mut self, _: &'a str, _: &'a Bytes) -> NetReady<'a> {
        Box::pin(async move {
            self.entered.send(()).unwrap();
            std::future::pending().await
        })
    }
    fn bytes(self: Box<Self>, _: String, bytes: Bytes) {
        let _ = self.result.send(Ok(bytes));
    }
    fn error(self: Box<Self>, _: String, error: String) {
        let _ = self.result.send(Err(error));
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

pub fn run(assets: AndroidAssets) -> Result<(), Box<dyn Error>> {
    let loader = Loader::new(config(assets.clone()))?;
    for (url, expected) in [
        (
            "asset://android/%74heme.css?v=1#part",
            include_bytes!("../assets/theme.css").as_slice(),
        ),
        (
            "asset://android/badge.png",
            include_bytes!("../assets/badge.png").as_slice(),
        ),
        (
            "asset://android/%E4%B8%AD%E6%96%87/%E7%BB%84%E5%90%88.txt",
            "中文😀 asset probe".as_bytes(),
        ),
    ] {
        let (reply, prepared) = fetch(&loader, url);
        assert_eq!(reply.unwrap(), expected);
        assert_eq!(prepared, 1, "native assets must await decode admission");
    }
    for url in [
        "asset://android/missing",
        "asset://android/%2foutside",
        "asset://android/%00",
        "asset://app/theme.css",
        "asset://other/theme.css",
        "file:///theme.css",
        "https://example.com/theme.css",
    ] {
        let (reply, prepared) = fetch(&loader, url);
        assert!(reply.is_err(), "{url}");
        assert_eq!(prepared, 0);
    }
    assert!(loader.stats().file_peak_active > 0);
    stop(&loader);

    let small = Loader::new(LoadConfig {
        limits: LoadLimits {
            body_bytes: 1,
            ..Default::default()
        },
        ..config(assets.clone())
    })?;
    let (reply, prepared) = fetch(&small, "asset://android/theme.css");
    assert!(reply.unwrap_err().contains("budget"));
    assert_eq!(prepared, 0);
    stop(&small);

    // Read the actual APK body, then hold decode admission. Close must drop the
    // pending future and handler without publishing a late completion.
    let pending = Loader::new(config(assets))?;
    let (entered, started) = mpsc::channel();
    let (result, reply) = mpsc::channel();
    let dropped = Arc::new(AtomicUsize::new(0));
    pending.fetch(
        7,
        Request::get(Url::parse("asset://android/theme.css").unwrap()),
        Box::new(Pending {
            entered,
            result,
            dropped: dropped.clone(),
        }),
    );
    started.recv_timeout(Duration::from_secs(5))?;
    stop(&pending);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(reply.try_recv().is_err());
    eprintln!(
        "Glory APK assets probe: CSS/PNG/Unicode, misses/policy, body limit, prepare and close passed"
    );
    Ok(())
}
