//! Explicit native text clipboard I/O, separate from semantic request ownership.
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

pub type Reply<T> = Pin<Box<dyn Future<Output = Result<T, ClipboardError>> + Send + 'static>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardError {
    Unavailable,
    Denied,
    Failed,
    Busy,
    TooLong,
    InvalidLimits,
}
impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ClipboardError {}

/// Constructors must return promptly and never call UI/business code. Native
/// completions wake their future; the document alone delivers semantic events.
/// Dropping a future withdraws its receipt, not an OS operation already started.
/// Drivers must retain native admission until that operation actually settles.
pub trait ClipboardDriver: Send + Sync + 'static {
    fn write(&self, text: String) -> Reply<()>;
    fn read(&self, max_bytes: usize) -> Reply<String> {
        let _ = max_bytes;
        Box::pin(std::future::ready(Err(ClipboardError::Unavailable)))
    }
}

#[derive(Clone)]
pub struct Clipboard {
    pub(crate) driver: Arc<dyn ClipboardDriver>,
    pub(crate) timeout: Duration,
}
impl Clipboard {
    pub fn new(driver: Arc<dyn ClipboardDriver>) -> Self {
        Self {
            driver,
            timeout: Duration::from_secs(30),
        }
    }
    pub fn with_timeout(
        driver: Arc<dyn ClipboardDriver>,
        timeout: Duration,
    ) -> Result<Self, ClipboardError> {
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(ClipboardError::InvalidLimits);
        }
        Ok(Self { driver, timeout })
    }
    /// One worker and native operation permit. OS clipboard creation is lazy
    /// and stays on that worker; failures are results, never initialization panics.
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub fn desktop() -> Result<Self, ClipboardError> {
        Ok(Self::new(Arc::new(desktop::Desktop::new()?)))
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
mod desktop {
    use super::*;
    use glory_render::MAX_CLIPBOARD_BYTES;
    use std::{
        sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, SyncSender},
        },
        task::{Context, Poll, Waker},
    };

    enum Payload {
        Written,
        Text(String),
    }
    enum Command {
        Write(String),
        Read(usize),
    }
    enum Delivery {
        Pending(Option<Waker>),
        Ready(Result<Payload, ClipboardError>),
        Abandoned,
    }
    struct Mailbox(Mutex<Delivery>);
    impl Mailbox {
        fn finish(&self, result: Result<Payload, ClipboardError>) {
            let previous = {
                let mut slot = self.0.lock().expect("clipboard reply");
                if matches!(*slot, Delivery::Abandoned) {
                    return;
                }
                std::mem::replace(&mut *slot, Delivery::Ready(result))
            };
            if let Delivery::Pending(Some(waker)) = previous {
                waker.wake();
            }
        }
    }
    struct Shared {
        busy: AtomicBool,
        closed: AtomicBool,
    }
    struct Job {
        command: Command,
        reply: Arc<Mailbox>,
    }
    pub(super) struct Desktop {
        sender: SyncSender<Option<Job>>,
        shared: Arc<Shared>,
    }
    impl Desktop {
        pub fn new() -> Result<Self, ClipboardError> {
            let (sender, receiver) = mpsc::sync_channel::<Option<Job>>(1);
            let shared = Arc::new(Shared {
                busy: AtomicBool::new(false),
                closed: AtomicBool::new(false),
            });
            let worker = shared.clone();
            std::thread::Builder::new()
                .name("glory-clipboard".into())
                .spawn(move || {
                    let mut clipboard = None;
                    while let Ok(Some(job)) = receiver.recv() {
                        let result = if worker.closed.load(Ordering::Acquire) {
                            Err(ClipboardError::Unavailable)
                        } else {
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                if clipboard.is_none() {
                                    clipboard = Some(
                                        arboard::Clipboard::new()
                                            .map_err(|_| ClipboardError::Unavailable)?,
                                    );
                                }
                                let native = clipboard.as_mut().expect("initialized clipboard");
                                match job.command {
                                    Command::Write(text) => native
                                        .set_text(text)
                                        .map(|()| Payload::Written)
                                        .map_err(|_| ClipboardError::Failed),
                                    Command::Read(limit) => {
                                        // Arboard/OS allocation precedes this returned String.
                                        // Bound before normalization or handing a body to the UI.
                                        let text = native
                                            .get_text()
                                            .map_err(|_| ClipboardError::Unavailable)?;
                                        if text.len() > limit {
                                            return Err(ClipboardError::TooLong);
                                        }
                                        let canonical = glory_render::normalize_text(&text, true);
                                        if canonical.len() > limit {
                                            return Err(ClipboardError::TooLong);
                                        }
                                        Ok(Payload::Text(canonical.into_owned()))
                                    }
                                }
                            }))
                            .unwrap_or_else(|_| {
                                clipboard = None;
                                Err(ClipboardError::Failed)
                            })
                        };
                        worker.busy.store(false, Ordering::Release);
                        job.reply.finish(result);
                        if worker.closed.load(Ordering::Acquire) {
                            break;
                        }
                    }
                })
                .map_err(|_| ClipboardError::Unavailable)?;
            Ok(Self { sender, shared })
        }
        fn request(&self, command: Command) -> Ticket {
            Ticket {
                command: Some(command),
                sender: self.sender.clone(),
                shared: self.shared.clone(),
                reply: Arc::new(Mailbox(Mutex::new(Delivery::Pending(None)))),
                done: false,
            }
        }
    }
    impl Drop for Desktop {
        fn drop(&mut self) {
            self.shared.closed.store(true, Ordering::Release);
            // Wake an idle worker; a running OS call remains non-abortable.
            let _ = self.sender.try_send(None);
        }
    }
    struct Ticket {
        command: Option<Command>,
        sender: SyncSender<Option<Job>>,
        shared: Arc<Shared>,
        reply: Arc<Mailbox>,
        done: bool,
    }
    impl Future for Ticket {
        type Output = Result<Payload, ClipboardError>;
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.done {
                return Poll::Ready(Err(ClipboardError::Failed));
            }
            if self.command.is_some() {
                let failure = if self.shared.closed.load(Ordering::Acquire) {
                    Some(ClipboardError::Unavailable)
                } else if self.shared.busy.swap(true, Ordering::AcqRel) {
                    Some(ClipboardError::Busy)
                } else {
                    None
                };
                if let Some(error) = failure {
                    self.done = true;
                    self.command = None;
                    return Poll::Ready(Err(error));
                }
                let job = Job {
                    command: self.command.take().unwrap(),
                    reply: self.reply.clone(),
                };
                if self.sender.try_send(Some(job)).is_err() {
                    self.shared.busy.store(false, Ordering::Release);
                    self.done = true;
                    return Poll::Ready(Err(ClipboardError::Unavailable));
                }
            }
            let next_waker = cx.waker().clone();
            let (result, previous) = {
                let mut slot = self.reply.0.lock().expect("clipboard reply");
                match &mut *slot {
                    Delivery::Pending(waker) => (None, waker.replace(next_waker)),
                    Delivery::Ready(_) => (
                        Some(std::mem::replace(&mut *slot, Delivery::Abandoned)),
                        None,
                    ),
                    Delivery::Abandoned => (Some(Delivery::Abandoned), None),
                }
            };
            drop(previous);
            match result {
                Some(Delivery::Ready(value)) => {
                    self.done = true;
                    Poll::Ready(value)
                }
                Some(_) => {
                    self.done = true;
                    Poll::Ready(Err(ClipboardError::Unavailable))
                }
                None => Poll::Pending,
            }
        }
    }
    impl Drop for Ticket {
        fn drop(&mut self) {
            let previous = std::mem::replace(
                &mut *self.reply.0.lock().expect("clipboard reply"),
                Delivery::Abandoned,
            );
            drop(previous);
            // The worker, rather than this future, owns the physical permit.
        }
    }
    impl ClipboardDriver for Desktop {
        fn write(&self, text: String) -> Reply<()> {
            if text.len() > MAX_CLIPBOARD_BYTES {
                return Box::pin(std::future::ready(Err(ClipboardError::TooLong)));
            }
            let ticket = self.request(Command::Write(text));
            Box::pin(async move {
                match ticket.await? {
                    Payload::Written => Ok(()),
                    _ => Err(ClipboardError::Failed),
                }
            })
        }
        fn read(&self, limit: usize) -> Reply<String> {
            if limit == 0 || limit > MAX_CLIPBOARD_BYTES {
                return Box::pin(std::future::ready(Err(ClipboardError::InvalidLimits)));
            }
            let ticket = self.request(Command::Read(limit));
            Box::pin(async move {
                match ticket.await? {
                    Payload::Text(text) => Ok(text),
                    _ => Err(ClipboardError::Failed),
                }
            })
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        fn fixture() -> (Desktop, mpsc::Receiver<Option<Job>>) {
            let (sender, receiver) = mpsc::sync_channel(1);
            (
                Desktop {
                    sender,
                    shared: Arc::new(Shared {
                        busy: AtomicBool::new(false),
                        closed: AtomicBool::new(false),
                    }),
                },
                receiver,
            )
        }
        #[test]
        fn desktop_tickets_are_lazy_cancel_receipts_and_keep_the_native_permit() {
            let (driver, receiver) = fixture();
            let mut first = driver.read(32);
            assert!(receiver.try_recv().is_err());
            let mut cx = Context::from_waker(Waker::noop());
            assert!(first.as_mut().poll(&mut cx).is_pending());
            let job = receiver.try_recv().unwrap().unwrap();
            drop(first);
            assert!(driver.shared.busy.load(Ordering::Acquire));
            assert!(matches!(*job.reply.0.lock().unwrap(), Delivery::Abandoned));
            for _ in 0..100 {
                let mut next = driver.write("bounded".into());
                assert!(matches!(
                    next.as_mut().poll(&mut cx),
                    Poll::Ready(Err(ClipboardError::Busy))
                ));
            }
            assert!(receiver.try_recv().is_err());
            // The actual worker owns this release, even after the receipt drops.
            driver.shared.busy.store(false, Ordering::Release);
            job.reply.finish(Ok(Payload::Text("discarded".into())));
            assert!(matches!(*job.reply.0.lock().unwrap(), Delivery::Abandoned));
            let mut next = driver.write("new".into());
            assert!(next.as_mut().poll(&mut cx).is_pending());
            let job = receiver.try_recv().unwrap().unwrap();
            driver.shared.busy.store(false, Ordering::Release);
            job.reply.finish(Ok(Payload::Written));
            assert!(matches!(next.as_mut().poll(&mut cx), Poll::Ready(Ok(()))));
        }
        #[test]
        fn desktop_bounds_and_close_fail_without_admitting_an_os_job() {
            let (driver, receiver) = fixture();
            let mut cx = Context::from_waker(Waker::noop());
            for limit in [0, MAX_CLIPBOARD_BYTES + 1] {
                assert!(matches!(
                    driver.read(limit).as_mut().poll(&mut cx),
                    Poll::Ready(Err(ClipboardError::InvalidLimits))
                ));
            }
            assert!(matches!(
                driver
                    .write("x".repeat(MAX_CLIPBOARD_BYTES + 1))
                    .as_mut()
                    .poll(&mut cx),
                Poll::Ready(Err(ClipboardError::TooLong))
            ));
            assert!(receiver.try_recv().is_err());
            let mut future = driver.read(8);
            drop(driver);
            assert!(receiver.try_recv().unwrap().is_none());
            assert!(matches!(
                future.as_mut().poll(&mut cx),
                Poll::Ready(Err(ClipboardError::Unavailable))
            ));
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use super::*;
    use std::{
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
        time::Instant,
    };
    struct Signal(std::thread::Thread);
    impl Wake for Signal {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    fn wait<T>(mut future: Reply<T>) -> Result<T, ClipboardError> {
        let waker = Waker::from(Arc::new(Signal(std::thread::current())));
        let mut context = Context::from_waker(&waker);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Poll::Ready(result) = future.as_mut().poll(&mut context) {
                return result;
            }
            assert!(
                Instant::now() < deadline,
                "OS clipboard worker did not settle"
            );
            std::thread::park_timeout(Duration::from_millis(20));
        }
    }
    #[test]
    #[ignore = "requires scripts/check-macos-clipboard.sh to preserve the complete pasteboard"]
    fn guarded_system_clipboard_round_trip() {
        let token = std::env::var("GLORY_CLIPBOARD_TEST_TOKEN").expect("run the pasteboard guard");
        assert!(token.starts_with("glory-owned-clipboard-") && token.len() > 40);
        let service = Clipboard::desktop().unwrap();
        let text = format!("{token}:中😀\nsecond line");
        wait(service.driver.write(text.clone())).unwrap();
        assert!(
            wait(service.driver.read(glory_render::MAX_CLIPBOARD_BYTES)).unwrap() == text,
            "owned clipboard content changed during the system test"
        );
        assert!(matches!(
            wait(service.driver.read(8)),
            Err(ClipboardError::TooLong)
        ));
        // A declaration's semantic CopyText uses this same worker.
        let runtime = glory_render::RuntimeId::new();
        let mut renderer = crate::Renderer::with_config(
            runtime,
            crate::Config {
                clipboard: Some(service.clone()),
                ..Default::default()
            },
        );
        let node = glory_render::NodeIds::new(runtime).allocate();
        let mut descriptor = glory_render::Descriptor {
            props: glory_render::Props::Button(Default::default()),
            ..Default::default()
        };
        descriptor.semantics = Some(Box::new(glory_render::Semantics {
            clipboard: Some(glory_render::ClipboardText {
                text: text.clone(),
                revision: 0,
            }),
            ..Default::default()
        }));
        use glory_render::Render;
        renderer
            .apply(&[
                glory_render::Mutation::Create {
                    id: node,
                    node: Box::new(descriptor),
                },
                glory_render::Mutation::Place {
                    id: node,
                    parent: None,
                    before: None,
                },
            ])
            .unwrap();
        renderer
            .action(glory_render::Action::CopyText {
                node,
                request: 1,
                revision: 0,
            })
            .unwrap();
        let mut events = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(10);
        while events.is_empty() {
            renderer.drain(&mut events);
            assert!(
                Instant::now() < deadline,
                "semantic clipboard result missing"
            );
            if events.is_empty() {
                std::thread::park_timeout(Duration::from_millis(10));
            }
        }
        assert!(matches!(
            &events[0],
            glory_render::Input::Event {
                event: glory_render::Event::Clipboard {
                    outcome: glory_render::ClipboardOutcome::Written,
                    ..
                },
                ..
            }
        ));
        assert!(
            wait(service.driver.read(glory_render::MAX_CLIPBOARD_BYTES)).unwrap() == text,
            "owned clipboard content changed during the semantic test"
        );
    }
}
