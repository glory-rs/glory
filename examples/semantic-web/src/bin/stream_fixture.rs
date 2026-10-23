//! A transport acceptance fixture, not a production HTTP server. Each request
//! has an explicitly driven local executor on its connection thread. Socket
//! writes provide demand; a separate socket read detects a pending disconnect.
#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    native::serve()
}
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use glory::prelude::*;
    use glory::ssr::stream::{Config, FrameKind, Stream};
    use std::future::Future;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::task::{Context, Poll, Wake, Waker};
    use std::time::Duration;

    #[derive(Default)]
    struct Metrics {
        active: AtomicUsize,
        completed: AtomicUsize,
        cancelled: AtomicUsize,
        timers: AtomicUsize,
        produced: AtomicUsize,
    }
    #[derive(Default)]
    struct TimerState {
        ready: bool,
        cancelled: bool,
        waker: Option<Waker>,
    }
    struct Timer(Arc<(Mutex<TimerState>, Condvar)>);
    impl Timer {
        fn new(milliseconds: u64, metrics: Arc<Metrics>) -> Self {
            let state = Arc::new((Mutex::new(TimerState::default()), Condvar::new()));
            let worker = state.clone();
            metrics.timers.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || {
                let (lock, signal) = &*worker;
                let (mut state, _) = signal
                    .wait_timeout_while(
                        lock.lock().unwrap(),
                        Duration::from_millis(milliseconds),
                        |state| !state.cancelled,
                    )
                    .unwrap();
                let waker = if state.cancelled {
                    None
                } else {
                    state.ready = true;
                    state.waker.take()
                };
                drop(state);
                metrics.timers.fetch_sub(1, Ordering::SeqCst);
                if let Some(waker) = waker {
                    waker.wake();
                }
            });
            Self(state)
        }
    }
    impl Future for Timer {
        type Output = ();
        fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
            let mut state = self.0.0.lock().unwrap();
            if state.ready {
                Poll::Ready(())
            } else {
                state.waker = Some(context.waker().clone());
                Poll::Pending
            }
        }
    }
    impl Drop for Timer {
        fn drop(&mut self) {
            let waker = {
                let mut state = self.0.0.lock().unwrap();
                state.cancelled = true;
                state.waker.take()
            };
            self.0.1.notify_one();
            drop(waker);
        }
    }
    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    fn next(
        stream: &mut Stream,
        cancelled: &AtomicBool,
    ) -> std::io::Result<Option<glory::ssr::stream::Frame>> {
        let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(stream.next_frame());
        loop {
            if cancelled.load(Ordering::SeqCst) {
                return Err(std::io::ErrorKind::ConnectionAborted.into());
            }
            match future.as_mut().poll(&mut context) {
                Poll::Ready(Some(Ok(frame))) => return Ok(Some(frame)),
                Poll::Ready(Some(Err(error))) => {
                    return Err(std::io::Error::other(error.to_string()));
                }
                Poll::Ready(None) => return Ok(None),
                Poll::Pending => std::thread::park(),
            }
        }
    }
    fn ui(scope: &Scope, resources: &Resources, metrics: Arc<Metrics>) -> impl Widget + use<> {
        let ma = metrics.clone();
        let a = resources
            .bind(
                scope.owner(),
                "first",
                "v1",
                || 1_u64,
                move |_| {
                    let ma = ma.clone();
                    async move {
                        Timer::new(100, ma).await;
                        Ok::<_, String>("ready first".into())
                    }
                },
            )
            .unwrap();
        let b = resources
            .bind(
                scope.owner(),
                "second",
                "v1",
                || 2_u64,
                move |_| {
                    let metrics = metrics.clone();
                    async move {
                        Timer::new(500, metrics).await;
                        Ok::<_, String>("ready second".into())
                    }
                },
            )
            .unwrap();
        let label = |state: &ResourceState<u64, String, String>| match state {
            ResourceState::Ready { value, .. } => value.to_string(),
            _ => "pending".into(),
        };
        view().fill((
            text().content("shell now"),
            when(
                a.map(|state| matches!(state, ResourceState::Ready { .. })),
                move || text().content(a.map(label)),
            )
            .otherwise(|| text().content("waiting first")),
            when(
                b.map(|state| matches!(state, ResourceState::Ready { .. })),
                move || text().content(b.map(label)),
            )
            .otherwise(|| text().content("waiting second")),
        ))
    }
    struct Monitor {
        socket: TcpStream,
        done: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl Drop for Monitor {
        fn drop(&mut self) {
            self.done.store(true, Ordering::SeqCst);
            let _ = self.socket.shutdown(Shutdown::Both);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }
    fn stream(
        socket: &mut TcpStream,
        metrics: Arc<Metrics>,
        page: bool,
        reverse: bool,
        pressure: bool,
        broken: bool,
    ) -> std::io::Result<()> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let mut reader = socket.try_clone()?;
        let closer = socket.try_clone()?;
        let observe = cancelled.clone();
        let finished = done.clone();
        let driver = std::thread::current();
        let thread = std::thread::spawn(move || {
            let mut byte = [0];
            while let Ok(1) = reader.read(&mut byte) {}
            if !finished.load(Ordering::SeqCst) {
                observe.store(true, Ordering::SeqCst);
                driver.unpark();
            }
        });
        let _monitor = Monitor {
            socket: closer,
            done,
            thread: Some(thread),
        };
        let mut body = if pressure {
            glory::ssr::stream_with(Resources::default(), Config::default(), |_, resources| {
                let resources = resources.clone();
                view().fill(each(
                    (0_u64..24).collect::<Vec<_>>(),
                    |id| *id,
                    move |id, _| {
                        let resources = resources.clone();
                        let metrics = metrics.clone();
                        component(move |scope| {
                            let id = id.get();
                            let metrics = metrics.clone();
                            let value = resources
                                .bind(
                                    scope.owner(),
                                    &format!("pressure/{id}"),
                                    "v1",
                                    move || id,
                                    move |_| {
                                        let metrics = metrics.clone();
                                        async move {
                                            metrics.produced.fetch_add(1, Ordering::SeqCst);
                                            Ok::<_, String>("x".repeat(256 * 1024))
                                        }
                                    },
                                )
                                .unwrap();
                            text()
                                .content(value.map(|state| match state {
                                    ResourceState::Ready { value, .. } => value.to_string(),
                                    _ => "pending".into(),
                                }))
                                .show_in(scope)
                        })
                    },
                ))
            })
        } else if page {
            let load: glory_semantic_web::streaming::Load = std::rc::Rc::new(move |id| {
                let metrics = metrics.clone();
                Box::pin(async move {
                    Timer::new(
                        if id == (if reverse { 2 } else { 1 }) {
                            1500
                        } else {
                            3000
                        },
                        metrics,
                    )
                    .await;
                    Ok(if id == 1 {
                        "ready first </script><script>window.streamInjected=1</script> 中文".into()
                    } else {
                        "ready second".into()
                    })
                })
            });
            glory::ssr::stream_with(Resources::default(), Config::default(), |_, resources| {
                glory_semantic_web::streaming::widget(resources.clone(), load, Default::default())
            })
        } else {
            glory::ssr::stream_with(
                Resources::default(),
                Config::default(),
                |scope, resources| ui(scope, resources, metrics),
            )
        }
        .map_err(|error| std::io::Error::other(error.to_string()))?;
        if page {
            let head = glory::ssr::stream::Bootstrap {
                root_id: "stream-root".into(),
                data_id: "stream-data".into(),
                script_url: "/glory-stream.js".into(),
                nonce: Some("fixture".into()),
            }
            .head_script()
            .map_err(|error| std::io::Error::other(error.to_string()))?;
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'; script-src 'self' 'wasm-unsafe-eval' 'nonce-fixture'; style-src 'nonce-fixture'; style-src-attr 'unsafe-inline'; connect-src 'self'\r\nConnection: close\r\n\r\n<!doctype html><html><head><meta charset=\"utf-8\"><title>Glory progressive stream</title><style nonce=\"fixture\" data-gly-base=\"v1\">{}</style>{head}<script type=\"module\" src=\"/stream-client-bootstrap.js\"></script></head><body>",
                glory::html::BASE_CSS
            )?;
        } else {
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n")?;
        }
        let bootstrap = glory::ssr::stream::Bootstrap {
            root_id: "stream-root".into(),
            data_id: "stream-data".into(),
            script_url: "/glory-stream-frame.js".into(),
            nonce: Some("fixture".into()),
        };
        while let Some(frame) = next(&mut body, &cancelled)? {
            if page {
                socket.write_all(
                    frame
                        .to_html(&bootstrap)
                        .map_err(|error| std::io::Error::other(error.to_string()))?
                        .as_bytes(),
                )?;
                if broken {
                    break;
                }
                continue;
            }
            let kind = match frame.kind() {
                FrameKind::Shell => "shell",
                FrameKind::Patch => "patch",
                FrameKind::End => "end",
            };
            let mut line = serde_json::to_vec(
                &serde_json::json!({"kind":kind,"sequence":frame.sequence(),"body":frame.body()}),
            )?;
            line.push(b'\n');
            socket.write_all(&line)?;
        }
        if page {
            socket.write_all(b"</body></html>")?;
        }
        Ok(())
    }
    fn asset(socket: &mut TcpStream, content_type: &str, bytes: &[u8]) -> std::io::Result<()> {
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        )?;
        socket.write_all(bytes)
    }
    fn connection(
        mut socket: TcpStream,
        metrics: Arc<Metrics>,
        directory: &Path,
    ) -> std::io::Result<()> {
        socket.set_nodelay(true)?;
        let mut request = Vec::new();
        let mut byte = [0];
        while request.len() < 16384 && !request.ends_with(b"\r\n\r\n") {
            if socket.read(&mut byte)? == 0 {
                return Ok(());
            }
            request.push(byte[0]);
        }
        if request.starts_with(b"GET /stats ") {
            let body = format!(
                "{{\"active\":{},\"completed\":{},\"cancelled\":{},\"timers\":{},\"produced\":{}}}",
                metrics.active.load(Ordering::SeqCst),
                metrics.completed.load(Ordering::SeqCst),
                metrics.cancelled.load(Ordering::SeqCst),
                metrics.timers.load(Ordering::SeqCst),
                metrics.produced.load(Ordering::SeqCst)
            );
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )?;
            return Ok(());
        }
        if request.starts_with(b"GET /glory-stream.js ")
            || request.starts_with(b"GET /glory-stream-frame.js ")
        {
            return asset(
                &mut socket,
                "text/javascript; charset=utf-8",
                glory::web::STREAM_SCRIPT.as_bytes(),
            );
        }
        if request.starts_with(b"GET /stream-client-bootstrap.js ") {
            return asset(
                &mut socket,
                "text/javascript",
                b"import init,* as demo from './glory_semantic_web.js';await init();window.demo=demo;window.ready=true;",
            );
        }
        for (name, mime) in [
            ("glory_semantic_web.js", "text/javascript"),
            ("glory_semantic_web_bg.wasm", "application/wasm"),
        ] {
            if request.starts_with(format!("GET /{name} ").as_bytes()) {
                return asset(&mut socket, mime, &std::fs::read(directory.join(name))?);
            }
        }
        let reverse = request.starts_with(b"GET /reverse ");
        let broken = request.starts_with(b"GET /broken ");
        let pressure = request.starts_with(b"GET /pressure ");
        let page = request.starts_with(b"GET /page ") || reverse || broken;
        if !request.starts_with(b"GET /stream ") && !page && !pressure {
            socket.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )?;
            return Ok(());
        }
        metrics.active.fetch_add(1, Ordering::SeqCst);
        let result = stream(
            &mut socket,
            metrics.clone(),
            page,
            reverse,
            pressure,
            broken,
        );
        metrics.active.fetch_sub(1, Ordering::SeqCst);
        if result.is_ok() {
            metrics.completed.fetch_add(1, Ordering::SeqCst);
        } else {
            metrics.cancelled.fetch_add(1, Ordering::SeqCst);
        }
        result
    }
    pub fn serve() -> std::io::Result<()> {
        let address = std::env::args()
            .nth(2)
            .unwrap_or_else(|| "127.0.0.1:8789".into());
        let listener = TcpListener::bind(&address)?;
        let metrics = Arc::new(Metrics::default());
        let directory = PathBuf::from(
            std::env::args()
                .nth(1)
                .unwrap_or_else(|| "D:/BuildCache/glory-semantic-ui/web-demo".into()),
        );
        println!("Stream transport fixture at http://{address}/stream");
        for socket in listener.incoming() {
            let socket = socket?;
            let metrics = metrics.clone();
            let directory = directory.clone();
            std::thread::spawn(move || {
                let _ = connection(socket, metrics, &directory);
            });
        }
        Ok(())
    }
}
