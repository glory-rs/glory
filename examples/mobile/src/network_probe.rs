//! Opt-in real socket/platform TLS checks. Never block the UIKit/Activity loop.
use glory::apace::assets::{
    Bytes, LoadConfig, Loader, NetHandler, NetProvider, NetReady, Request, Url,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

type Reply = Result<(String, Bytes), String>;
struct Handler {
    reply: mpsc::Sender<Reply>,
    prepared: Arc<AtomicUsize>,
}
impl NetHandler for Handler {
    fn prepare<'a>(&'a mut self, _: &'a str, _: &'a Bytes) -> NetReady<'a> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(Ok(())))
    }
    fn bytes(self: Box<Self>, url: String, bytes: Bytes) {
        let _ = self.reply.send(Ok((url, bytes)));
    }
    fn error(self: Box<Self>, _: String, error: String) {
        let _ = self.reply.send(Err(error));
    }
}
fn start(loader: &Loader, url: &str) -> (mpsc::Receiver<Reply>, Arc<AtomicUsize>) {
    let (reply, receive) = mpsc::channel();
    let prepared = Arc::new(AtomicUsize::new(0));
    loader.fetch(
        7,
        Request::get(Url::parse(url).unwrap()),
        Box::new(Handler {
            reply,
            prepared: prepared.clone(),
        }),
    );
    (receive, prepared)
}
fn fetch(loader: &Loader, url: &str) -> (Reply, usize) {
    let (receive, prepared) = start(loader, url);
    (
        receive
            .recv_timeout(Duration::from_secs(20))
            .expect("network fixture reply"),
        prepared.load(Ordering::SeqCst),
    )
}
fn wait(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < end, "network fixture observation timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn stop(loader: &Loader) {
    loader.shutdown();
    wait(|| loader.stats().stopped);
    assert_eq!((loader.stats().active, loader.stats().queued), (0, 0));
}
fn state(monitor: &Loader, origin: &str, token: &str, expected: &[u8]) {
    wait(|| {
        fetch(monitor, &format!("{origin}status?token={token}"))
            .0
            .unwrap()
            .1
            == expected
    });
}

pub fn spawn(mut config: LoadConfig, origin: String, https: String, rejected_tls: String) {
    config.limits.body_bytes = 64 * 1024;
    config.limits.timeout = Duration::from_secs(15);
    std::thread::Builder::new()
        .name("glory-network-probe".into())
        .spawn(move || {
            run(config, &origin, &https, &rejected_tls);
        })
        .expect("network probe thread");
}
fn run(config: LoadConfig, origin: &str, https: &str, rejected_tls: &str) {
    let loader = Loader::new(config.clone()).unwrap();
    let monitor = Loader::new(config.clone()).unwrap();
    for path in ["ok", "redirect"] {
        let (reply, prepared) = fetch(&loader, &format!("{origin}{path}"));
        let (url, bytes) = reply.unwrap();
        assert_eq!(bytes, b"glory network"[..]);
        assert_eq!(url, format!("{origin}ok"));
        assert_eq!(prepared, 1);
    }
    for (path, error) in [
        ("redirect-outside", "outside allowed"),
        ("redirect-loop", "redirect limit"),
        ("large", "body budget"),
        ("chunked", "body budget"),
        ("encoded", "encoded content"),
        ("error", "HTTP status"),
    ] {
        let (reply, prepared) = fetch(&loader, &format!("{origin}{path}"));
        assert!(reply.unwrap_err().contains(error), "{path}");
        assert_eq!(prepared, 0);
    }
    let (reply, prepared) = fetch(&loader, https);
    assert!(!reply.expect("platform-trusted HTTPS").1.is_empty());
    assert_eq!(prepared, 1);
    let (reply, prepared) = fetch(&loader, rejected_tls);
    assert!(
        reply.unwrap_err().contains("HTTP request/body"),
        "self-signed TLS must fail"
    );
    assert_eq!(prepared, 0);
    eprintln!(
        "Glory network probe: HTTP/redirect/origin/body and trusted/untrusted platform TLS passed"
    );

    // Wait for the fixture server to observe each actual request. Cancelling
    // only a queued job would not prove that the TCP connection was closed.
    let token = format!("{}-cancel", std::process::id());
    let url = format!("{origin}slow?token={token}");
    let (reply, prepared) = start(&loader, &url);
    state(&monitor, origin, &token, b"entered");
    loader.cancel(7, &url);
    wait(|| loader.stats().active == 0);
    state(&monitor, origin, &token, b"disconnected");
    assert!(reply.recv_timeout(Duration::from_secs(1)).is_err());
    assert_eq!(prepared.load(Ordering::SeqCst), 0);
    stop(&loader);

    let token = format!("{}-close", std::process::id());
    let closing = Loader::new(config.clone()).unwrap();
    let (reply, prepared) = start(&closing, &format!("{origin}slow?token={token}"));
    state(&monitor, origin, &token, b"entered");
    stop(&closing);
    state(&monitor, origin, &token, b"disconnected");
    assert!(reply.recv_timeout(Duration::from_secs(1)).is_err());
    assert_eq!(prepared.load(Ordering::SeqCst), 0);

    let token = format!("{}-deadline", std::process::id());
    let mut short = config;
    // Keep enough time for a software-rendered emulator to establish TCP. The
    // server must observe the actual connection before this deadline expires.
    short.limits.timeout = Duration::from_secs(3);
    let expiring = Loader::new(short).unwrap();
    let (reply, prepared) = fetch(&expiring, &format!("{origin}slow?token={token}"));
    assert!(reply.unwrap_err().contains("deadline"));
    assert_eq!(prepared, 0);
    state(&monitor, origin, &token, b"disconnected");
    stop(&expiring);
    stop(&monitor);
    eprintln!(
        "Glory network probe: cancel/close/deadline closed actual sockets; no late prepare/publication; passed"
    );
}
