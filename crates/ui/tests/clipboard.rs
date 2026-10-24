use glory_render::{memory::Memory, *};
use glory_ui::{App, button, view};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[derive(Default)]
struct Wire {
    events: RefCell<Vec<Input>>,
    actions: RefCell<Vec<Action>>,
    reject: Cell<bool>,
}
struct Host {
    tree: Memory,
    wire: Rc<Wire>,
}
impl Render for Host {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            clipboard_write: true,
            files: None,
            ..Default::default()
        }
    }
    fn apply(&mut self, m: &[Mutation]) -> Result<Receipt, Error> {
        self.tree.apply(m)
    }
    fn request(&mut self, q: Query, t: QueryToken) -> Result<(), Error> {
        self.tree.request(q, t)
    }
    fn action(&mut self, a: Action) -> Result<(), Error> {
        self.tree.admit_clipboard(a)?;
        self.wire.actions.borrow_mut().push(a);
        if self.wire.reject.get() {
            Err(Error::Unsupported("test policy"))
        } else {
            Ok(())
        }
    }
    fn drain(&mut self, into: &mut Vec<Input>) {
        into.extend(self.wire.events.borrow_mut().drain(..));
        self.tree.drain(into)
    }
}
fn metadata(text: &str, revision: u64) -> Option<Box<Semantics>> {
    Some(Box::new(Semantics {
        clipboard: Some(ClipboardText {
            text: text.into(),
            revision,
        }),
        ..Default::default()
    }))
}
fn event(node: NodeId, request: u64, revision: u64) -> Input {
    Input::Event {
        node,
        event: Event::Clipboard {
            request,
            revision,
            outcome: ClipboardOutcome::Written,
        },
    }
}
#[test]
fn physical_results_are_first_only_and_forged_unknown_stale_events_cannot_succeed() {
    let wire = Rc::new(Wire::default());
    let host = wire.clone();
    let app = App::new(move |runtime| Host {
        tree: Memory::new(runtime),
        wire: host,
    });
    let node = app.scope().cage(None);
    let calls = Rc::new(RefCell::new(vec![]));
    let output = calls.clone();
    app.mount(
        button()
            .semantics(metadata("one", 0))
            .node_ref(node)
            .on(EventKind::Clipboard, move |event| {
                output.borrow_mut().push(event)
            }),
    )
    .unwrap();
    let id = node.get().unwrap();
    assert_eq!(app.scope().copy_text(id).unwrap(), 1);
    assert!(wire.actions.borrow().is_empty());
    app.flush().unwrap();
    assert_eq!(wire.actions.borrow().len(), 1);
    assert!(
        app.dispatch(
            id,
            Event::Clipboard {
                request: 1,
                revision: 0,
                outcome: ClipboardOutcome::Written
            }
        )
        .is_err()
    );
    assert!(calls.borrow().is_empty());
    wire.events
        .borrow_mut()
        .extend([event(id, 1, 0), event(id, 1, 0)]);
    app.pump().unwrap();
    assert_eq!(calls.borrow().len(), 1);
    assert!(
        app.scope()
            .action(Action::CopyText {
                node: id,
                revision: 0,
                request: 1
            })
            .is_err()
    );
    assert_eq!(app.scope().copy_text(id).unwrap(), 2);
    app.flush().unwrap();
    wire.events
        .borrow_mut()
        .extend([event(id, 99, 0), event(id, 2, 0)]);
    assert!(app.pump().is_err());
    assert_eq!(calls.borrow().len(), 2);
    assert_eq!(app.scope().copy_text(id).unwrap(), 3);
    app.scope()
        .patch(id, Patch::Semantics(Change::Set(metadata("two", 1))))
        .unwrap();
    app.flush().unwrap();
    assert_eq!(wire.actions.borrow().len(), 2);
    wire.events.borrow_mut().push(event(id, 3, 0));
    app.pump().unwrap();
    assert_eq!(calls.borrow().len(), 2);
    assert_eq!(app.scope().copy_text(id).unwrap(), 4);
    app.clear().unwrap();
    wire.events.borrow_mut().push(event(id, 4, 1));
    app.pump().unwrap();
    assert_eq!(calls.borrow().len(), 2);
}
#[test]
fn queued_disable_and_host_refusal_deliver_failure_on_next_turn_without_fault_or_busy_leak() {
    let wire = Rc::new(Wire::default());
    let host = wire.clone();
    let app = App::new(move |runtime| Host {
        tree: Memory::new(runtime),
        wire: host,
    });
    let parent = app.scope().cage(None);
    let node = app.scope().cage(None);
    let calls = Rc::new(RefCell::new(vec![]));
    let output = calls.clone();
    app.mount(
        view().node_ref(parent).fill(
            button()
                .semantics(metadata("x", 0))
                .node_ref(node)
                .on(EventKind::Clipboard, move |event| {
                    output.borrow_mut().push(event)
                }),
        ),
    )
    .unwrap();
    let id = node.get().unwrap();
    app.scope().copy_text(id).unwrap();
    app.scope()
        .patch(
            parent.get().unwrap(),
            Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                inert: true,
                ..Default::default()
            })))),
        )
        .unwrap();
    app.flush().unwrap();
    assert!(calls.borrow().is_empty());
    assert!(wire.actions.borrow().is_empty());
    app.settle(2).unwrap();
    assert!(matches!(
        calls.borrow()[0],
        Event::Clipboard {
            outcome: ClipboardOutcome::Failed,
            ..
        }
    ));
    app.scope()
        .patch(parent.get().unwrap(), Patch::Semantics(Change::Clear))
        .unwrap();
    app.flush().unwrap();
    wire.reject.set(true);
    app.scope().copy_text(id).unwrap();
    app.flush().unwrap();
    assert_eq!(calls.borrow().len(), 1);
    app.settle(2).unwrap();
    assert_eq!(calls.borrow().len(), 2);
    wire.reject.set(false);
    let request = app.scope().copy_text(id).unwrap();
    app.flush().unwrap();
    wire.events.borrow_mut().push(event(id, request, 0));
    app.pump().unwrap();
    assert_eq!(calls.borrow().len(), 3);
}
#[test]
fn unsupported_capability_rejects_before_queue_or_counter_and_source_validation_precedes_it() {
    let app = App::memory();
    let node = app.scope().cage(None);
    app.mount(button().node_ref(node).semantics(metadata("x", 0)))
        .unwrap();
    let id = node.get().unwrap();
    let before = app.stats();
    assert!(app.scope().copy_text(id).is_err());
    assert_eq!(app.stats(), before);
    assert_eq!(app.tree().next_clipboard_request(id), Ok(1));
    assert!(
        app.scope()
            .action(Action::CopyText {
                node: id,
                revision: 1,
                request: 1
            })
            .is_err()
    );
    assert_eq!(app.tree().next_clipboard_request(id), Ok(1));
}
