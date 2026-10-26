use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, ready};

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
}
impl StreamRenderer {
    pub fn new(max_requests: usize) -> Result<Self, StreamError> {
        if max_requests == 0 || max_requests > Semaphore::MAX_PERMITS {
            return Err(StreamError::Config);
        }
        Ok(Self {
            slots: Arc::new(Semaphore::new(max_requests)),
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
                runtime.block_on(serve(page.bootstrap, opening, factory, started, requests));
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
        });
        Ok(response)
    }
}

struct Chunk {
    html: String,
    end: bool,
}
type Reply = oneshot::Sender<Result<Chunk, StreamError>>;

async fn serve(
    bootstrap: Bootstrap,
    mut opening: String,
    factory: impl FnOnce() -> Result<SsrStream, glory_ssr::stream::Error>,
    started: oneshot::Sender<Result<String, StreamError>>,
    mut requests: mpsc::Receiver<Reply>,
) {
    if started.is_closed() {
        return;
    }
    let mut stream = match factory() {
        Ok(stream) => stream,
        Err(error) => {
            let _ = started.send(Err(StreamError::Render(error)));
            return;
        }
    };
    // next_frame yields the prebuilt shell without polling the local executor.
    let shell = match stream.next_frame().await {
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
    if started.send(Ok(opening)).is_err() {
        return;
    }
    while let Some(mut reply) = requests.recv().await {
        let frame = tokio::select! {
            biased;
            _ = reply.closed() => break,
            frame = stream.next_frame() => frame,
        };
        let result = match frame {
            Some(Ok(frame)) => frame
                .to_html(&bootstrap)
                .map(|mut html| {
                    let end = frame.kind() == FrameKind::End;
                    if end {
                        html.push_str("</body></html>");
                    }
                    Chunk { html, end }
                })
                .map_err(StreamError::Render),
            Some(Err(error)) => Err(StreamError::Render(error)),
            None => Err(StreamError::WorkerStopped),
        };
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
}
impl Body {
    fn finish(&mut self) {
        self.done = true;
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
                return Poll::Ready(Some(Err(StreamError::WorkerStopped)));
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
                Poll::Ready(Some(Err(StreamError::WorkerStopped)))
            }
        }
    }
}
