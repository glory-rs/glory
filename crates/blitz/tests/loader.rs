#![cfg(all(
    feature = "assets",
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use glory_blitz::assets::{
    Bytes, LoadConfig, LoadLimits, Loader, NetHandler, NetProvider, Request, Url,
};
use glory_blitz::{Config, Renderer};
use glory_html::Part;
use glory_ui::{App, image, text, view};

fn wait(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !predicate() {
        assert!(Instant::now() < deadline, "fixture deadline");
        thread::sleep(Duration::from_millis(2));
    }
}
struct Server {
    url: Url,
    stop: Arc<AtomicBool>,
    hits: Arc<Mutex<Vec<String>>>,
    thread: Option<JoinHandle<()>>,
}
impl Server {
    fn new(handler: impl Fn(&str, &mut TcpStream, &AtomicBool) + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let hits = Arc::new(Mutex::new(Vec::new()));
        let (ending, paths, handler) = (stop.clone(), hits.clone(), Arc::new(handler));
        let thread = thread::spawn(move || {
            let mut clients = Vec::new();
            while !ending.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let (handler, paths, ending) =
                            (handler.clone(), paths.clone(), ending.clone());
                        clients.push(thread::spawn(move || {
                            stream
                                .set_read_timeout(Some(Duration::from_millis(20)))
                                .unwrap();
                            stream
                                .set_write_timeout(Some(Duration::from_secs(1)))
                                .unwrap();
                            let mut header = Vec::new();
                            while !header.ends_with(b"\r\n\r\n")
                                && header.len() < 8192
                                && !ending.load(Ordering::Acquire)
                            {
                                let mut byte = [0];
                                match stream.read(&mut byte) {
                                    Ok(0) => return,
                                    Ok(_) => header.push(byte[0]),
                                    Err(error)
                                        if matches!(
                                            error.kind(),
                                            std::io::ErrorKind::WouldBlock
                                                | std::io::ErrorKind::TimedOut
                                        ) => {}
                                    Err(_) => return,
                                }
                            }
                            if ending.load(Ordering::Acquire) || !header.ends_with(b"\r\n\r\n") {
                                return;
                            }
                            let header = String::from_utf8(header).unwrap();
                            let path = header
                                .lines()
                                .next()
                                .unwrap()
                                .split_whitespace()
                                .nth(1)
                                .unwrap();
                            paths.lock().unwrap().push(path.to_owned());
                            handler(path, &mut stream, &ending);
                        }));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(error) => panic!("listener: {error}"),
                }
            }
            for client in clients {
                client.join().unwrap();
            }
        });
        Self {
            url,
            stop,
            hits,
            thread: Some(thread),
        }
    }
    fn config(&self, limits: LoadLimits) -> LoadConfig {
        LoadConfig {
            origins: vec![self.url.clone()],
            limits,
            ..Default::default()
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn reply(stream: &mut TcpStream, status: &str, headers: &str, bytes: &[u8]) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n",
        bytes.len()
    );
    let _ = stream.write_all(bytes);
    let _ = stream.flush();
}
fn hold(stream: &mut TcpStream, stop: &AtomicBool, disconnected: &AtomicUsize) {
    stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n")
        .unwrap();
    stream.flush().unwrap();
    while !stop.load(Ordering::Acquire) {
        let mut byte = [0];
        match stream.read(&mut byte) {
            Ok(0) => {
                disconnected.fetch_add(1, Ordering::Relaxed);
                return;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => {
                disconnected.fetch_add(1, Ordering::Relaxed);
                return;
            }
            Ok(_) => {}
        }
    }
}
type Response = (String, Result<Bytes, String>);
struct Handler(mpsc::Sender<Response>);
impl NetHandler for Handler {
    fn bytes(self: Box<Self>, url: String, bytes: Bytes) {
        let _ = self.0.send((url, Ok(bytes)));
    }
    fn error(self: Box<Self>, url: String, error: String) {
        let _ = self.0.send((url, Err(error)));
    }
}
fn fetch(loader: &Loader, url: Url) -> mpsc::Receiver<Response> {
    let (tx, rx) = mpsc::channel();
    loader.fetch(1, Request::get(url), Box::new(Handler(tx)));
    rx
}
fn response(rx: mpsc::Receiver<Response>) -> Response {
    rx.recv_timeout(Duration::from_secs(5))
        .expect("resource completion")
}
fn done(loader: &Loader) {
    loader.shutdown();
    wait(|| loader.stats().stopped);
    assert_eq!((loader.stats().queued, loader.stats().active), (0, 0));
}

#[test]
fn embedded_package_is_explicit_bounded_and_document_owned() {
    let config = LoadConfig {
        package: BTreeMap::from([("/theme.css".into(), Bytes::from_static(b"body{color:red}"))]),
        ..Default::default()
    };
    let loader = Loader::new(config.clone()).unwrap();
    let (url, bytes) = response(fetch(
        &loader,
        Url::parse("asset://app/theme.css?v=1#x").unwrap(),
    ));
    assert!(url.ends_with("theme.css?v=1#x"));
    assert_eq!(bytes.unwrap(), b"body{color:red}"[..]);
    for url in [
        "file:///outside.png",
        "data:text/css,body{}",
        "https://example.com/image.png",
        "asset://other/theme.css",
        "asset://user@app/theme.css",
        "asset://app/missing.css",
    ] {
        assert!(
            response(fetch(&loader, Url::parse(url).unwrap()))
                .1
                .is_err()
        );
    }
    let (tx, rx) = mpsc::channel();
    loader.fetch(
        2,
        Request::get(Url::parse("asset://app/theme.css").unwrap()),
        Box::new(Handler(tx)),
    );
    assert!(response(rx).1.unwrap_err().contains("another document"));
    done(&loader);
    assert!(
        fetch(&loader, Url::parse("asset://app/theme.css").unwrap())
            .recv_timeout(Duration::from_millis(20))
            .is_err()
    );
    let mut invalid = config;
    invalid.package.insert("/a/../escape".into(), Bytes::new());
    assert!(Loader::new(invalid).is_err());
    assert!(
        Loader::new(LoadConfig {
            limits: LoadLimits {
                workers: 0,
                ..Default::default()
            },
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        Loader::new(LoadConfig {
            origins: vec![Url::parse("https://example.com/path").unwrap()],
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        Loader::new(LoadConfig {
            origins: vec![Url::parse(&format!("https://{}.example/", "a".repeat(4096))).unwrap()],
            ..Default::default()
        })
        .is_err()
    );
}

#[test]
fn every_redirect_and_body_is_checked_before_publication() {
    let forbidden = Server::new(|_, stream, _| reply(stream, "200 OK", "", b"forbidden"));
    let forbidden_url = forbidden.url.to_string();
    let mut server = Server::new(move |path, stream, _| match path {
        "/start" => reply(stream, "302 Found", "Location: /dir/end\r\n", b""),
        "/dir/end" => reply(stream, "200 OK", "", b"allowed"),
        "/loop" => reply(stream, "302 Found", "Location: /loop\r\n", b""),
        "/other" => reply(
            stream,
            "302 Found",
            &format!("Location: {forbidden_url}\r\n"),
            b"",
        ),
        "/package" => reply(
            stream,
            "302 Found",
            "Location: asset://app/theme.css\r\n",
            b"",
        ),
        "/length" => reply(stream, "200 OK", "", &[b'x'; 257]),
        "/chunked" => {
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n101\r\n");
            let _ = stream.write_all(&[b'x'; 257]);
            let _ = stream.write_all(b"\r\n0\r\n\r\n");
        }
        "/gzip" => reply(stream, "200 OK", "Content-Encoding: gzip\r\n", b"encoded"),
        _ => reply(stream, "404 Not Found", "", b""),
    });
    // Exercise the bounded host resolver too, rather than only literal IPs.
    server.url.set_host(Some("localhost")).unwrap();
    let loader = Loader::new(server.config(LoadLimits {
        body_bytes: 128,
        redirects: 2,
        ..Default::default()
    }))
    .unwrap();
    let (url, body) = response(fetch(&loader, server.url.join("start").unwrap()));
    assert_eq!(url, server.url.join("dir/end").unwrap().as_str());
    assert_eq!(body.unwrap(), b"allowed"[..]);
    for path in [
        "loop", "other", "package", "length", "chunked", "gzip", "missing",
    ] {
        assert!(
            response(fetch(&loader, server.url.join(path).unwrap()))
                .1
                .is_err(),
            "{path}"
        );
    }
    assert!(forbidden.hits.lock().unwrap().is_empty());
    let (tx, rx) = mpsc::channel();
    let mut request = Request::get(server.url.clone());
    request.method = glory_blitz::assets::Method::POST;
    loader.fetch(1, request, Box::new(Handler(tx)));
    assert!(response(rx).1.is_err());
    done(&loader);
}

#[test]
fn bounded_workers_queue_and_cancel_close_the_actual_http_connection() {
    let disconnected = Arc::new(AtomicUsize::new(0));
    let seen = disconnected.clone();
    let server = Server::new(move |path, stream, stop| {
        if path.starts_with("/hold") {
            hold(stream, stop, &seen);
        } else {
            reply(stream, "200 OK", "", b"next");
        }
    });
    let loader = Loader::new(server.config(LoadLimits {
        workers: 1,
        queued: 2,
        ..Default::default()
    }))
    .unwrap();
    let url = server.url.join("hold").unwrap();
    let first = fetch(&loader, url.clone());
    wait(|| server.hits.lock().unwrap().len() == 1);
    let second = fetch(&loader, url.clone());
    let third = fetch(&loader, url.clone());
    assert!(
        response(fetch(&loader, url.clone()))
            .1
            .unwrap_err()
            .contains("queue")
    );
    assert_eq!((loader.stats().active, loader.stats().queued), (1, 2));
    loader.cancel(2, url.as_str());
    assert_eq!(loader.stats().queued, 2);
    loader.cancel(1, url.as_str());
    wait(|| loader.stats().active == 0 && disconnected.load(Ordering::Relaxed) == 1);
    assert_eq!(loader.stats().cancelled, 3);
    for rx in [first, second, third] {
        assert!(rx.recv_timeout(Duration::from_millis(20)).is_err());
    }
    for index in 0..100 {
        let pending = fetch(&loader, server.url.join(&format!("hold?{index}")).unwrap());
        wait(|| server.hits.lock().unwrap().len() == index + 2);
        loader.cancel(
            1,
            server.url.join(&format!("hold?{index}")).unwrap().as_str(),
        );
        wait(|| loader.stats().active == 0 && disconnected.load(Ordering::Relaxed) == index + 2);
        assert!(pending.recv_timeout(Duration::from_millis(20)).is_err());
    }
    assert_eq!(
        response(fetch(&loader, server.url.join("good").unwrap()))
            .1
            .unwrap(),
        b"next"[..]
    );
    wait(|| loader.stats().active == 0);
    assert_eq!(loader.stats().peak_active, 1);
    assert!(loader.stats().peak_queued <= 2);
    let late = fetch(&loader, server.url.join("hold?close").unwrap());
    wait(|| {
        server
            .hits
            .lock()
            .unwrap()
            .iter()
            .any(|path| path == "/hold?close")
    });
    loader.close(1);
    wait(|| loader.stats().stopped && disconnected.load(Ordering::Relaxed) == 102);
    assert!(late.recv_timeout(Duration::from_millis(20)).is_err());
    assert_eq!((loader.stats().queued, loader.stats().active), (0, 0));
}

#[test]
fn queue_and_active_deadlines_and_external_abort_release_slots() {
    let disconnected = Arc::new(AtomicUsize::new(0));
    let seen = disconnected.clone();
    let server = Server::new(move |_, stream, stop| hold(stream, stop, &seen));
    let loader = Loader::new(server.config(LoadLimits {
        workers: 1,
        queued: 4,
        timeout: Duration::from_millis(150),
        ..Default::default()
    }))
    .unwrap();
    let first = fetch(&loader, server.url.join("first").unwrap());
    wait(|| loader.stats().active == 1);
    let queued = fetch(&loader, server.url.join("queued").unwrap());
    for rx in [first, queued] {
        let error = response(rx).1.unwrap_err();
        assert!(error.contains("deadline"), "{error}");
    }
    wait(|| loader.stats().active == 0 && loader.stats().queued == 0);
    let abort = blitz_traits::net::AbortController::default();
    let (tx, rx) = mpsc::channel();
    loader.fetch(
        1,
        Request::get(server.url.join("abort").unwrap()).signal(abort.signal.clone()),
        Box::new(Handler(tx)),
    );
    wait(|| {
        server
            .hits
            .lock()
            .unwrap()
            .iter()
            .any(|path| path == "/abort")
    });
    abort.abort();
    assert!(response(rx).1.unwrap_err().contains("cancelled"));
    wait(|| loader.stats().active == 0);
    assert!(loader.stats().cancelled >= 1);
    done(&loader);
}

fn png() -> Vec<u8> {
    png_height(8)
}
fn png_height(height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 8, height);
        encoder.set_color(png::ColorType::Rgba);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[11, 177, 33, 255].repeat(8 * height as usize))
            .unwrap();
    }
    bytes
}

#[cfg(feature = "shell")]
#[test]
fn delayed_http_intrinsic_images_preserve_anchor_and_end_navigation() {
    use glory_blitz::shell::WindowApp;
    use glory_render::{Action, Event, Navigation, Query, QueryValue, Rect};
    use glory_ui::list;
    let gates = Arc::new(Mutex::new(BTreeMap::<usize, u32>::new()));
    let released = gates.clone();
    let automatic = Arc::new(AtomicBool::new(false));
    let ready = automatic.clone();
    let server = Server::new(move |path, stream, stop| {
        let key = path
            .trim_start_matches('/')
            .trim_end_matches(".png")
            .parse::<usize>()
            .unwrap();
        let height = loop {
            if stop.load(Ordering::Acquire) {
                return;
            }
            if let Some(height) = released.lock().unwrap().get(&key).copied() {
                break height;
            }
            if ready.load(Ordering::Acquire) && key != 99_999_999 {
                break 24;
            }
            match stream.peek(&mut [0; 1]) {
                Ok(0) => return,
                Err(error)
                    if !matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    return;
                }
                _ => {}
            }
        };
        reply(stream, "200 OK", "", &png_height(height));
    });
    let loader = Arc::new(
        Loader::new(server.config(LoadLimits {
            workers: 4,
            queued: 32,
            ..Default::default()
        }))
        .unwrap(),
    );
    let window = WindowApp::with_config(Config {
        base_url: server.url.clone(),
        provider: Some(loader.clone()),
        ..Default::default()
    });
    let app = window.app();
    let renderer = window.renderer();
    renderer.set_css(".photo{display:block;min-height:24px}");
    let position = app.scope().cage(24_000_007.);
    let reads = std::rc::Rc::new(std::cell::Cell::new(0));
    let source = reads.clone();
    app.mount(
        list(
            100_000_000_usize,
            move |i| {
                source.set(source.get() + 1);
                i
            },
            |_, key| {
                glory_ui::image()
                    .source(format!("{key}.png"))
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
        .on_scroll(move |p| position.revise(|value| *value = p)),
    )
    .unwrap();
    for _ in 0..8 {
        app.pump().unwrap();
    }
    let root = app.tree().roots()[0];
    let row = |key| {
        let tree = app.tree();
        let body = tree.node(root).unwrap().children[0];
        tree.node(body)
            .unwrap()
            .children
            .iter()
            .copied()
            .find(|id| {
                tree.node(*id)
                    .unwrap()
                    .descriptor
                    .view()
                    .item
                    .is_some_and(|item| item.index == key)
            })
            .unwrap()
    };
    let bounds = |id| -> Rect {
        let value = std::rc::Rc::new(std::cell::RefCell::new(None));
        let output = value.clone();
        app.request(Query::Bounds(id), move |response| {
            *output.borrow_mut() = Some(response)
        })
        .unwrap();
        app.pump().unwrap();
        let QueryValue::Bounds(rect) = value.borrow_mut().take().unwrap().unwrap() else {
            panic!("bounds")
        };
        rect
    };
    let anchor = row(1_000_000);
    let before = bounds(anchor);
    let logical = position.get();
    gates.lock().unwrap().insert(999_999, 96);
    wait(|| {
        app.pump().unwrap();
        (bounds(row(999_999)).height - 96.).abs() < 0.03
    });
    assert_eq!(row(1_000_000), anchor);
    assert!((bounds(anchor).y - before.y).abs() < 0.03);
    assert!((position.get() - logical - 72.).abs() < 0.03);
    let anchored = bounds(anchor);
    automatic.store(true, Ordering::Release);
    app.scope().action(Action::Focus(root)).unwrap();
    app.flush().unwrap();
    app.dispatch(
        root,
        Event::Navigate {
            from: root,
            command: Navigation::Last,
        },
    )
    .unwrap();
    wait(|| {
        app.pump().unwrap();
        server
            .hits
            .lock()
            .unwrap()
            .iter()
            .any(|path| path == "/99999999.png")
    });
    for _ in 0..6 {
        app.pump().unwrap();
    }
    let last = row(99_999_999);
    let placeholder = bounds(last);
    assert!((placeholder.height - 24.).abs() < 0.03);
    gates.lock().unwrap().insert(99_999_999, 96);
    wait(|| {
        app.pump().unwrap();
        let rect = bounds(last);
        rect.height > 95.9 && rect.y + rect.height <= 180.03
    });
    let revealed = bounds(last);
    assert!(revealed.y >= -0.03);
    assert!(reads.get() < 500);
    eprintln!(
        "HTTP intrinsic anchor: {before:?} -> {anchored:?}; End {placeholder:?} -> {revealed:?}; key_reads={}",
        reads.get()
    );
    app.clear().unwrap();
    wait(|| {
        renderer.poll_resources();
        loader.stats().active == 0
            && loader.stats().queued == 0
            && renderer.resource_stats().requests == 0
    });
    drop(window);
    wait(|| loader.stats().stopped);
}
#[test]
fn redirected_css_import_background_font_and_image_keep_correct_bases_and_ownership() {
    let png = png();
    let server = Server::new(move |path, stream, _| {
        match path {
        "/theme.css" => reply(stream, "302 Found", "Location: /theme/main.css\r\n", b""),
        "/theme/main.css" => reply(stream, "200 OK", "", b"@import url('nested.css'); .box{background-image:url('bg.png')}"),
        "/theme/nested.css" => reply(stream, "302 Found", "Location: /fonts/end.css\r\n", b""),
        "/fonts/end.css" => reply(
            stream,
            "200 OK",
            "",
            b"@font-face{font-family:Loaded;src:url('font.otf')} .box{width:73px;height:20px;font-family:Loaded}",
        ),
        "/fonts/font.otf" => reply(stream, "200 OK", "", blitz_dom::BULLET_FONT),
        "/picture.png" => reply(stream, "302 Found", "Location: /images/end.png\r\n", b""),
        "/broken.png" => reply(stream, "302 Found", "Location: /images/broken.png\r\n", b""),
        "/images/broken.png" => reply(stream, "200 OK", "", b"not an image"),
        "/images/end.png" | "/theme/bg.png" => reply(stream, "200 OK", "", &png),
        _ => reply(stream, "404 Not Found", "", b""),
    }
    });
    let loader = Arc::new(Loader::new(server.config(Default::default())).unwrap());
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::with_config(
            runtime,
            Config {
                base_url: server.url.clone(),
                provider: Some(loader.clone()),
                ..Default::default()
            },
        );
        renderer = Some(host.clone());
        host
    });
    let renderer = renderer.unwrap();
    renderer.set_css("@import url('theme.css');");
    app.mount(view().class("box").fill((
        text().content("loaded"),
        image().source("picture.png").alt("test"),
    )))
    .unwrap();
    wait(|| {
        app.pump().unwrap();
        let stats = renderer.resource_stats();
        stats.pending_images == 0 && stats.cached_images == 2 && stats.registered_fonts == 1
    });
    assert_eq!(renderer.resource_stats().failures, 0);
    let id = app.tree().roots()[0];
    let physical = renderer.engine_id(id, Part::Main).unwrap();
    assert_eq!(
        renderer.inspect(|doc| doc.get_client_bounding_rect(physical).unwrap().width),
        73.
    );
    let hits = server.hits.lock().unwrap().clone();
    for path in [
        "/theme/nested.css",
        "/theme/bg.png",
        "/fonts/font.otf",
        "/images/end.png",
    ] {
        assert!(hits.iter().any(|hit| hit == path), "{hits:?}");
    }
    app.clear().unwrap();
    let image_hits = hits.iter().filter(|path| path.ends_with(".png")).count();
    app.mount(image().source("picture.png").alt("cache"))
        .unwrap();
    app.pump().unwrap();
    assert_eq!(renderer.resource_stats().pending_images, 0);
    assert_eq!(
        server
            .hits
            .lock()
            .unwrap()
            .iter()
            .filter(|path| path.ends_with(".png"))
            .count(),
        image_hits
    );
    app.clear().unwrap();
    app.mount(image().source("broken.png").alt("failed redirect"))
        .unwrap();
    wait(|| {
        app.pump().unwrap();
        renderer.resource_stats().failures == 1
    });
    assert_eq!(renderer.resource_stats().pending_images, 0);
    drop(app);
    drop(renderer);
    wait(|| loader.stats().stopped);
}

#[test]
fn huge_fixed_and_measured_lists_bound_real_http_jobs_and_image_history() {
    use glory_blitz::assets::ResourceLimits;
    use glory_render::style::{Length, Style};
    use glory_ui::list;
    for measured in [false, true] {
        let encoded = png();
        let server = Server::new(move |_, stream, _| reply(stream, "200 OK", "", &encoded));
        let loader = Arc::new(
            Loader::new(server.config(LoadLimits {
                workers: 2,
                queued: 32,
                ..Default::default()
            }))
            .unwrap(),
        );
        let mut renderer = None;
        let app = App::new(|runtime| {
            let host = Renderer::with_config(
                runtime,
                Config {
                    base_url: server.url.clone(),
                    provider: Some(loader.clone()),
                    resources: ResourceLimits {
                        image_cache_entries: 8,
                        image_cache_bytes: 1024,
                        pending_images: 32,
                        ..Default::default()
                    },
                },
            );
            renderer = Some(host.clone());
            host
        });
        let renderer = renderer.unwrap();
        let position = app.scope().cage(0.);
        let reads = std::rc::Rc::new(std::cell::Cell::new(0));
        let source = reads.clone();
        let widget = list(
            100_000_000_usize,
            move |index| {
                source.set(source.get() + 1);
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
        for step in 0..40 {
            position.revise(|value| *value = (step * 19_123_000) as f64);
            app.pump().unwrap();
            if step % 2 == 0 {
                wait(|| {
                    app.pump().unwrap();
                    let stats = loader.stats();
                    renderer.resource_stats().pending_images == 0
                        && stats.active == 0
                        && stats.queued == 0
                });
            }
            let stats = renderer.resource_stats();
            assert!(
                stats.cached_images <= 8 && stats.cached_image_bytes <= 1024,
                "{stats:?}"
            );
            assert!(
                stats.pending_images <= 16 && stats.image_waiters <= 16,
                "{stats:?}"
            );
            assert!(
                stats.requests <= 32 && stats.completion_bytes <= 32 * 256,
                "{stats:?}"
            );
            assert!(loader.stats().active <= 2 && loader.stats().queued <= 32);
            assert!(app.stats().nodes < 64 && app.runtime_stats().node_slots < 160);
        }
        assert!(renderer.resource_stats().image_evictions > 20);
        assert!(reads.get() < 4000);
        app.clear().unwrap();
        wait(|| {
            renderer.poll_resources();
            loader.stats().active == 0
                && loader.stats().queued == 0
                && renderer.resource_stats().requests == 0
        });
        assert_eq!(renderer.resource_stats().pending_images, 0);
        assert!(loader.stats().peak_active <= 2 && loader.stats().peak_queued <= 32);
        eprintln!(
            "HTTP list measured={measured}: reads={}, resources={:?}, loader={:?}",
            reads.get(),
            renderer.resource_stats(),
            loader.stats()
        );
        drop(app);
        drop(renderer);
        wait(|| loader.stats().stopped);
    }
}

#[test]
fn https_rejects_a_well_formed_untrusted_certificate() {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![certified.cert.der().clone()],
        rustls::pki_types::PrivatePkcs8KeyDer::from(certified.signing_key.serialize_der()).into(),
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = Url::parse(&format!(
        "https://localhost:{}/",
        listener.local_addr().unwrap().port()
    ))
    .unwrap();
    let server = thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut stream = rustls::StreamOwned::new(
            rustls::ServerConnection::new(Arc::new(tls)).unwrap(),
            socket,
        );
        assert!(
            stream.read(&mut [0; 1]).is_err(),
            "untrusted certificate must fail the handshake"
        );
    });
    let loader = Loader::new(LoadConfig {
        origins: vec![url.clone()],
        ..Default::default()
    })
    .unwrap();
    assert!(response(fetch(&loader, url)).1.is_err());
    server.join().unwrap();
    done(&loader);
}

fn pressure_app(
    loader: Arc<Loader>,
    base_url: Url,
    limits: glory_blitz::assets::ResourceLimits,
) -> (App, Renderer) {
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::with_config(
            runtime,
            Config {
                base_url,
                provider: Some(loader),
                resources: limits,
            },
        );
        renderer = Some(host.clone());
        host
    });
    (app, renderer.unwrap())
}

#[test]
fn paused_host_bounds_decoded_results_and_cancel_resume_close_releases_real_leases() {
    let encoded = png();
    let server = Server::new(move |_, stream, _| reply(stream, "200 OK", "", &encoded));
    let loader = Arc::new(
        Loader::new(server.config(LoadLimits {
            workers: 2,
            queued: 16,
            ..Default::default()
        }))
        .unwrap(),
    );
    let (app, renderer) = pressure_app(
        loader.clone(),
        server.url.clone(),
        glory_blitz::assets::ResourceLimits {
            requests: 6,
            completion_bytes: 256,
            ..Default::default()
        },
    );
    app.mount(
        view().fill(
            (0..8)
                .map(|i| image().source(format!("initial/{i}.png")).alt("row"))
                .collect::<Vec<_>>(),
        ),
    )
    .unwrap();
    // Do not pump/poll resources: workers must stop before decoding another
    // 8x8 RGBA image, even though the HTTP body has already arrived.
    wait(|| {
        let s = renderer.resource_stats();
        s.completions == 1 && s.decode_waiters == 2
    });
    let s = renderer.resource_stats();
    assert_eq!((s.requests, s.completions, s.completion_bytes), (6, 1, 256));
    assert_eq!(loader.stats().completed, 1);
    assert!(s.rejected >= 2);
    app.clear().unwrap();
    // The local RunningGuard publishes idle before the job argument's handler
    // drops its decode lease. Wait for that distinct lifetime too, without
    // pumping the host or refunding the still-queued completion early.
    wait(|| {
        loader.stats().active == 0
            && loader.stats().queued == 0
            && renderer.resource_stats().requests == 1
    });
    let stale = renderer.resource_stats();
    assert_eq!(
        (stale.requests, stale.completions, stale.completion_bytes),
        (1, 1, 256),
        "cancelled queued payload still owns its charge"
    );
    renderer.poll_resources();
    assert_eq!(renderer.resource_stats().requests, 0);
    assert_eq!(renderer.resource_stats().completion_bytes, 0);
    assert_eq!(
        renderer.resource_stats().cached_images,
        0,
        "cancelled queued image cannot enter cache"
    );
    app.mount(
        view().fill(
            (0..4)
                .map(|i| image().source(format!("resume/{i}.png")).alt("row"))
                .collect::<Vec<_>>(),
        ),
    )
    .unwrap();
    wait(|| {
        renderer.poll_resources();
        let s = renderer.resource_stats();
        s.requests == 0 && s.pending_images == 0
    });
    assert_eq!(renderer.resource_stats().cached_images, 4);
    assert_eq!(renderer.resource_stats().completion_bytes, 0);
    app.clear().unwrap();
    app.mount(
        view().fill(
            (0..4)
                .map(|i| image().source(format!("close/{i}.png")).alt("row"))
                .collect::<Vec<_>>(),
        ),
    )
    .unwrap();
    wait(|| {
        let s = renderer.resource_stats();
        s.completions == 1 && s.decode_waiters >= 1
    });
    eprintln!(
        "paused resource pipeline: {:?}, transport={:?}",
        renderer.resource_stats(),
        loader.stats()
    );
    drop(app);
    drop(renderer);
    wait(|| loader.stats().stopped);
    assert_eq!((loader.stats().active, loader.stats().queued), (0, 0));
}

#[test]
fn synchronous_failure_and_import_storm_share_bounded_request_slots_without_error_channel_growth() {
    let loader = Arc::new(Loader::new(LoadConfig::default()).unwrap());
    let (app, renderer) = pressure_app(
        loader.clone(),
        Url::parse("https://denied.example/").unwrap(),
        glory_blitz::assets::ResourceLimits {
            requests: 4,
            completion_bytes: 256,
            ..Default::default()
        },
    );
    let css = (0..2000)
        .map(|i| format!("@import url('missing/{i}.css');"))
        .collect::<String>();
    renderer.set_css(&css);
    for index in 0..400 {
        app.mount(image().source(format!("row/{index}.png")).alt("row"))
            .unwrap();
        app.clear().unwrap();
        let stats = renderer.resource_stats();
        assert_eq!(
            (stats.requests, stats.completions, stats.completion_bytes),
            (4, 4, 0)
        );
    }
    assert_eq!(
        loader.stats().rejected,
        4,
        "no unregistered rejection may enqueue another error"
    );
    assert!(renderer.resource_stats().rejected >= 2396);
    renderer.poll_resources();
    assert_eq!(renderer.resource_stats().requests, 0);
    app.mount(image().source("after.png").alt("failure recovery"))
        .unwrap();
    renderer.poll_resources();
    assert_eq!(renderer.resource_stats().pending_images, 0);
    assert_eq!(renderer.resource_stats().completion_bytes, 0);
    assert_eq!(renderer.resource_stats().requests, 0);
    eprintln!(
        "bounded error/import storm: {:?}",
        renderer.resource_stats()
    );
    drop(app);
    drop(renderer);
    wait(|| loader.stats().stopped);
}
