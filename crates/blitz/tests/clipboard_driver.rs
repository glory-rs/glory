#![cfg(all(feature = "clipboard", not(target_arch = "wasm32")))]
use glory_blitz::{
    Config, Renderer,
    clipboard::{Clipboard, ClipboardDriver, ClipboardError, Reply},
};
use glory_core::Cage;
use glory_render::*;
use glory_ui::{App, Widget, button};
use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    time::Duration,
};

#[derive(Default)]
struct Gate {
    result: Option<Result<(), ClipboardError>>,
    waker: Option<Waker>,
}
struct Wait(Arc<Mutex<Gate>>);
impl Future for Wait {
    type Output = Result<(), ClipboardError>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let next = cx.waker().clone();
        let mut gate = self.0.lock().unwrap();
        if let Some(result) = gate.result.take() {
            return Poll::Ready(result);
        }
        let old = gate.waker.replace(next);
        drop(gate);
        drop(old);
        Poll::Pending
    }
}
impl Drop for Wait {
    fn drop(&mut self) {
        let waker = self.0.lock().unwrap().waker.take();
        drop(waker);
    }
}
#[derive(Default)]
struct Driver {
    calls: Mutex<Vec<(String, Arc<Mutex<Gate>>)>>,
}
impl ClipboardDriver for Driver {
    fn write(&self, text: String) -> Reply<()> {
        let gate = Arc::new(Mutex::new(Gate::default()));
        self.calls.lock().unwrap().push((text, gate.clone()));
        Box::pin(Wait(gate))
    }
}
impl Driver {
    fn len(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
    fn finish(&self, index: usize, result: Result<(), ClipboardError>) {
        let gate = self.calls.lock().unwrap()[index].1.clone();
        let wake = {
            let mut g = gate.lock().unwrap();
            g.result = Some(result);
            g.waker.take()
        };
        if let Some(w) = wake {
            w.wake()
        }
    }
}
struct Fixture {
    app: App,
    renderer: Renderer,
    driver: Arc<Driver>,
    id: NodeId,
    source: Cage<ClipboardText>,
    events: Rc<RefCell<Vec<Event>>>,
}
fn fixture(timeout: Duration) -> Fixture {
    let driver = Arc::new(Driver::default());
    let mut renderer = None;
    let service = Clipboard::with_timeout(driver.clone(), timeout).unwrap();
    let app = App::new(|runtime| {
        let r = Renderer::with_config(
            runtime,
            Config {
                clipboard: Some(service),
                ..Default::default()
            },
        );
        renderer = Some(r.clone());
        r
    });
    let scope = app.scope();
    let reference = scope.cage(None);
    let source = scope.cage(ClipboardText {
        text: "private source\n中文😀".into(),
        revision: 0,
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let record = events.clone();
    app.mount(
        button()
            .label("Copy")
            .node_ref(reference)
            .semantics(scope.bond(move || {
                Some(Box::new(Semantics {
                    clipboard: Some(source.get()),
                    ..Default::default()
                }))
            }))
            .on(EventKind::Clipboard, move |event| {
                record.borrow_mut().push(event)
            }),
    )
    .unwrap();
    Fixture {
        app,
        renderer: renderer.unwrap(),
        driver,
        id: reference.get().unwrap(),
        source,
        events,
    }
}
fn start(f: &Fixture) -> u64 {
    let request = f.app.scope().copy_text(f.id).unwrap();
    f.app.flush().unwrap();
    request
}

#[test]
fn explicit_native_clipboard_is_lazy_and_completes_the_exact_current_request_once() {
    let f = fixture(Duration::from_secs(30));
    assert!(f.renderer.capabilities().clipboard_write);
    assert!(
        f.renderer
            .clone()
            .action(Action::CopyText {
                node: f.id,
                request: 1,
                revision: 1
            })
            .is_err()
    );
    assert_eq!(f.driver.len(), 0);
    let request = start(&f);
    assert_eq!(request, 1);
    assert_eq!(f.driver.len(), 0, "action admission must not call a driver");
    f.app.pump().unwrap();
    assert_eq!(f.driver.len(), 1);
    assert!(f.events.borrow().is_empty());
    assert_eq!(
        f.driver.calls.lock().unwrap()[0].0,
        "private source\n中文😀"
    );
    f.driver.finish(0, Ok(()));
    assert!(f.renderer.needs_observe());
    f.app.pump().unwrap();
    assert_eq!(
        *f.events.borrow(),
        [Event::Clipboard {
            request,
            revision: 0,
            outcome: ClipboardOutcome::Written
        }]
    );
    f.app.pump().unwrap();
    assert_eq!(f.events.borrow().len(), 1);
    assert!(!f.renderer.needs_observe());
    assert_eq!(f.app.tree().next_clipboard_request(f.id), Ok(2));
}

#[test]
fn native_clipboard_source_replacement_before_poll_never_starts_the_os_write() {
    let f = fixture(Duration::from_secs(30));
    start(&f);
    f.source.revise(|s| {
        s.text = "replacement".into();
        s.revision += 1
    });
    f.app.flush().unwrap();
    f.app.pump().unwrap();
    assert_eq!(f.driver.len(), 0);
    assert!(f.events.borrow().is_empty());
    let request = start(&f);
    f.app.pump().unwrap();
    assert_eq!(request, 2);
    assert_eq!(f.driver.len(), 1);
    f.driver.finish(0, Ok(()));
    f.app.pump().unwrap();
    assert_eq!(
        *f.events.borrow(),
        [Event::Clipboard {
            request: 2,
            revision: 1,
            outcome: ClipboardOutcome::Written
        }]
    );
}

#[test]
fn native_clipboard_retirement_keeps_the_physical_permit_until_completion_and_drops_late_results() {
    let f = fixture(Duration::from_secs(30));
    start(&f);
    f.app.pump().unwrap();
    assert_eq!(f.driver.len(), 1);
    f.source.revise(|s| {
        s.text = "new source".into();
        s.revision += 1
    });
    f.app.flush().unwrap();
    let second = start(&f);
    f.app.pump().unwrap();
    assert_eq!(second, 2);
    assert_eq!(f.driver.len(), 1);
    assert_eq!(
        *f.events.borrow(),
        [Event::Clipboard {
            request: 2,
            revision: 1,
            outcome: ClipboardOutcome::Failed
        }]
    );
    f.driver.finish(0, Ok(()));
    f.app.pump().unwrap();
    assert_eq!(f.events.borrow().len(), 1);
    assert_eq!(start(&f), 3);
    f.app.pump().unwrap();
    assert_eq!(f.driver.len(), 2);
    f.app.clear().unwrap();
    f.driver.finish(1, Ok(()));
    f.app.pump().unwrap();
    assert_eq!(f.events.borrow().len(), 1);
    assert_eq!(f.renderer.stats().nodes, 0);
}

#[test]
fn native_clipboard_outcomes_and_concurrent_sources_do_not_create_a_native_queue() {
    for (error, expected) in [
        (ClipboardError::Denied, ClipboardOutcome::Denied),
        (ClipboardError::Unavailable, ClipboardOutcome::Unavailable),
        (ClipboardError::Failed, ClipboardOutcome::Failed),
    ] {
        let f = fixture(Duration::from_secs(30));
        start(&f);
        f.app.pump().unwrap();
        let node = f.app.scope().cage(None);
        let extra = f.app.scope().child().unwrap();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let output = seen.clone();
        button()
            .node_ref(node)
            .semantics(Some(Box::new(Semantics {
                clipboard: Some(ClipboardText {
                    text: "second".into(),
                    revision: 0,
                }),
                ..Default::default()
            })))
            .on(EventKind::Clipboard, move |event| {
                output.borrow_mut().push(event)
            })
            .show_in(&extra)
            .unwrap();
        f.app.flush().unwrap();
        for _ in 0..20 {
            f.app.scope().copy_text(node.get().unwrap()).unwrap();
            f.app.flush().unwrap();
            f.app.pump().unwrap();
        }
        assert_eq!(f.driver.len(), 1);
        assert_eq!(seen.borrow().len(), 20);
        assert!(seen.borrow().iter().all(|e| matches!(
            e,
            Event::Clipboard {
                outcome: ClipboardOutcome::Failed,
                ..
            }
        )));
        f.driver.finish(0, Err(error));
        f.app.pump().unwrap();
        assert_eq!(
            *f.events.borrow(),
            [Event::Clipboard {
                request: 1,
                revision: 0,
                outcome: expected
            }]
        );
    }
}

#[test]
fn native_clipboard_deadline_reports_failure_without_refunding_an_unsettled_os_write() {
    let f = fixture(Duration::from_millis(10));
    start(&f);
    f.app.pump().unwrap();
    assert_eq!(f.driver.len(), 1);
    std::thread::sleep(Duration::from_millis(30));
    f.app.pump().unwrap();
    assert_eq!(
        *f.events.borrow(),
        [Event::Clipboard {
            request: 1,
            revision: 0,
            outcome: ClipboardOutcome::Failed
        }]
    );
    start(&f);
    f.app.pump().unwrap();
    assert_eq!(f.driver.len(), 1);
    assert_eq!(f.events.borrow().len(), 2);
    f.driver.finish(0, Ok(()));
    f.app.pump().unwrap();
    assert_eq!(f.events.borrow().len(), 2);
}

#[test]
fn native_clipboard_disabled_sources_retired_identities_and_invalid_timeouts_do_not_write() {
    let f = fixture(Duration::from_secs(30));
    start(&f);
    f.app
        .scope()
        .patch(f.id, Patch::Enabled(Change::Set(false)))
        .unwrap();
    f.app.flush().unwrap();
    f.app.pump().unwrap();
    assert_eq!(f.driver.len(), 0);
    assert!(
        f.renderer
            .clone()
            .action(Action::CopyText {
                node: f.id,
                request: 2,
                revision: 0
            })
            .is_err()
    );
    f.app.clear().unwrap();
    assert!(matches!(
        f.renderer.clone().action(Action::CopyText {
            node: f.id,
            request: 2,
            revision: 0
        }),
        Err(Error::NodeGone(_))
    ));
    assert!(Clipboard::with_timeout(f.driver.clone(), Duration::ZERO).is_err());
    assert!(Clipboard::with_timeout(f.driver, Duration::from_secs(121)).is_err());
}

#[test]
fn foreign_clipboard_construction_poll_and_drop_run_outside_the_document_borrow() {
    use std::cell::Cell;
    thread_local! {
        static DOCUMENT: RefCell<Option<Renderer>> = const { RefCell::new(None) };
        static DROPPED: Cell<bool> = const { Cell::new(false) };
    }
    fn inspect() {
        DOCUMENT.with(|r| assert!(r.borrow().as_ref().unwrap().stats().nodes > 0));
    }
    struct Done;
    impl Future for Done {
        type Output = Result<(), ClipboardError>;
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            inspect();
            Poll::Ready(Ok(()))
        }
    }
    impl Drop for Done {
        fn drop(&mut self) {
            inspect();
            DROPPED.set(true);
        }
    }
    struct Inspect;
    impl ClipboardDriver for Inspect {
        fn write(&self, _: String) -> Reply<()> {
            inspect();
            Box::pin(Done)
        }
    }
    let mut renderer = None;
    let app = App::new(|runtime| {
        let r = Renderer::with_config(
            runtime,
            Config {
                clipboard: Some(Clipboard::new(Arc::new(Inspect))),
                ..Default::default()
            },
        );
        renderer = Some(r.clone());
        r
    });
    let node = app.scope().cage(None);
    app.mount(button().node_ref(node).semantics(Some(Box::new(Semantics {
        clipboard: Some(ClipboardText {
            text: "owned test".into(),
            revision: 0,
        }),
        ..Default::default()
    }))))
    .unwrap();
    DOCUMENT.with(|r| *r.borrow_mut() = renderer);
    app.scope().copy_text(node.get().unwrap()).unwrap();
    app.flush().unwrap();
    app.pump().unwrap();
    assert!(DROPPED.get());
    DOCUMENT.with(|r| *r.borrow_mut() = None);
}

#[test]
fn native_clipboard_declarations_never_appear_in_native_dom_or_create_an_implicit_driver() {
    let f = fixture(Duration::from_secs(30));
    let physical = f.renderer.engine_id(f.id, glory_html::Part::Main).unwrap();
    f.renderer.inspect(|doc| {
        let node = doc.get_node(physical).unwrap();
        assert!(!format!("{node:?}").contains("private source"));
    });
    let plain = Renderer::new(RuntimeId::new());
    assert!(!plain.capabilities().clipboard_write);
}
