use glory_render::{memory::Memory, *};
use glory_ui::App;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
struct Wire {
    document: FileDocumentId,
    events: RefCell<Vec<Input>>,
    actions: RefCell<Vec<Action>>,
    reject: Cell<bool>,
}
impl Wire {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            document: FileDocumentId::new().unwrap(),
            events: RefCell::default(),
            actions: RefCell::default(),
            reject: Cell::new(false),
        })
    }
}
struct Host {
    tree: Memory,
    wire: Rc<Wire>,
}
impl Render for Host {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            files: Some(FileCapabilities {
                document: self.wire.document,
                picker: true,
                drop: true,
                read: false,
                preview: false,
            }),
            ..Default::default()
        }
    }
    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, Error> {
        self.tree.apply(mutations)
    }
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        self.tree.request(query, token)
    }
    fn action(&mut self, action: Action) -> Result<(), Error> {
        self.tree.admit_file_action(action)?;
        self.wire.actions.borrow_mut().push(action);
        if self.wire.reject.get() {
            Err(Error::Unsupported("test file policy"))
        } else {
            Ok(())
        }
    }
    fn drain(&mut self, into: &mut Vec<Input>) {
        into.extend(self.wire.events.borrow_mut().drain(..));
        self.tree.drain(into);
    }
}
fn selected(
    node: NodeId,
    document: FileDocumentId,
    request: u64,
    batch: u64,
    revision: u64,
) -> Event {
    Event::Files(Box::new(FileEvent {
        document,
        request: Some(request),
        batch,
        revision,
        origin: FileOrigin::Picker,
        outcome: FileOutcome::Selected(vec![FileInfo {
            token: FileToken {
                document,
                node,
                revision,
                serial: batch,
            },
            name: "hello.txt".into(),
            media_type: "text/plain".into(),
            size: 5,
        }]),
    }))
}
#[test]
fn host_file_results_are_first_only_document_scoped_revision_checked_and_not_dispatchable() {
    let wire = Wire::new();
    let host = wire.clone();
    let app = App::new(move |runtime| Host {
        tree: Memory::new(runtime),
        wire: host,
    });
    let scope = app.scope().child().unwrap();
    let id = scope
        .create(Descriptor::new(Props::File(FileProps::default())))
        .unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let output = calls.clone();
    scope
        .on(id, EventKind::Files, move |event| {
            output.borrow_mut().push(event)
        })
        .unwrap();
    app.flush().unwrap();
    assert_eq!(scope.pick_files(id).unwrap(), 1);
    app.flush().unwrap();
    let first = selected(id, wire.document, 1, 50, 0);
    assert!(app.dispatch(id, first.clone()).is_err());
    wire.events.borrow_mut().extend([
        Input::Event {
            node: id,
            event: first.clone(),
        },
        Input::Event {
            node: id,
            event: first.clone(),
        },
    ]);
    app.pump().unwrap();
    assert_eq!(calls.borrow().len(), 1);
    let Event::Files(first) = first else { panic!() };
    let FileOutcome::Selected(files) = first.outcome else {
        panic!()
    };
    let token = files[0].token;
    assert!(app.tree().selected_file(token).is_ok());
    assert_eq!(scope.pick_files(id).unwrap(), 2);
    app.flush().unwrap();
    wire.events.borrow_mut().extend([
        Input::Event {
            node: id,
            event: selected(id, FileDocumentId::new().unwrap(), 2, 51, 0),
        },
        Input::Event {
            node: id,
            event: selected(id, wire.document, 99, 52, 0),
        },
        Input::Event {
            node: id,
            event: selected(id, wire.document, 2, 53, 0),
        },
    ]);
    assert!(app.pump().is_err());
    assert_eq!(calls.borrow().len(), 2);
    assert!(app.tree().selected_file(token).is_err());
    assert_eq!(scope.pick_files(id).unwrap(), 3);
    scope
        .patch(
            id,
            Patch::File {
                configuration: Change::Set(Box::new(FileConfig {
                    filters: vec![FileFilter::Mime("image/png".into())],
                    ..Default::default()
                })),
                revision: 1,
            },
        )
        .unwrap();
    app.flush().unwrap();
    assert_eq!(
        wire.actions.borrow().len(),
        2,
        "retired picker is never sent to host"
    );
    wire.events.borrow_mut().push(Input::Event {
        node: id,
        event: selected(id, wire.document, 3, 54, 0),
    });
    app.pump().unwrap();
    assert_eq!(calls.borrow().len(), 2);
    drop(scope);
    app.flush().unwrap();
    wire.events.borrow_mut().push(Input::Event {
        node: id,
        event: selected(id, wire.document, 3, 55, 0),
    });
    app.pump().unwrap();
    assert_eq!(calls.borrow().len(), 2);
}
#[test]
fn queued_disable_and_platform_refusal_finish_pending_even_after_a_larger_previous_batch() {
    let wire = Wire::new();
    let host = wire.clone();
    let app = App::new(move |runtime| Host {
        tree: Memory::new(runtime),
        wire: host,
    });
    let parent = app.scope().create(Descriptor::new(Props::View)).unwrap();
    let scope = app.scope().child_in(parent).unwrap();
    let id = scope
        .create(Descriptor::new(Props::File(FileProps::default())))
        .unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let output = calls.clone();
    scope
        .on(id, EventKind::Files, move |event| {
            output.borrow_mut().push(event)
        })
        .unwrap();
    app.flush().unwrap();
    scope.pick_files(id).unwrap();
    app.flush().unwrap();
    wire.events.borrow_mut().push(Input::Event {
        node: id,
        event: selected(id, wire.document, 1, 50, 0),
    });
    app.pump().unwrap();
    assert_eq!(scope.pick_files(id).unwrap(), 2);
    scope
        .patch(
            parent,
            Patch::Semantics(Change::Set(Some(Box::new(Semantics {
                inert: true,
                ..Default::default()
            })))),
        )
        .unwrap();
    app.flush().unwrap();
    app.settle(3).unwrap();
    assert_eq!(calls.borrow().len(), 2);
    assert!(!app.tree().file_pending(id, 2, 0).unwrap());
    assert_eq!(wire.actions.borrow().len(), 1);
    scope
        .patch(parent, Patch::Semantics(Change::Clear))
        .unwrap();
    app.flush().unwrap();
    wire.reject.set(true);
    assert_eq!(scope.pick_files(id).unwrap(), 3);
    app.flush().unwrap();
    app.settle(3).unwrap();
    assert_eq!(calls.borrow().len(), 3);
    assert!(!app.tree().file_pending(id, 3, 0).unwrap());
    for event in calls.borrow().iter().skip(1) {
        assert!(
            matches!(event,Event::Files(result) if result.outcome==FileOutcome::Rejected(FileRejection::Failed))
        );
    }
}
#[test]
fn unavailable_files_reject_before_request_watermark_queue_or_capability_io() {
    let app = App::memory();
    let id = app
        .scope()
        .create(Descriptor::new(Props::File(FileProps::default())))
        .unwrap();
    app.flush().unwrap();
    let before = app.stats();
    assert!(app.scope().pick_files(id).is_err());
    assert_eq!(app.stats(), before);
    assert_eq!(app.tree().next_file_request(id).unwrap(), 1);
    assert!(matches!(
        app.scope().action(Action::PickFiles {
            node: id,
            revision: 1,
            request: 1
        }),
        Err(glory_ui::Error::Render(Error::InputRevision { .. }))
    ));
    assert_eq!(app.tree().next_file_request(id).unwrap(), 1);
}
