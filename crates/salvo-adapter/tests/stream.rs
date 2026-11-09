#![cfg(all(feature = "stream", not(target_arch = "wasm32")))]

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use glory::prelude::*;
use glory_salvo::{StreamError, StreamPage, StreamRenderer, StreamTimeout, StreamTimeouts, ssr};
use http_body_util::BodyExt;
use salvo::conn::Acceptor;
use salvo::http::ResBody;
use salvo::prelude::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;
use tokio::time::{sleep, timeout};

// These timers run on separate native worker runtimes. Allow thread scheduling
// slack; the five-second watchdogs detect missing cleanup/wake independently.
const TIMEOUT_TEST_BUDGET: Duration = Duration::from_secs(1);

#[derive(Default)]
struct Metrics {
    polls: AtomicUsize,
    dropped: AtomicUsize,
    cleaned: AtomicUsize,
}
impl Metrics {
    fn polls(&self) -> usize {
        self.polls.load(Ordering::SeqCst)
    }
    fn cleaned(&self) -> usize {
        self.cleaned.load(Ordering::SeqCst)
    }
}
struct Waiting {
    receive: oneshot::Receiver<String>,
    metrics: Arc<Metrics>,
    // Prove resource futures stay local rather than requiring Send.
    _local: Rc<()>,
}
impl Future for Waiting {
    type Output = std::result::Result<String, String>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        this.metrics.polls.fetch_add(1, Ordering::SeqCst);
        Pin::new(&mut this.receive)
            .poll(cx)
            .map(|result| result.map_err(|error| error.to_string()))
    }
}
impl Drop for Waiting {
    fn drop(&mut self) {
        self.metrics.dropped.fetch_add(1, Ordering::SeqCst);
    }
}
type Gate = (oneshot::Receiver<String>, Arc<Metrics>);
fn gate() -> (oneshot::Sender<String>, Gate) {
    let (send, receive) = oneshot::channel();
    (send, (receive, Arc::new(Metrics::default())))
}
fn pending_stream(
    gates: Vec<Gate>,
    config: ssr::stream::Config,
) -> std::result::Result<ssr::stream::Stream, ssr::stream::Error> {
    ssr::stream_with(Resources::default(), config, move |scope, resources| {
        let children = gates
            .into_iter()
            .enumerate()
            .map(|(index, (receive, metrics))| {
                let cleanup = metrics.clone();
                scope
                    .owner()
                    .on_cleanup(move || {
                        cleanup.cleaned.fetch_add(1, Ordering::SeqCst);
                    })
                    .unwrap();
                let receive = Rc::new(RefCell::new(Some(receive)));
                let resource = resources
                    .bind(
                        scope.owner(),
                        &format!("r{index}"),
                        "v1",
                        move || index,
                        move |_| Waiting {
                            receive: receive.borrow_mut().take().unwrap(),
                            metrics: metrics.clone(),
                            _local: Rc::new(()),
                        },
                    )
                    .unwrap();
                text()
                    .class(format!("r{index}"))
                    .content(resource.map(|state| match state {
                        ResourceState::Ready { value, .. } => value.to_string(),
                        ResourceState::Failed { error, .. } => error.to_string(),
                        _ => "pending".into(),
                    }))
            })
            .collect::<Vec<_>>();
        view().fill(children)
    })
}
fn page() -> StreamPage {
    StreamPage {
        document: ssr::Document {
            title: "Salvo & stream".into(),
            nonce: Some("test-nonce".into()),
            ..Default::default()
        },
        bootstrap: ssr::stream::Bootstrap {
            root_id: "app".into(),
            data_id: "resources".into(),
            script_url: "/stream.js".into(),
            nonce: Some("test-nonce".into()),
        },
    }
}
async fn until(mut ready: impl FnMut() -> bool) {
    timeout(Duration::from_secs(5), async {
        while !ready() {
            sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("worker did not make progress");
}
async fn chunk(body: &mut ResBody) -> String {
    let frame = timeout(Duration::from_secs(5), body.frame())
        .await
        .unwrap()
        .expect("expected chunk")
        .expect("body error");
    String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap()
}

async fn assert_reusable(renderer: &StreamRenderer) {
    timeout(Duration::from_secs(5), async {
        loop {
            match renderer
                .render(page(), || pending_stream(vec![], Default::default()))
                .await
            {
                Err(StreamError::Busy) => tokio::task::yield_now().await,
                result => {
                    let html = result
                        .unwrap()
                        .take_body()
                        .collect()
                        .await
                        .unwrap()
                        .to_bytes();
                    assert!(String::from_utf8_lossy(&html).contains("\"kind\":\"end\""));
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn shell_is_early_and_each_body_demand_controls_local_progress() {
    let renderer = StreamRenderer::new(1).unwrap();
    let (send, gate) = gate();
    let metrics = gate.1.clone();
    let mut response = renderer
        .render(page(), move || {
            pending_stream(vec![gate], Default::default())
        })
        .await
        .unwrap();
    assert_eq!(response.status_code, Some(StatusCode::OK));
    assert_eq!(
        response.headers()["content-type"],
        "text/html; charset=utf-8"
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-accel-buffering"], "no");
    assert!(!response.headers().contains_key("content-length"));
    let mut body = response.take_body();
    let shell = chunk(&mut body).await;
    assert!(shell.starts_with("<!doctype html>"));
    assert!(shell.contains("<title>Salvo &amp; stream</title>"));
    assert!(
        shell
            .find("<script src=\"/stream.js\" nonce=\"test-nonce\"")
            .unwrap()
            < shell.find("id=\"app\"").unwrap()
    );
    assert!(shell.contains("pending"));
    assert_eq!(metrics.polls(), 0);
    // A ready loader must still not run while the network body has no demand.
    send.send("late <value>".into()).unwrap();
    sleep(Duration::from_millis(30)).await;
    assert_eq!(metrics.polls(), 0);
    let patch = chunk(&mut body).await;
    assert!(patch.contains("\"kind\":\"patch\""));
    assert!(patch.contains("late \\u003cvalue\\u003e"));
    let end = chunk(&mut body).await;
    assert!(end.contains("\"kind\":\"end\""));
    assert!(end.contains("\"rekey\""));
    assert!(end.contains("\"resources\""));
    assert!(end.ends_with("</body></html>"));
    assert!(body.frame().await.is_none());
    assert!(body.frame().await.is_none());
    until(|| metrics.cleaned() == 1).await;
}

#[tokio::test]
async fn reverse_completion_is_incremental_and_end_exports_both_resources() {
    let renderer = StreamRenderer::new(1).unwrap();
    let (send_a, a) = gate();
    let (send_b, b) = gate();
    let mut response = renderer
        .render(page(), move || {
            pending_stream(vec![a, b], Default::default())
        })
        .await
        .unwrap();
    let mut body = response.take_body();
    chunk(&mut body).await;
    send_b.send("second first".into()).unwrap();
    let b = chunk(&mut body).await;
    assert!(b.contains("second first") && !b.contains("first last"));
    send_a.send("first last".into()).unwrap();
    let a = chunk(&mut body).await;
    assert!(a.contains("first last") && !a.contains("second first"));
    let end = chunk(&mut body).await;
    assert!(end.contains("second first") && end.contains("first last"));
    assert!(end.contains("\"kind\":\"end\""));
}

#[tokio::test]
async fn dropping_an_unread_response_cancels_without_polling_the_loader() {
    let renderer = StreamRenderer::new(1).unwrap();
    let (mut send, gate) = gate();
    let metrics = gate.1.clone();
    let response = renderer
        .render(page(), move || {
            pending_stream(vec![gate], Default::default())
        })
        .await
        .unwrap();
    drop(response);
    timeout(Duration::from_secs(5), send.closed())
        .await
        .unwrap();
    until(|| metrics.cleaned() == 1).await;
    assert_eq!(metrics.polls(), 0);
}

#[tokio::test]
async fn dropping_a_pending_body_cancels_io_and_releases_request_admission() {
    let renderer = StreamRenderer::new(1).unwrap();
    let (mut send, gate) = gate();
    let metrics = gate.1.clone();
    let mut response = renderer
        .render(page(), move || {
            pending_stream(vec![gate], Default::default())
        })
        .await
        .unwrap();
    let mut body = response.take_body();
    chunk(&mut body).await;
    assert!(
        timeout(Duration::from_millis(30), body.frame())
            .await
            .is_err()
    );
    until(|| metrics.polls() > 0).await;
    let rejected = renderer
        .render(page(), || panic!("busy factory must not execute"))
        .await;
    assert!(matches!(rejected, Err(StreamError::Busy)));
    drop(body);
    timeout(Duration::from_secs(5), send.closed())
        .await
        .unwrap();
    until(|| metrics.cleaned() == 1 && metrics.dropped.load(Ordering::SeqCst) == 1).await;
    // Permit release follows all UI cleanup; retry admission across that handoff.
    timeout(Duration::from_secs(5), async {
        loop {
            match renderer
                .render(page(), || pending_stream(vec![], Default::default()))
                .await
            {
                Err(StreamError::Busy) => tokio::task::yield_now().await,
                result => {
                    drop(result.unwrap());
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn outer_configuration_is_validated_before_the_factory() {
    assert!(matches!(StreamRenderer::new(0), Err(StreamError::Config)));
    let renderer = StreamRenderer::new(1).unwrap();
    let mut invalid = page();
    invalid.bootstrap.data_id = invalid.bootstrap.root_id.clone();
    let result = renderer
        .render(invalid, || panic!("invalid bootstrap must not mount"))
        .await;
    assert!(matches!(
        result,
        Err(StreamError::Render(ssr::stream::Error::Config))
    ));
}

#[test]
fn timeout_configuration_rejects_zero_and_unrepresentable_durations() {
    for duration in [Duration::ZERO, Duration::MAX] {
        for timeouts in [
            StreamTimeouts {
                request: Some(duration),
                idle: None,
            },
            StreamTimeouts {
                request: None,
                idle: Some(duration),
            },
        ] {
            assert!(matches!(
                StreamRenderer::with_timeouts(1, timeouts),
                Err(StreamError::Config)
            ));
        }
    }
}

#[tokio::test]
async fn request_and_idle_timeouts_cancel_pending_frames_without_dropping_the_body() {
    for (timeouts, expected) in [
        (
            StreamTimeouts {
                request: Some(TIMEOUT_TEST_BUDGET),
                idle: None,
            },
            "Timeout(Request)",
        ),
        (
            StreamTimeouts {
                request: None,
                idle: Some(TIMEOUT_TEST_BUDGET),
            },
            "Timeout(Idle)",
        ),
    ] {
        let renderer = StreamRenderer::with_timeouts(1, timeouts).unwrap();
        let (mut send, gate) = gate();
        let metrics = gate.1.clone();
        let mut response = renderer
            .render(page(), move || {
                pending_stream(vec![gate], Default::default())
            })
            .await
            .unwrap();
        let mut body = response.take_body();
        assert!(!chunk(&mut body).await.contains("\"kind\":\"end\""));
        let error = timeout(Duration::from_secs(5), body.frame())
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert_eq!(error.to_string(), expected);
        assert!(body.frame().await.is_none());
        timeout(Duration::from_secs(5), send.closed())
            .await
            .unwrap();
        until(|| metrics.cleaned() == 1 && metrics.dropped.load(Ordering::SeqCst) == 1).await;
        assert!(metrics.polls() > 0);
        assert_reusable(&renderer).await;
    }
}

#[tokio::test]
async fn timeouts_reclaim_unread_responses_and_preserve_the_error_for_later_polls() {
    for (timeouts, expected) in [
        (
            StreamTimeouts {
                request: Some(TIMEOUT_TEST_BUDGET),
                idle: None,
            },
            "Timeout(Request)",
        ),
        (
            StreamTimeouts {
                request: None,
                idle: Some(TIMEOUT_TEST_BUDGET),
            },
            "Timeout(Idle)",
        ),
    ] {
        let renderer = StreamRenderer::with_timeouts(1, timeouts).unwrap();
        let (mut send, gate) = gate();
        let metrics = gate.1.clone();
        let mut response = renderer
            .render(page(), move || {
                pending_stream(vec![gate], Default::default())
            })
            .await
            .unwrap();
        // Keep the response alive without ever demanding a frame. The worker's
        // receive timeout must release its Owner and admission independently.
        timeout(Duration::from_secs(5), send.closed())
            .await
            .unwrap();
        until(|| metrics.cleaned() == 1).await;
        assert_eq!(metrics.polls(), 0);
        assert_reusable(&renderer).await;
        let mut body = response.take_body();
        let error = body.frame().await.unwrap().unwrap_err();
        assert_eq!(error.to_string(), expected);
        assert!(body.frame().await.is_none());
    }
}

#[tokio::test]
async fn request_timeout_checks_synchronous_startup_after_it_returns() {
    // A heavily delayed worker may correctly expire before invoking factory.
    // Validate that path, then retry: this test must still exercise at least one
    // factory that started, remained synchronous across expiry, and returned.
    for _ in 0..3 {
        let renderer = StreamRenderer::with_timeouts(
            1,
            StreamTimeouts {
                request: Some(TIMEOUT_TEST_BUDGET),
                idle: None,
            },
        )
        .unwrap();
        let (mut send, gate) = gate();
        let metrics = gate.1.clone();
        let (started, receive_started) = oneshot::channel();
        let (release, receive_release) = std::sync::mpsc::sync_channel(1);
        let worker = renderer.clone();
        let request = tokio::spawn(async move {
            worker
                .render(page(), move || {
                    started.send(()).unwrap();
                    receive_release
                        .recv_timeout(Duration::from_secs(5))
                        .unwrap();
                    pending_stream(vec![gate], Default::default())
                })
                .await
        });
        let entered = timeout(Duration::from_secs(5), receive_started)
            .await
            .expect("startup signal watchdog")
            .is_ok();
        if entered {
            // The deadline began before started was sent. Release only after
            // a full budget from that signal, without guessing thread latency.
            sleep(TIMEOUT_TEST_BUDGET).await;
            assert!(
                !request.is_finished(),
                "synchronous factory was not released"
            );
            assert_eq!(metrics.cleaned(), 0);
            release.send(()).unwrap();
        }
        let result = timeout(Duration::from_secs(5), request)
            .await
            .expect("startup completion watchdog")
            .unwrap();
        assert!(matches!(
            result,
            Err(StreamError::Timeout(StreamTimeout::Request))
        ));
        timeout(Duration::from_secs(5), send.closed())
            .await
            .unwrap();
        assert_eq!(metrics.polls(), 0);
        if entered {
            until(|| metrics.cleaned() == 1).await;
        } else {
            assert_eq!(metrics.cleaned(), 0, "factory never registered an Owner");
        }
        assert_reusable(&renderer).await;
        if entered {
            return;
        }
    }
    panic!("workers never entered the factory within three scheduling budgets");
}

#[tokio::test]
async fn startup_failures_and_panics_do_not_return_a_success_response_or_leak_slots() {
    let renderer = StreamRenderer::new(1).unwrap();
    let result = renderer
        .render(page(), || Err(ssr::stream::Error::Config))
        .await;
    assert!(matches!(
        result,
        Err(StreamError::Render(ssr::stream::Error::Config))
    ));
    assert_reusable(&renderer).await;
    let renderer = StreamRenderer::new(1).unwrap();
    let result = renderer
        .render(page(), || panic!("startup panic probe"))
        .await;
    assert!(matches!(result, Err(StreamError::WorkerStopped)));
    assert_reusable(&renderer).await;
    let renderer = StreamRenderer::new(1).unwrap();
    let result = renderer
        .render(page(), || {
            ssr::stream_with(
                Resources::default(),
                ssr::stream::Config {
                    chunk_bytes: 1,
                    ..Default::default()
                },
                |_, _| text().content("too big"),
            )
        })
        .await;
    assert!(matches!(
        result,
        Err(StreamError::Render(ssr::stream::Error::Limit(_)))
    ));
    assert_reusable(&renderer).await;
}

#[tokio::test]
async fn cancelling_during_startup_drops_the_late_shell_and_its_owner() {
    let renderer = StreamRenderer::new(1).unwrap();
    let clone = renderer.clone();
    let (started, receive_started) = oneshot::channel();
    let (release, receive_release) = std::sync::mpsc::sync_channel(1);
    let (mut send, gate) = gate();
    let metrics = gate.1.clone();
    let request = tokio::spawn(async move {
        clone
            .render(page(), move || {
                started.send(()).unwrap();
                // Simulate synchronous factory work crossing request cancellation.
                receive_release
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
                pending_stream(vec![gate], Default::default())
            })
            .await
    });
    timeout(Duration::from_secs(5), receive_started)
        .await
        .unwrap()
        .unwrap();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    release.send(()).unwrap();
    timeout(Duration::from_secs(5), send.closed())
        .await
        .unwrap();
    until(|| metrics.cleaned() == 1).await;
    assert_eq!(metrics.polls(), 0);
    assert_reusable(&renderer).await;
}

#[tokio::test]
async fn a_loader_panic_terminates_the_body_and_reclaims_its_owner() {
    let renderer = StreamRenderer::new(1).unwrap();
    let metrics = Arc::new(Metrics::default());
    let cleanup = metrics.clone();
    let mut response = renderer
        .render(page(), move || {
            ssr::stream_with(Resources::default(), Default::default(), |scope, _| {
                scope
                    .owner()
                    .on_cleanup(move || {
                        cleanup.cleaned.fetch_add(1, Ordering::SeqCst);
                    })
                    .unwrap();
                let value = scope.resource(
                    || 0_u64,
                    |_| {
                        std::future::poll_fn(|_| -> Poll<std::result::Result<String, String>> {
                            panic!("loader panic probe")
                        })
                    },
                );
                text().content(value.map(|_| "pending".into()))
            })
        })
        .await
        .unwrap();
    let mut body = response.take_body();
    chunk(&mut body).await;
    assert!(
        timeout(Duration::from_secs(5), body.frame())
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert!(body.frame().await.is_none());
    until(|| metrics.cleaned() == 1).await;
    assert_reusable(&renderer).await;
}

#[tokio::test]
async fn later_serialization_failure_is_a_body_error_without_a_successful_end() {
    let renderer = StreamRenderer::new(1).unwrap();
    let (send, gate) = gate();
    let metrics = gate.1.clone();
    let mut response = renderer
        .render(page(), move || {
            pending_stream(
                vec![gate],
                ssr::stream::Config {
                    chunk_bytes: 16 * 1024,
                    ..Default::default()
                },
            )
        })
        .await
        .unwrap();
    let mut body = response.take_body();
    chunk(&mut body).await;
    send.send("x".repeat(64 * 1024)).unwrap();
    let frame = timeout(Duration::from_secs(5), body.frame())
        .await
        .unwrap()
        .unwrap();
    assert!(frame.is_err());
    assert!(body.frame().await.is_none());
    until(|| metrics.cleaned() == 1).await;
}

#[tokio::test]
async fn simultaneous_requests_keep_resources_and_owners_independent() {
    let renderer = StreamRenderer::new(2).unwrap();
    let (send_a, a) = gate();
    let (send_b, b) = gate();
    let metrics_a = a.1.clone();
    let metrics_b = b.1.clone();
    let mut a = renderer
        .render(page(), move || pending_stream(vec![a], Default::default()))
        .await
        .unwrap();
    let mut b = renderer
        .render(page(), move || pending_stream(vec![b], Default::default()))
        .await
        .unwrap();
    drop(a.take_body());
    until(|| metrics_a.cleaned() == 1).await;
    assert_eq!(metrics_b.cleaned(), 0);
    assert!(send_a.send("must not publish".into()).is_err());
    let mut b = b.take_body();
    chunk(&mut b).await;
    send_b.send("request B".into()).unwrap();
    assert!(chunk(&mut b).await.contains("request B"));
    assert!(chunk(&mut b).await.contains("request B"));
}

type Factory =
    Box<dyn FnOnce() -> std::result::Result<ssr::stream::Stream, ssr::stream::Error> + Send>;
struct HttpStream {
    renderer: StreamRenderer,
    factory: Mutex<Option<Factory>>,
}
#[handler]
impl HttpStream {
    async fn handle(&self, response: &mut Response) {
        let factory = self.factory.lock().unwrap().take().unwrap();
        *response = self.renderer.render(page(), factory).await.unwrap();
    }
}
async fn server(
    gates: Vec<Gate>,
) -> (
    tokio::net::TcpStream,
    salvo::server::ServerHandle,
    tokio::task::JoinHandle<std::io::Result<()>>,
) {
    server_with_factory(move || pending_stream(gates, Default::default())).await
}
async fn server_with_factory(
    factory: impl FnOnce() -> std::result::Result<ssr::stream::Stream, ssr::stream::Error>
    + Send
    + 'static,
) -> (
    tokio::net::TcpStream,
    salvo::server::ServerHandle,
    tokio::task::JoinHandle<std::io::Result<()>>,
) {
    server_with_renderer(StreamRenderer::new(2).unwrap(), factory).await
}
async fn server_with_renderer(
    renderer: StreamRenderer,
    factory: impl FnOnce() -> std::result::Result<ssr::stream::Stream, ssr::stream::Error>
    + Send
    + 'static,
) -> (
    tokio::net::TcpStream,
    salvo::server::ServerHandle,
    tokio::task::JoinHandle<std::io::Result<()>>,
) {
    let acceptor = TcpListener::new("127.0.0.1:0").bind().await;
    let address = acceptor.holdings()[0]
        .local_addr
        .clone()
        .into_std()
        .unwrap();
    let server = Server::new(acceptor);
    let handle = server.handle();
    let router = Router::new().get(HttpStream {
        renderer,
        factory: Mutex::new(Some(Box::new(factory))),
    });
    let task = tokio::spawn(server.try_serve(router));
    let mut client = tokio::net::TcpStream::connect(address).await.unwrap();
    client
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    (client, handle, task)
}
async fn read_until(client: &mut tokio::net::TcpStream, needle: &str) -> String {
    timeout(Duration::from_secs(5), async {
        let mut bytes = Vec::new();
        let mut buffer = [0; 8192];
        loop {
            let count = client.read(&mut buffer).await.unwrap();
            assert!(count > 0, "connection ended before {needle}");
            bytes.extend_from_slice(&buffer[..count]);
            let text = String::from_utf8_lossy(&bytes);
            if text.contains(needle) {
                return text.into_owned();
            }
        }
    })
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_http_flushes_shell_before_completion_then_patch_and_end() {
    let (send, gate) = gate();
    let (mut client, handle, task) = server(vec![gate]).await;
    let shell = read_until(&mut client, "\"kind\":\"start\"").await;
    assert!(shell.starts_with("HTTP/1.1 200"));
    assert!(
        shell
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked")
    );
    assert!(shell.contains("pending"));
    assert!(!shell.contains("\"kind\":\"end\""));
    send.send("over the wire".into()).unwrap();
    let mut remaining = String::new();
    timeout(
        Duration::from_secs(5),
        client.read_to_string(&mut remaining),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        remaining.find("\"kind\":\"patch\"").unwrap() < remaining.find("\"kind\":\"end\"").unwrap()
    );
    assert!(remaining.contains("over the wire"));
    assert!(remaining.contains("</body></html>"));
    handle.stop_graceful(Some(Duration::from_secs(1)));
    task.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_socket_disconnect_drops_pending_local_resource_future() {
    let (mut send, gate) = gate();
    let metrics = gate.1.clone();
    let (mut client, handle, task) = server(vec![gate]).await;
    read_until(&mut client, "\"kind\":\"start\"").await;
    until(|| metrics.polls() > 0).await;
    drop(client);
    timeout(Duration::from_secs(5), send.closed())
        .await
        .unwrap();
    until(|| metrics.cleaned() == 1 && metrics.dropped.load(Ordering::SeqCst) == 1).await;
    handle.stop_graceful(Some(Duration::from_secs(1)));
    task.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_socket_stays_connected_while_request_timeout_cancels_its_pending_loader() {
    let renderer = StreamRenderer::with_timeouts(
        1,
        StreamTimeouts {
            request: Some(TIMEOUT_TEST_BUDGET),
            idle: None,
        },
    )
    .unwrap();
    let (mut send, gate) = gate();
    let metrics = gate.1.clone();
    let (mut client, handle, task) = server_with_renderer(renderer.clone(), move || {
        pending_stream(vec![gate], Default::default())
    })
    .await;
    let shell = read_until(&mut client, "\"kind\":\"start\"").await;
    assert!(!shell.contains("\"kind\":\"end\""));
    until(|| metrics.polls() > 0).await;
    // Do not disconnect: expiration must revoke this still-owned request.
    timeout(Duration::from_secs(5), send.closed())
        .await
        .unwrap();
    until(|| metrics.cleaned() == 1 && metrics.dropped.load(Ordering::SeqCst) == 1).await;
    let mut remaining = String::new();
    // An aborted chunked body can close cleanly or reset the socket.
    let _ = timeout(
        Duration::from_secs(5),
        client.read_to_string(&mut remaining),
    )
    .await
    .unwrap();
    assert!(!remaining.contains("\"kind\":\"end\""));
    assert_reusable(&renderer).await;
    handle.stop_graceful(Some(Duration::from_secs(1)));
    task.await.unwrap().unwrap();
}

struct Pressure {
    value: Cage<String>,
    metrics: Arc<Metrics>,
}
impl Future for Pressure {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let frame = self.metrics.polls.fetch_add(1, Ordering::SeqCst);
        self.value
            .revise(|value| *value = format!("{frame}:{}", "x".repeat(512 * 1024)));
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
impl Drop for Pressure {
    fn drop(&mut self) {
        self.metrics.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_paused_tcp_reader_stops_rendering_then_disconnect_cancels_the_producer() {
    let metrics = Arc::new(Metrics::default());
    let worker_metrics = metrics.clone();
    let (client, handle, task) = server_with_factory(move || {
        ssr::stream_with(Resources::default(), Default::default(), |scope, _| {
            let value = scope.cage("small shell".to_owned());
            let cleanup = worker_metrics.clone();
            scope
                .owner()
                .on_cleanup(move || {
                    cleanup.cleaned.fetch_add(1, Ordering::SeqCst);
                })
                .unwrap();
            scope
                .spawn(Pressure {
                    value,
                    metrics: worker_metrics,
                })
                .unwrap();
            text().content(value)
        })
    })
    .await;
    // Do not read from the socket. Kernel/HTTP buffers can absorb a bounded
    // prefix, but an infinite ready producer must eventually stop advancing.
    until(|| metrics.polls() > 0).await;
    timeout(Duration::from_secs(5), async {
        let mut previous = metrics.polls();
        let mut stable = 0;
        while stable < 4 {
            sleep(Duration::from_millis(100)).await;
            let current = metrics.polls();
            assert!(
                current < 64,
                "HTTP body rendered ahead despite socket backpressure"
            );
            stable = if previous == current { stable + 1 } else { 0 };
            previous = current;
        }
    })
    .await
    .expect("paused TCP reader never stopped the producer");
    drop(client);
    until(|| metrics.cleaned() == 1 && metrics.dropped.load(Ordering::SeqCst) == 1).await;
    handle.stop_graceful(Some(Duration::from_secs(1)));
    task.await.unwrap().unwrap();
}
