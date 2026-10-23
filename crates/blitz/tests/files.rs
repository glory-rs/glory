#![cfg(all(
    feature = "assets",
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

use glory_blitz::assets::{
    Bytes, LoadConfig, LoadLimits, Loader, NetHandler, NetProvider, Request, ResourceLimits, Url,
};
use glory_blitz::{Config, Renderer};
use glory_ui::{App, image};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let base = std::env::temp_dir().canonicalize().unwrap();
        let normalized = base.to_string_lossy().replace('\\', "/");
        let normalized = normalized.strip_prefix("//?/").unwrap_or(&normalized);
        assert!(
            normalized.starts_with("D:/BuildCache/")
                || normalized.starts_with("/mnt/d/BuildCache/"),
            "fixture files must stay on D: {base:?}"
        );
        let path = base.join(format!(
            "glory-file-assets-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn config(&self) -> LoadConfig {
        LoadConfig {
            mounts: BTreeMap::from([("media".into(), self.0.clone())]),
            ..Default::default()
        }
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let target = self.0.canonicalize().unwrap();
        let base = std::env::temp_dir().canonicalize().unwrap();
        assert!(
            target.starts_with(&base)
                && target != base
                && target
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("glory-file-assets-")
        );
        fs::remove_dir_all(target).unwrap();
    }
}
fn wait(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !predicate() {
        assert!(Instant::now() < deadline, "file fixture deadline");
        std::thread::sleep(Duration::from_millis(2));
    }
}
struct Handler(mpsc::Sender<Result<Bytes, String>>);
impl NetHandler for Handler {
    fn bytes(self: Box<Self>, _: String, bytes: Bytes) {
        let _ = self.0.send(Ok(bytes));
    }
    fn error(self: Box<Self>, _: String, error: String) {
        let _ = self.0.send(Err(error));
    }
}
fn fetch(loader: &Loader, path: &str) -> Result<Bytes, String> {
    let (tx, rx) = mpsc::channel();
    loader.fetch(
        1,
        Request::get(Url::parse(path).unwrap()),
        Box::new(Handler(tx)),
    );
    rx.recv_timeout(Duration::from_secs(5)).unwrap()
}
fn done(loader: &Loader) {
    loader.shutdown();
    wait(|| loader.stats().stopped && loader.stats().file_active == 0);
    assert_eq!((loader.stats().active, loader.stats().queued), (0, 0));
}

#[test]
fn mounts_require_explicit_roots_and_decode_portable_file_paths_once() {
    let temp = Temp::new();
    fs::create_dir(temp.0.join("nested")).unwrap();
    fs::write(temp.0.join("nested/你好 world.txt"), b"allowed").unwrap();
    fs::write(temp.0.join("empty"), []).unwrap();
    fs::write(temp.0.join("%2e%2e"), b"literal encoded filename").unwrap();
    let mut config = temp.config();
    config
        .package
        .insert("/built-in".into(), Bytes::from_static(b"package"));
    let loader = Loader::new(config).unwrap();
    assert_eq!(
        fetch(
            &loader,
            "asset://media/nested/%E4%BD%A0%E5%A5%BD%20world.txt?v=1#x"
        )
        .unwrap(),
        b"allowed"[..]
    );
    assert!(fetch(&loader, "asset://media/empty").unwrap().is_empty());
    assert_eq!(
        fetch(&loader, "asset://media/%252e%252e").unwrap(),
        b"literal encoded filename"[..]
    );
    assert_eq!(
        fetch(&loader, "asset://app/built-in").unwrap(),
        b"package"[..]
    );
    for url in [
        "file:///outside",
        "asset://other/empty",
        "asset://user@media/empty",
        "asset://media:80/empty",
        "asset://media/nested",
        "asset://media/",
        "asset://media/a%2fb",
        "asset://media/a%5cb",
        "asset://media/%00",
        "asset://media/%ff",
        "asset://media/C%3A/x",
        "asset://media/CON.txt",
        "asset://media/LPT1",
        "asset://media/trailing%20",
        "asset://media/not-found",
    ] {
        assert!(fetch(&loader, url).is_err(), "must reject {url}");
    }
    done(&loader);
    for (name, root) in [
        ("app", temp.0.clone()),
        ("Upper", temp.0.clone()),
        ("-bad", temp.0.clone()),
        ("ok", PathBuf::from("relative")),
        ("ok", temp.0.join("empty")),
    ] {
        assert!(
            Loader::new(LoadConfig {
                mounts: BTreeMap::from([(name.into(), root)]),
                ..Default::default()
            })
            .is_err()
        );
    }
}

#[test]
fn files_obey_body_limits_and_opened_roots_do_not_follow_later_path_replacement() {
    let temp = Temp::new();
    let root = temp.0.join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("exact"), [42u8; 256]).unwrap();
    fs::write(root.join("large"), [0u8; 257]).unwrap();
    let loader = Loader::new(LoadConfig {
        mounts: BTreeMap::from([("media".into(), root.clone())]),
        limits: LoadLimits {
            body_bytes: 256,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    assert_eq!(fetch(&loader, "asset://media/exact").unwrap().len(), 256);
    assert!(
        fetch(&loader, "asset://media/large")
            .unwrap_err()
            .contains("budget")
    );
    match fs::rename(&root, temp.0.join("retained")) {
        Ok(()) => {
            fs::create_dir(&root).unwrap();
            fs::write(root.join("exact"), b"replacement").unwrap();
        }
        #[cfg(windows)]
        Err(error) => {
            // The selected capability implementation opens Windows roots
            // without delete sharing, preventing the rename itself.
            assert_eq!(error.raw_os_error(), Some(32));
            eprintln!("Windows mount handle prevented root replacement (sharing violation)");
        }
        #[cfg(not(windows))]
        Err(error) => panic!("root rename: {error}"),
    }
    assert_eq!(
        fetch(&loader, "asset://media/exact").unwrap(),
        [42u8; 256][..]
    );
    done(&loader);
    // Keep Loader alive for statistics; close still releases idle directory
    // handles (notably the Windows handle that prevented replacement above).
    fs::rename(&root, temp.0.join("after-close")).unwrap();
}

fn directory_link(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        // Junctions require no symlink privilege and exercise actual reparse
        // points on Windows. Creation only; Rust owns bounded fixture cleanup.
        let output = std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(output.status.success(), "junction: {:?}", output);
    }
}
#[test]
fn directory_links_cannot_escape_an_explicit_mount() {
    let temp = Temp::new();
    let root = temp.0.join("root");
    let outside = temp.0.join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("secret"), b"not granted").unwrap();
    directory_link(&outside, &root.join("escape"));
    let loader = Loader::new(LoadConfig {
        mounts: BTreeMap::from([("media".into(), root.clone())]),
        ..Default::default()
    })
    .unwrap();
    for _ in 0..20 {
        assert!(fetch(&loader, "asset://media/escape/secret").is_err());
    }
    #[cfg(unix)]
    {
        fs::create_dir(root.join("inside")).unwrap();
        fs::write(root.join("inside/ok"), b"inside").unwrap();
        directory_link(Path::new("inside"), &root.join("relative"));
        assert_eq!(
            fetch(&loader, "asset://media/relative/ok").unwrap(),
            b"inside"[..]
        );
        std::os::unix::fs::symlink("../outside/secret", root.join("relative-escape")).unwrap();
        assert!(fetch(&loader, "asset://media/relative-escape").is_err());
    }
    done(&loader);
}

#[cfg(unix)]
#[test]
fn concurrent_symlink_retargeting_never_reads_outside_and_special_files_are_rejected() {
    use std::sync::atomic::AtomicBool;
    let temp = Temp::new();
    let root = temp.0.join("root");
    let outside = temp.0.join("outside");
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("inside")).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(root.join("inside/value"), b"inside").unwrap();
    fs::write(outside.join("value"), b"outside").unwrap();
    let loader = Loader::new(LoadConfig {
        mounts: BTreeMap::from([("media".into(), root.clone())]),
        ..Default::default()
    })
    .unwrap();
    let socket = std::os::unix::net::UnixListener::bind(root.join("socket")).unwrap();
    assert!(
        fetch(&loader, "asset://media/socket")
            .unwrap_err()
            .contains("regular")
    );
    let fifo = std::process::Command::new("mkfifo")
        .arg(root.join("fifo"))
        .status()
        .unwrap();
    assert!(fifo.success());
    assert!(
        fetch(&loader, "asset://media/fifo")
            .unwrap_err()
            .contains("regular")
    );
    struct Race {
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl Drop for Race {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            self.thread.take().unwrap().join().unwrap();
        }
    }
    let stop = Arc::new(AtomicBool::new(false));
    let ending = stop.clone();
    let paths = root.clone();
    let thread = std::thread::spawn(move || {
        let mut inside = false;
        while !ending.load(Ordering::Acquire) {
            let target = if inside {
                Path::new("inside")
            } else {
                Path::new("../outside")
            };
            std::os::unix::fs::symlink(target, paths.join("next")).unwrap();
            fs::rename(paths.join("next"), paths.join("changing")).unwrap();
            inside = !inside;
            std::thread::yield_now();
        }
    });
    let race = Race {
        stop,
        thread: Some(thread),
    };
    for _ in 0..200 {
        if let Ok(bytes) = fetch(&loader, "asset://media/changing/value") {
            assert_eq!(bytes, b"inside"[..]);
        }
    }
    drop(race);
    drop(socket);
    done(&loader);
}

fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 8, 8);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&[255u8; 8 * 8 * 4])
        .unwrap();
    bytes
}

#[test]
fn mounted_stylesheets_resolve_relative_images_through_the_same_provider() {
    let temp = Temp::new();
    fs::create_dir(temp.0.join("css")).unwrap();
    fs::write(temp.0.join("css/main.css"), b"@import url('nested.css');").unwrap();
    fs::write(
        temp.0.join("css/nested.css"),
        b"img{display:block;width:32px;height:48px}body{background-image:url('../pixel.png')}",
    )
    .unwrap();
    fs::write(temp.0.join("pixel.png"), png()).unwrap();
    let loader = Arc::new(Loader::new(temp.config()).unwrap());
    let mut renderer = None;
    let app = App::new(|runtime| {
        let host = Renderer::with_config(
            runtime,
            Config {
                base_url: Url::parse("asset://media/").unwrap(),
                provider: Some(loader.clone()),
                ..Default::default()
            },
        );
        renderer = Some(host.clone());
        host
    });
    let renderer = renderer.unwrap();
    renderer.set_css("@import url('css/main.css');");
    app.mount(image().source("pixel.png").alt("mounted"))
        .unwrap();
    app.pump().unwrap();
    wait(|| {
        app.pump().unwrap();
        let s = loader.stats();
        s.active == 0
            && s.queued == 0
            && renderer.resource_stats().pending_images == 0
            && renderer.resource_stats().requests == 0
    });
    assert_eq!(loader.stats().failed, 0);
    assert!(loader.stats().completed >= 3, "{:?}", loader.stats());
    let physical = renderer
        .engine_id(app.tree().roots()[0], glory_html::Part::Main)
        .unwrap();
    let rect = renderer.inspect(|doc| doc.get_client_bounding_rect(physical).unwrap());
    assert_eq!((rect.width, rect.height), (32., 48.));
    assert_eq!(renderer.resource_stats().cached_images, 1);
    assert_eq!(
        renderer.resource_stats().cached_image_bytes,
        256 + "asset://media/pixel.png".len()
    );
    drop(app);
    drop(renderer);
    done(&loader);
}

#[test]
fn huge_file_image_lists_bound_workers_requests_and_history() {
    use glory_render::style::{Length, Style};
    use glory_ui::list;
    let temp = Temp::new();
    fs::write(temp.0.join("pixel.png"), png()).unwrap();
    for measured in [false, true] {
        let loader = Arc::new(
            Loader::new(LoadConfig {
                limits: LoadLimits {
                    workers: 2,
                    queued: 32,
                    ..Default::default()
                },
                ..temp.config()
            })
            .unwrap(),
        );
        let mut renderer = None;
        let app = App::new(|runtime| {
            let host = Renderer::with_config(
                runtime,
                Config {
                    base_url: Url::parse("asset://media/").unwrap(),
                    provider: Some(loader.clone()),
                    resources: ResourceLimits {
                        image_cache_entries: 8,
                        image_cache_bytes: 1024,
                        pending_images: 32,
                        requests: 32,
                        completion_bytes: 8192,
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
                    .source(format!("pixel.png?row={key}"))
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
                    let s = loader.stats();
                    renderer.resource_stats().pending_images == 0
                        && s.active == 0
                        && s.queued == 0
                        && s.file_active == 0
                });
            }
            let s = renderer.resource_stats();
            let transport = loader.stats();
            assert!(
                s.cached_images <= 8
                    && s.cached_image_bytes <= 1024
                    && s.pending_images <= 16
                    && s.image_waiters <= 16
                    && s.requests <= 32
                    && s.completion_bytes <= 8192,
                "{s:?}"
            );
            assert!(
                transport.file_active <= 2 && transport.active <= 2 && transport.queued <= 32,
                "{transport:?}"
            );
            assert!(app.stats().nodes < 64 && app.runtime_stats().node_slots < 160);
        }
        assert!(renderer.resource_stats().image_evictions > 20 && reads.get() < 4000);
        app.clear().unwrap();
        wait(|| {
            renderer.poll_resources();
            let s = loader.stats();
            s.active == 0
                && s.queued == 0
                && s.file_active == 0
                && renderer.resource_stats().requests == 0
        });
        assert_eq!(renderer.resource_stats().pending_images, 0);
        assert!(loader.stats().file_peak_active <= 2);
        eprintln!(
            "file list measured={measured}: reads={}, resources={:?}, loader={:?}",
            reads.get(),
            renderer.resource_stats(),
            loader.stats()
        );
        drop(app);
        drop(renderer);
        done(&loader);
    }
}
