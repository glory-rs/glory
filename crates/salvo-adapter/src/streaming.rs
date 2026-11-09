use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, ready};
use std::time::{Duration, Instant};

use futures_core::Stream;
use glory_ssr::stream::{Bootstrap, FrameKind, Stream as SsrStream};
use salvo::Response;
use salvo::http::{HeaderValue, header};
use tokio::sync::{Semaphore, mpsc, oneshot};

/// Document and bootstrap configuration. Serve `glory_web::STREAM_SCRIPT` at
/// `bootstrap.script_url` under the application's CSP. Hydration is optional.
#[derive(Clone, Debug)]
pub struct StreamPage {
    pub document: glory_ssr::Document,
    pub bootstrap: Bootstrap,
}

/// Optional cooperative limits for admitted requests. Defaults preserve streams
/// with arbitrarily slow resources or clients. Neither limit preempts synchronous
/// application code; expiration is checked when that code returns.
#[derive(Clone, Copy, Debug, Default)]
pub struct StreamTimeouts {
    /// Total time from admission, including startup and all body demands.
    pub request: Option<Duration>,
    /// Time without a successful frame reply, including waiting for body demand
    /// or a pending loader. Starts after the shell is prepared.
    pub idle: Option<Duration>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Which configured deadline expired. No application state crosses threads.
pub enum StreamTimeout {
    /// The admitted request exceeded its total lifetime.
    Request,
    /// No frame reply was prepared within the configured idle interval.
    Idle,
}

#[derive(Debug)]
pub enum StreamError {
    /// No worker is admitted; retry or return HTTP 503. There is no wait queue.
    Busy,
    Config,
    Ui(glory_ui::Error),
    Render(glory_ssr::stream::Error),
    Io(io::Error),
    /// A worker exited (including a panic) without completing its reply.
    WorkerStopped,
    /// Cooperative request/idle expiration; the stream emits no successful End.
    Timeout(StreamTimeout),
}
impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for StreamError {}

/// Share one renderer across routes. At most `max_requests` native worker
/// threads exist at once; each owns a current-thread Tokio runtime and one
/// request's !Send UI. Only owned HTML and errors cross the thread boundary.
///
/// The network body requests exactly one frame per poll cycle. There is no
/// eager producer or queued frame backlog. Dropping it cancels idle workers
/// and workers awaiting resource I/O, without blocking the network thread.
#[derive(Clone)]
pub struct StreamRenderer {
    slots: Arc<Semaphore>,
    timeouts: StreamTimeouts,
}
impl StreamRenderer {
    pub fn new(max_requests: usize) -> Result<Self, StreamError> {
        Self::with_timeouts(max_requests, StreamTimeouts::default())
    }

    /// Set explicit request/idle limits shared by cloned renderers. Zero and
    /// unrepresentable durations are rejected before any worker is admitted.
    pub fn with_timeouts(
        max_requests: usize,
        timeouts: StreamTimeouts,
    ) -> Result<Self, StreamError> {
        if max_requests == 0
            || max_requests > Semaphore::MAX_PERMITS
            || [timeouts.request, timeouts.idle]
                .into_iter()
                .flatten()
                .any(|time| time.is_zero() || Instant::now().checked_add(time).is_none())
        {
            return Err(StreamError::Config);
        }
        Ok(Self {
            slots: Arc::new(Semaphore::new(max_requests)),
            timeouts,
        })
    }

    /// Construct the Stream, Resources and UI inside `factory` on the worker.
    /// Captures must be Send; the UI and its resource futures need not be.
    ///
    /// Returns only after shell serialization succeeds, before polling any
    /// resource loader. Startup failures can still become an HTTP error response.
    /// Later failures are body errors and terminate the transport, never a false
    /// successful End. Middleware/proxies must preserve incremental flushing.
    pub async fn render(
        &self,
        page: StreamPage,
        factory: impl FnOnce() -> Result<SsrStream, glory_ssr::stream::Error> + Send + 'static,
    ) -> Result<Response, StreamError> {
        // Validate all outer configuration before executing application code.
        let mut opening = page.document.opening_html().map_err(StreamError::Ui)?;
        opening.push_str(&page.bootstrap.head_script().map_err(StreamError::Render)?);
        let permit = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| StreamError::Busy)?;
        let (started, initial) = oneshot::channel();
        let (demand, requests) = mpsc::channel(1);
        let timing = Timing::new(self.timeouts)?;
        let terminal = Arc::new(Mutex::new(None));
        let worker_terminal = terminal.clone();
        std::thread::Builder::new()
            .name("glory-salvo-ssr".into())
            .spawn(move || {
                // Release admission only after the UI, futures and runtime drop.
                let _permit = permit;
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = started.send(Err(StreamError::Io(error)));
                        return;
                    }
                };
                runtime.block_on(serve(
                    page.bootstrap,
                    opening,
                    factory,
                    started,
                    requests,
                    timing,
                    worker_terminal,
                ));
            })
            .map_err(StreamError::Io)?;
        let first = initial.await.map_err(|_| StreamError::WorkerStopped)??;
        let mut response = super::html_headers();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
            .headers_mut()
            .insert("x-accel-buffering", HeaderValue::from_static("no"));
        response.stream(Body {
            first: Some(first),
            demand: Some(demand),
            pending: None,
            done: false,
            terminal,
        });
        Ok(response)
    }
}

struct Chunk {
    html: String,
    end: bool,
}
type Reply = oneshot::Sender<Result<Chunk, StreamError>>;
type Terminal = Arc<Mutex<Option<StreamTimeout>>>;

struct Timing {
    request: Option<Instant>,
    idle: Option<Instant>,
    idle_duration: Option<Duration>,
}
impl Timing {
    fn new(timeouts: StreamTimeouts) -> Result<Self, StreamError> {
        let now = Instant::now();
        let request = timeouts
            .request
            .map(|duration| now.checked_add(duration).ok_or(StreamError::Config))
            .transpose()?;
        Ok(Self {
            request,
            idle: None,
            idle_duration: timeouts.idle,
        })
    }
    fn progress(&mut self) -> Result<(), StreamError> {
        self.idle = self
            .idle_duration
            .map(|duration| {
                Instant::now()
                    .checked_add(duration)
                    .ok_or(StreamError::Config)
            })
            .transpose()?;
        Ok(())
    }
    fn deadline(&self) -> Option<(Instant, StreamTimeout)> {
        match (self.request, self.idle) {
            (Some(request), Some(idle)) if idle < request => Some((idle, StreamTimeout::Idle)),
            (Some(request), _) => Some((request, StreamTimeout::Request)),
            (_, Some(idle)) => Some((idle, StreamTimeout::Idle)),
            _ => None,
        }
    }
    fn expired(&self) -> Option<StreamTimeout> {
        self.deadline()
            .filter(|(deadline, _)| *deadline <= Instant::now())
            .map(|(_, kind)| kind)
    }
    async fn wait(&self) -> StreamTimeout {
        match self.deadline() {
            Some((deadline, kind)) => {
                tokio::time::sleep_until(deadline.into()).await;
                kind
            }
            None => std::future::pending().await,
        }
    }
}
fn expire(terminal: &Terminal, kind: StreamTimeout) -> StreamError {
    *terminal.lock().expect("stream terminal state") = Some(kind);
    StreamError::Timeout(kind)
}

async fn serve(
    bootstrap: Bootstrap,
    mut opening: String,
    factory: impl FnOnce() -> Result<SsrStream, glory_ssr::stream::Error>,
    started: oneshot::Sender<Result<String, StreamError>>,
    mut requests: mpsc::Receiver<Reply>,
    mut timing: Timing,
    terminal: Terminal,
) {
    if started.is_closed() {
        return;
    }
    if let Some(kind) = timing.expired() {
        let _ = started.send(Err(expire(&terminal, kind)));
        return;
    }
    let created = factory();
    if let Some(kind) = timing.expired() {
        let _ = started.send(Err(expire(&terminal, kind)));
        return;
    }
    let mut stream = match created {
        Ok(stream) => stream,
        Err(error) => {
            let _ = started.send(Err(StreamError::Render(error)));
            return;
        }
    };
    // next_frame yields the prebuilt shell without polling the local executor.
    let frame = tokio::select! {
        biased;
        kind = timing.wait() => {
            let _ = started.send(Err(expire(&terminal, kind)));
            return;
        }
        frame = stream.next_frame() => frame,
    };
    let shell = match frame {
        Some(Ok(frame)) => frame.to_html(&bootstrap).map_err(StreamError::Render),
        Some(Err(error)) => Err(StreamError::Render(error)),
        None => Err(StreamError::WorkerStopped),
    };
    match shell {
        Ok(shell) => opening.push_str(&shell),
        Err(error) => {
            let _ = started.send(Err(error));
            return;
        }
    }
    if let Some(kind) = timing.expired() {
        let _ = started.send(Err(expire(&terminal, kind)));
        return;
    }
    if let Err(error) = timing.progress() {
        let _ = started.send(Err(error));
        return;
    }
    if started.send(Ok(opening)).is_err() {
        return;
    }
    loop {
        let Some(mut reply) = (tokio::select! {
            biased;
            kind = timing.wait() => { expire(&terminal, kind); break; }
            reply = requests.recv() => reply,
        }) else {
            break;
        };
        let result = tokio::select! {
            biased;
            kind = timing.wait() => Err(expire(&terminal, kind)),
            _ = reply.closed() => break,
            frame = stream.next_frame() => Ok(frame),
        };
        let result = match result {
            Ok(Some(Ok(frame))) => frame
                .to_html(&bootstrap)
                .map(|mut html| {
                    let end = frame.kind() == FrameKind::End;
                    if end {
                        html.push_str("</body></html>");
                    }
                    Chunk { html, end }
                })
                .map_err(StreamError::Render),
            Ok(Some(Err(error))) => Err(StreamError::Render(error)),
            Ok(None) => Err(StreamError::WorkerStopped),
            Err(error) => Err(error),
        };
        let result = if let Some(kind) = timing.expired() {
            Err(expire(&terminal, kind))
        } else {
            result
        };
        let result = result.and_then(|chunk| {
            timing.progress()?;
            Ok(chunk)
        });
        let terminal = match &result {
            Ok(chunk) => chunk.end,
            Err(_) => true,
        };
        if reply.send(result).is_err() || terminal {
            break;
        }
    }
    // SsrStream::drop cancels its App and all resource futures here, locally.
}

struct Body {
    first: Option<String>,
    demand: Option<mpsc::Sender<Reply>>,
    pending: Option<oneshot::Receiver<Result<Chunk, StreamError>>>,
    done: bool,
    terminal: Terminal,
}
impl Body {
    fn stopped(&self) -> StreamError {
        self.terminal
            .lock()
            .expect("stream terminal state")
            .map_or(StreamError::WorkerStopped, StreamError::Timeout)
    }
    fn finish(&mut self) {
        self.done = true;
        self.first = None;
        self.pending = None;
        self.demand = None;
    }
}
impl Stream for Body {
    type Item = Result<String, StreamError>;
    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.done {
            return Poll::Ready(None);
        }
        let timeout = *this.terminal.lock().expect("stream terminal state");
        if let Some(kind) = timeout {
            this.finish();
            return Poll::Ready(Some(Err(StreamError::Timeout(kind))));
        }
        if let Some(first) = this.first.take() {
            return Poll::Ready(Some(Ok(first)));
        }
        if this.pending.is_none() {
            let (reply, receive) = oneshot::channel();
            if this
                .demand
                .as_ref()
                .expect("active body")
                .try_send(reply)
                .is_err()
            {
                this.finish();
                return Poll::Ready(Some(Err(this.stopped())));
            }
            this.pending = Some(receive);
        }
        let result = ready!(Pin::new(this.pending.as_mut().expect("pending reply")).poll(context));
        this.pending = None;
        match result {
            Ok(Ok(chunk)) => {
                if chunk.end {
                    this.finish();
                }
                Poll::Ready(Some(Ok(chunk.html)))
            }
            Ok(Err(error)) => {
                this.finish();
                Poll::Ready(Some(Err(error)))
            }
            Err(_) => {
                this.finish();
                Poll::Ready(Some(Err(this.stopped())))
            }
        }
    }
}
