use super::*;
use crate::{
    files::{FileDrop, FilePicker, FileReader, Files, Reply},
    shell::WindowApp,
};
use glory_core::Cage;
use glory_render::FileConfig;
use std::{
    cell::RefCell,
    future::Future,
    io,
    path::PathBuf,
    rc::Rc,
    sync::{Mutex, atomic::AtomicUsize},
};
struct Gate<T> {
    value: Option<Result<T, FileRejection>>,
    waker: Option<Waker>,
}
impl<T> Default for Gate<T> {
    fn default() -> Self {
        Self {
            value: None,
            waker: None,
        }
    }
}
struct Wait<T>(Arc<Mutex<Gate<T>>>);
impl<T: Send + 'static> Future for Wait<T> {
    type Output = Result<T, FileRejection>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut gate = self.0.lock().unwrap();
        if let Some(value) = gate.value.take() {
            return Poll::Ready(value);
        }
        let old = gate.waker.replace(cx.waker().clone());
        drop(gate);
        drop(old);
        Poll::Pending
    }
}
fn complete<T>(gate: &Arc<Mutex<Gate<T>>>, value: Result<T, FileRejection>) {
    let wake = {
        let mut gate = gate.lock().unwrap();
        gate.value = Some(value);
        gate.waker.take()
    };
    if let Some(wake) = wake {
        wake.wake();
    }
}
type PickGate = Arc<Mutex<Gate<Option<Vec<SelectedFile>>>>>;
type OpenGate = Arc<Mutex<Gate<Vec<SelectedFile>>>>;
#[derive(Default)]
struct Driver {
    picks: Mutex<Vec<PickGate>>,
    opens: Mutex<Vec<(Vec<PathBuf>, OpenGate)>>,
}
impl FilePicker for Driver {
    fn pick(&self, _: FileConfig) -> Reply<Option<Vec<SelectedFile>>> {
        let gate = Arc::new(Mutex::new(Gate::default()));
        self.picks.lock().unwrap().push(gate.clone());
        Box::pin(Wait(gate))
    }
}
impl FileDrop for Driver {
    fn open(&self, paths: Vec<PathBuf>) -> Reply<Vec<SelectedFile>> {
        let gate = Arc::new(Mutex::new(Gate::default()));
        self.opens.lock().unwrap().push((paths, gate.clone()));
        Box::pin(Wait(gate))
    }
}
impl Driver {
    fn open_len(&self) -> usize {
        self.opens.lock().unwrap().len()
    }
    fn finish(&self, n: usize, files: Vec<SelectedFile>) {
        let gate = self.opens.lock().unwrap()[n].1.clone();
        complete(&gate, Ok(files));
    }
    fn finish_pick(&self, n: usize, files: Vec<SelectedFile>) {
        let gate = self.picks.lock().unwrap()[n].clone();
        complete(&gate, Ok(Some(files)));
    }
}
struct Reader;
impl FileReader for Reader {
    fn read(&self, _: u64, length: u32) -> Reply<Vec<u8>> {
        Box::pin(std::future::ready(Ok(vec![7; length as usize])))
    }
}
fn selected(name: &str) -> SelectedFile {
    SelectedFile {
        name: name.into(),
        media_type: "text/plain".into(),
        size: 4,
        reader: Arc::new(Reader),
    }
}
struct Fixture {
    host: WindowApp,
    driver: Arc<Driver>,
    files: Files,
    id: NodeId,
    position: PhysicalPosition<f64>,
    enabled: Cage<bool>,
    events: Rc<RefCell<Vec<FileEvent>>>,
}
fn with_service(files: Files, driver: Arc<Driver>, scale: f32) -> Fixture {
    let host = WindowApp::with_config(Config {
        files: Some(files.clone()),
        ..Default::default()
    });
    host.renderer().set_viewport(800, 600, scale).unwrap();
    host.renderer()
        .set_css("body{margin:0}button{width:180px;height:44px;margin-left:30px;margin-top:40px}");
    let scope = host.app().scope();
    let node = scope.cage(None);
    let enabled = scope.cage(true);
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    host.mount(
        glory_ui::file()
            .node_ref(node)
            .enabled(enabled)
            .configuration(FileConfig {
                drop: true,
                ..Default::default()
            })
            .on(EventKind::Files, move |event| {
                if let Event::Files(event) = event {
                    output.borrow_mut().push(*event);
                }
            }),
    )
    .unwrap();
    host.observe(&Context::from_waker(Waker::noop())).unwrap();
    let id = node.get().unwrap();
    let engine = host
        .renderer()
        .engine_id(id, glory_html::Part::Main)
        .unwrap();
    let rect = host
        .renderer()
        .inspect(|doc| doc.get_client_bounding_rect(engine).unwrap());
    assert!(
        rect.width > 0. && rect.height > 0. && rect.x >= 30. && rect.y >= 40.,
        "actual laid-out file target: {rect:?}"
    );
    let position = PhysicalPosition::new(
        (rect.x + rect.width / 2.) * f64::from(scale),
        (rect.y + rect.height / 2.) * f64::from(scale),
    );
    Fixture {
        host,
        driver,
        files,
        id,
        position,
        enabled,
        events,
    }
}
fn fixture(scale: f32) -> Fixture {
    let driver = Arc::new(Driver::default());
    with_service(
        Files::new(driver.clone()).with_drop_driver(driver.clone()),
        driver,
        scale,
    )
}
fn settle(f: &Fixture) {
    f.host.observe(&Context::from_waker(Waker::noop())).unwrap();
}
#[derive(Default)]
struct Api {
    fetches: RefCell<Vec<(DataTransferId, AsyncRequestSerial)>>,
    accepts: RefCell<Vec<bool>>,
}
impl Transfers for Api {
    fn has_uris(&self, _: DataTransferId) -> bool {
        true
    }
    fn accept_copy(&self, _: DataTransferId, accept: bool) {
        self.accepts.borrow_mut().push(accept);
    }
    fn fetch(&self, id: DataTransferId) -> Result<AsyncRequestSerial, FileRejection> {
        let serial = AsyncRequestSerial::get();
        self.fetches.borrow_mut().push((id, serial));
        Ok(serial)
    }
}
fn id(n: i64) -> DataTransferId {
    DataTransferId::from_raw(n)
}
fn begin(f: &Fixture, api: &Api, n: i64) -> AsyncRequestSerial {
    receive(
        f.host.renderer(),
        &WindowEvent::DragEntered {
            id: id(n),
            position: Some(f.position),
        },
        api,
    );
    receive(
        f.host.renderer(),
        &WindowEvent::DragDropped {
            id: id(n),
            proposed_action: Some(DndAction::Copy),
        },
        api,
    );
    api.fetches.borrow().last().unwrap().1
}
#[derive(Debug)]
struct Data {
    uris: Vec<String>,
    calls: Arc<AtomicUsize>,
    error: Option<io::ErrorKind>,
    before: Option<fn()>,
}
impl TypedData for Data {
    fn type_(&self) -> &dyn wingy::data_transfer::TransferType {
        &TypeHint::UriList
    }
    fn try_read(&self) -> Option<Box<dyn io::BufRead>> {
        panic!("URI transfer must not use an unbounded generic byte reader")
    }
    fn try_as_uris(&self) -> io::Result<Vec<String>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if let Some(before) = self.before {
            before();
        }
        if let Some(error) = self.error {
            Err(error.into())
        } else {
            Ok(self.uris.clone())
        }
    }
    fn try_as_string(&self) -> io::Result<String> {
        panic!("drop must not coerce text")
    }
}
fn data(uris: Vec<String>) -> Arc<Data> {
    Arc::new(Data {
        uris,
        calls: Arc::new(AtomicUsize::new(0)),
        error: None,
        before: None,
    })
}
fn uri() -> (String, PathBuf) {
    let path = std::env::temp_dir().join("native-drop-中文 😀.txt");
    (
        apace_traits::net::Url::from_file_path(&path)
            .unwrap()
            .to_string(),
        path,
    )
}
fn deliver(f: &Fixture, api: &Api, n: i64, serial: AsyncRequestSerial, data: Arc<Data>) {
    receive(
        f.host.renderer(),
        &WindowEvent::DataTransferReceived {
            id: id(n),
            serial,
            value: data,
        },
        api,
    );
}
fn file(f: &Fixture) -> FileToken {
    let events = f.events.borrow();
    let FileOutcome::Selected(files) = &events.last().unwrap().outcome else {
        panic!("selected")
    };
    files[0].token
}
#[test]
fn dpi_hit_testing_trusted_copy_drop_and_exact_receipts_produce_readable_file_tokens() {
    for scale in [1., 2.] {
        let f = fixture(scale);
        let api = Api::default();
        let raw = data(vec![uri().0]);
        assert!(f.host.renderer().capabilities().files.unwrap().drop);
        receive(
            f.host.renderer(),
            &WindowEvent::DragEntered {
                id: id(1),
                position: Some(f.position),
            },
            &api,
        );
        assert_eq!(api.accepts.borrow().as_slice(), &[true]);
        assert!(api.fetches.borrow().is_empty());
        assert_eq!(raw.calls.load(Ordering::Relaxed), 0);
        assert_eq!(f.driver.open_len(), 0);
        receive(
            f.host.renderer(),
            &WindowEvent::DragDropped {
                id: id(1),
                proposed_action: Some(DndAction::Copy),
            },
            &api,
        );
        let serial = api.fetches.borrow()[0].1;
        deliver(&f, &api, 2, serial, raw.clone());
        deliver(&f, &api, 1, AsyncRequestSerial::get(), raw.clone());
        assert_eq!(raw.calls.load(Ordering::Relaxed), 0);
        deliver(&f, &api, 1, serial, raw.clone());
        deliver(&f, &api, 1, serial, raw.clone());
        assert_eq!(raw.calls.load(Ordering::Relaxed), 1);
        assert_eq!(f.driver.open_len(), 0);
        settle(&f);
        assert_eq!(f.driver.opens.lock().unwrap()[0].0, vec![uri().1]);
        f.driver.finish(0, vec![selected("drop-中文.txt")]);
        settle(&f);
        let event = f.events.borrow().last().unwrap().clone();
        assert_eq!(event.origin, FileOrigin::Drop);
        assert!(event.request.is_none());
        let token = file(&f);
        assert!(f.host.app().tree().selected_file(token).is_ok());
        let result = f.host.app().scope().cage(None);
        f.host
            .app()
            .request(
                Query::FileRead(glory_render::FileRead {
                    file: token,
                    offset: 0,
                    length: 4,
                }),
                move |r| result.revise(|v| *v = Some(r)),
            )
            .unwrap();
        settle(&f);
        assert!(matches!(result.get(), Some(Ok(QueryValue::FileBytes(_)))));
        assert!(!f.files.stats().dropping);
    }
}
#[test]
fn rejected_uri_payloads_do_not_open_paths_or_replace_a_selected_grant() {
    let f = fixture(1.);
    let api = Api::default();
    let serial = begin(&f, &api, 1);
    deliver(&f, &api, 1, serial, data(vec![uri().0]));
    settle(&f);
    f.driver.finish(0, vec![selected("previous.txt")]);
    settle(&f);
    let previous = file(&f);
    for (n, uris, error) in [
        (
            2,
            vec!["https://example.test/file".into()],
            FileRejection::Type,
        ),
        (3, vec![], FileRejection::Count),
        (4, vec![uri().0, uri().0], FileRejection::Count),
        (
            5,
            vec!["file://server/share/file.txt".into()],
            FileRejection::Type,
        ),
        (
            6,
            vec![format!("{}#fragment", uri().0)],
            FileRejection::Type,
        ),
        (
            7,
            vec!["x".repeat(MAX_URI_BYTES + 1)],
            FileRejection::Metadata,
        ),
        (
            8,
            // Keep the file URI valid for the host platform, then inject a
            // decoded NUL. A Unix path is a Type rejection on Windows first.
            vec![format!("{}%00", uri().0)],
            FileRejection::Metadata,
        ),
    ] {
        let serial = begin(&f, &api, n);
        deliver(&f, &api, n, serial, data(uris));
        settle(&f);
        assert_eq!(
            f.events.borrow().last().unwrap().outcome,
            FileOutcome::Rejected(error)
        );
        assert!(f.host.app().tree().selected_file(previous).is_ok());
        assert_eq!(f.driver.open_len(), 1);
        assert!(!f.files.stats().dropping);
    }
}
#[test]
fn retired_or_unavailable_targets_ignore_the_native_body_before_any_reader_call() {
    for change in 0..5 {
        let f = fixture(1.);
        let api = Api::default();
        let serial = begin(&f, &api, 1);
        match change {
            0 => {
                f.host.app().scope().clear_files(f.id).unwrap();
                f.host.app().flush().unwrap();
            }
            1 => {
                f.enabled.revise(|v| *v = false);
                f.host.app().flush().unwrap();
            }
            2 => {
                f.host
                    .app()
                    .scope()
                    .set_file_configuration(
                        f.id,
                        FileConfig {
                            drop: true,
                            max_file_bytes: 8,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                f.host.app().flush().unwrap();
            }
            3 => {
                receive(f.host.renderer(), &WindowEvent::Focused(false), &api);
            }
            _ => {
                f.host.app().clear().unwrap();
            }
        }
        let raw = data(vec![uri().0]);
        deliver(&f, &api, 1, serial, raw.clone());
        settle(&f);
        assert_eq!(raw.calls.load(Ordering::Relaxed), 0);
        assert_eq!(f.driver.open_len(), 0);
        assert!(!f.files.stats().dropping);
        assert!(f.events.borrow().is_empty());
    }
}
#[test]
fn cancelled_opening_retains_cross_document_admission_until_actual_driver_settlement() {
    let f = fixture(1.);
    let other = with_service(f.files.clone(), f.driver.clone(), 1.);
    let api = Api::default();
    let serial = begin(&f, &api, 1);
    deliver(&f, &api, 1, serial, data(vec![uri().0]));
    settle(&f);
    f.host.app().scope().clear_files(f.id).unwrap();
    f.host.app().flush().unwrap();
    settle(&f);
    assert!(f.files.stats().dropping);
    assert_eq!(f.driver.open_len(), 1);
    receive(
        other.host.renderer(),
        &WindowEvent::DragEntered {
            id: id(2),
            position: Some(other.position),
        },
        &api,
    );
    receive(
        other.host.renderer(),
        &WindowEvent::DragDropped {
            id: id(2),
            proposed_action: Some(DndAction::Copy),
        },
        &api,
    );
    settle(&other);
    assert_eq!(api.fetches.borrow().len(), 1);
    assert_eq!(
        other.events.borrow().last().unwrap().outcome,
        FileOutcome::Rejected(FileRejection::Busy)
    );
    f.driver.finish(0, vec![selected("late.txt")]);
    settle(&f);
    assert!(!f.files.stats().dropping);
    assert_eq!(f.files.stats().references, 0);
    assert!(f.events.borrow().is_empty());
    let serial = begin(&other, &api, 3);
    deliver(&other, &api, 3, serial, data(vec![uri().0]));
    settle(&other);
    f.driver.finish(1, vec![selected("current.txt")]);
    settle(&other);
    assert!(other.host.app().tree().selected_file(file(&other)).is_ok());
}
#[test]
fn picker_and_drop_completion_order_never_resurrects_an_older_selection() {
    for picker_first in [true, false] {
        let f = fixture(1.);
        let api = Api::default();
        f.host.app().scope().pick_files(f.id).unwrap();
        f.host.app().flush().unwrap();
        settle(&f);
        let serial = begin(&f, &api, 1);
        deliver(&f, &api, 1, serial, data(vec![uri().0]));
        settle(&f);
        if picker_first {
            f.driver.finish_pick(0, vec![selected("picker.txt")]);
            settle(&f);
            f.driver.finish(0, vec![selected("drop.txt")]);
        } else {
            f.driver.finish(0, vec![selected("drop.txt")]);
            settle(&f);
            f.driver.finish_pick(0, vec![selected("picker.txt")]);
        }
        settle(&f);
        assert_eq!(f.events.borrow().len(), 1);
        let token = file(&f);
        let tree = f.host.app().tree();
        assert_eq!(
            tree.selected_file(token).unwrap().name,
            if picker_first {
                "picker.txt"
            } else {
                "drop.txt"
            }
        );
        assert!(!f.files.stats().picker && !f.files.stats().dropping);
    }
}
#[test]
fn would_block_retries_only_the_current_receipt_and_raw_deadlines_do_not_open_late_paths() {
    let f = fixture(1.);
    let api = Api::default();
    let serial = begin(&f, &api, 1);
    let blocked = Arc::new(Data {
        uris: vec![],
        calls: Arc::new(AtomicUsize::new(0)),
        error: Some(io::ErrorKind::WouldBlock),
        before: None,
    });
    deliver(&f, &api, 1, serial, blocked);
    assert!(f.files.stats().dropping);
    assert_eq!(f.driver.open_len(), 0);
    let raw = data(vec![uri().0]);
    deliver(&f, &api, 1, serial, raw);
    settle(&f);
    f.driver.finish(0, vec![selected("ready.txt")]);
    settle(&f);
    assert!(!f.files.stats().dropping);
    let previous = file(&f);
    let serial = begin(&f, &api, 2);
    // Set the current deadline deterministically without a wall-clock race.
    f.host
        .renderer()
        .0
        .borrow_mut()
        .files
        .drop
        .current
        .as_mut()
        .unwrap()
        .deadline = Instant::now();
    settle(&f);
    assert!(f.files.stats().dropping);
    assert_eq!(
        f.events.borrow().last().unwrap().outcome,
        FileOutcome::Rejected(FileRejection::Failed)
    );
    let raw = data(vec![uri().0]);
    deliver(&f, &api, 2, serial, raw.clone());
    settle(&f);
    assert_eq!(raw.calls.load(Ordering::Relaxed), 0);
    assert_eq!(f.driver.open_len(), 1);
    assert!(!f.files.stats().dropping);
    assert!(f.host.app().tree().selected_file(previous).is_ok());
}
#[test]
fn noncopy_actions_outside_hits_and_unpaired_ids_never_fetch_native_data() {
    let f = fixture(1.);
    let api = Api::default();
    receive(
        f.host.renderer(),
        &WindowEvent::DragEntered {
            id: id(1),
            position: Some(PhysicalPosition::new(f64::NAN, 0.)),
        },
        &api,
    );
    receive(
        f.host.renderer(),
        &WindowEvent::DragDropped {
            id: id(1),
            proposed_action: None,
        },
        &api,
    );
    receive(
        f.host.renderer(),
        &WindowEvent::DragEntered {
            id: id(2),
            position: Some(f.position),
        },
        &api,
    );
    receive(
        f.host.renderer(),
        &WindowEvent::DragDropped {
            id: id(2),
            proposed_action: Some(DndAction::Move),
        },
        &api,
    );
    receive(
        f.host.renderer(),
        &WindowEvent::DragPosition {
            id: id(3),
            position: f.position,
            proposed_action: None,
        },
        &api,
    );
    receive(
        f.host.renderer(),
        &WindowEvent::DragDropped {
            id: id(3),
            proposed_action: None,
        },
        &api,
    );
    assert!(api.fetches.borrow().is_empty());
    assert_eq!(f.driver.open_len(), 0);
}

#[test]
fn source_callbacks_run_outside_borrows_and_cannot_open_after_reentrant_clear() {
    thread_local! { static CURRENT: RefCell<Option<(Renderer, NodeId)>> = const { RefCell::new(None) }; }
    fn clear() {
        let (mut renderer, node) = CURRENT.with(|c| c.borrow().as_ref().unwrap().clone());
        assert!(renderer.stats().nodes > 0);
        renderer
            .action(Action::ClearFiles { node, revision: 0 })
            .unwrap();
    }
    let f = fixture(1.);
    let api = Api::default();
    let serial = begin(&f, &api, 1);
    CURRENT.with(|c| *c.borrow_mut() = Some((f.host.renderer().clone(), f.id)));
    let raw = Arc::new(Data {
        uris: vec![uri().0],
        calls: Arc::new(AtomicUsize::new(0)),
        error: None,
        before: Some(clear),
    });
    deliver(&f, &api, 1, serial, raw.clone());
    settle(&f);
    assert_eq!(raw.calls.load(Ordering::Relaxed), 1);
    assert_eq!(f.driver.open_len(), 0);
    assert!(!f.files.stats().dropping);
    assert!(f.events.borrow().is_empty());
    CURRENT.with(|c| {
        c.borrow_mut().take();
    });
}
#[test]
fn containing_focus_scope_and_new_modal_revoke_background_drop_rights() {
    use glory_render::{FocusMode, FocusScopeConfig, Layer, Role, Semantics};
    let f = fixture(1.);
    let api = Api::default();
    let serial = begin(&f, &api, 1);
    let active = f.host.app().scope().cage(false);
    f.host
        .mount(
            glory_ui::view()
                .focus(FocusMode::Programmatic)
                .semantics(active.map(|active| {
                    Some(Box::new(Semantics {
                        focus_scope: Some(FocusScopeConfig {
                            active: *active,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }))
                }))
                .fill(glory_ui::button().label("Inside scope")),
        )
        .unwrap();
    active.revise(|v| *v = true);
    f.host.app().flush().unwrap();
    settle(&f);
    let raw = data(vec![uri().0]);
    deliver(&f, &api, 1, serial, raw.clone());
    settle(&f);
    assert_eq!(raw.calls.load(Ordering::Relaxed), 0);
    assert_eq!(f.driver.open_len(), 0);
    active.revise(|v| *v = false);
    f.host.app().flush().unwrap();
    settle(&f);
    let serial = begin(&f, &api, 2);
    f.host
        .mount(
            glory_ui::view()
                .role(Role::Dialog)
                .name("New modal")
                .focus(FocusMode::Programmatic)
                .semantics(Some(Box::new(Semantics {
                    id: Some(glory_render::SemanticId::new("native-drop-modal").unwrap()),
                    layer: Some(Layer {
                        modal: true,
                        ..Default::default()
                    }),
                    ..Default::default()
                })))
                .fill(glory_ui::button().label("Confirm")),
        )
        .unwrap();
    settle(&f);
    let raw = data(vec![uri().0]);
    deliver(&f, &api, 2, serial, raw.clone());
    settle(&f);
    assert_eq!(raw.calls.load(Ordering::Relaxed), 0);
    assert_eq!(f.driver.open_len(), 0);
}
#[test]
fn drop_declarations_require_an_explicit_opener_and_an_interactive_host() {
    let driver = Arc::new(Driver::default());
    for files in [
        Files::new(driver.clone()),
        Files::new(driver.clone()).with_drop_driver(driver.clone()),
    ] {
        let runtime = RuntimeId::new();
        let mut renderer = Renderer::with_config(
            runtime,
            Config {
                files: Some(files),
                ..Default::default()
            },
        );
        assert!(!renderer.capabilities().files.unwrap().drop);
        let declaration = glory_render::Descriptor::new(Props::File(glory_render::FileProps {
            configuration: FileConfig {
                drop: true,
                ..Default::default()
            },
            ..Default::default()
        }));
        assert_eq!(
            renderer.preflight(declaration.view(), None),
            Err(Error::Unsupported("native file drop"))
        );
        let mut ids = glory_render::NodeIds::new(runtime);
        assert!(
            renderer
                .apply(&[Mutation::Create {
                    id: ids.allocate(),
                    node: Box::new(declaration)
                }])
                .is_err()
        );
    }
}

#[test]
fn drop_opener_future_and_retired_readers_run_outside_the_document_borrow() {
    use std::cell::Cell;
    thread_local! {
        static DOCUMENT: RefCell<Option<Renderer>> = const { RefCell::new(None) };
        static FUTURE_DROPPED: Cell<bool> = const { Cell::new(false) };
        static READER_DROPPED: Cell<bool> = const { Cell::new(false) };
    }
    fn inspect() {
        DOCUMENT.with(|doc| assert!(doc.borrow().as_ref().unwrap().stats().nodes > 0));
    }
    struct Opened;
    impl FileReader for Opened {
        fn read(&self, _: u64, length: u32) -> Reply<Vec<u8>> {
            Box::pin(std::future::ready(Ok(vec![7; length as usize])))
        }
    }
    impl Drop for Opened {
        fn drop(&mut self) {
            inspect();
            READER_DROPPED.set(true);
        }
    }
    struct Done;
    impl Future for Done {
        type Output = Result<Vec<SelectedFile>, FileRejection>;
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            inspect();
            Poll::Ready(Ok(vec![SelectedFile {
                name: "current.txt".into(),
                media_type: "text/plain".into(),
                size: 4,
                reader: Arc::new(Opened),
            }]))
        }
    }
    impl Drop for Done {
        fn drop(&mut self) {
            inspect();
            FUTURE_DROPPED.set(true);
        }
    }
    struct Opener;
    impl FileDrop for Opener {
        fn open(&self, _: Vec<PathBuf>) -> Reply<Vec<SelectedFile>> {
            inspect();
            Box::pin(Done)
        }
    }
    let driver = Arc::new(Driver::default());
    let f = with_service(
        Files::new(driver.clone()).with_drop_driver(Arc::new(Opener)),
        driver,
        1.,
    );
    DOCUMENT.with(|doc| *doc.borrow_mut() = Some(f.host.renderer().clone()));
    let api = Api::default();
    let serial = begin(&f, &api, 1);
    deliver(&f, &api, 1, serial, data(vec![uri().0]));
    settle(&f);
    assert!(FUTURE_DROPPED.get());
    assert!(!READER_DROPPED.get());
    f.host.app().scope().clear_files(f.id).unwrap();
    f.host.app().flush().unwrap();
    assert!(READER_DROPPED.get());
    DOCUMENT.with(|doc| {
        doc.borrow_mut().take();
    });
}

#[test]
fn native_fetch_refusal_releases_admission_and_preserves_the_previous_selection() {
    struct Unavailable;
    impl Transfers for Unavailable {
        fn has_uris(&self, _: DataTransferId) -> bool {
            true
        }
        fn accept_copy(&self, _: DataTransferId, _: bool) {}
        fn fetch(&self, _: DataTransferId) -> Result<AsyncRequestSerial, FileRejection> {
            Err(FileRejection::Unavailable)
        }
    }
    let f = fixture(1.);
    let api = Api::default();
    let serial = begin(&f, &api, 1);
    deliver(&f, &api, 1, serial, data(vec![uri().0]));
    settle(&f);
    f.driver.finish(0, vec![selected("previous.txt")]);
    settle(&f);
    let previous = file(&f);
    receive(
        f.host.renderer(),
        &WindowEvent::DragEntered {
            id: id(2),
            position: Some(f.position),
        },
        &Unavailable,
    );
    receive(
        f.host.renderer(),
        &WindowEvent::DragDropped {
            id: id(2),
            proposed_action: Some(DndAction::Copy),
        },
        &Unavailable,
    );
    settle(&f);
    assert_eq!(
        f.events.borrow().last().unwrap().outcome,
        FileOutcome::Rejected(FileRejection::Unavailable)
    );
    assert!(f.host.app().tree().selected_file(previous).is_ok());
    assert!(!f.files.stats().dropping);
    assert_eq!(f.driver.open_len(), 1);
}

#[test]
fn author_css_visibility_is_current_before_native_reads_and_late_open_publication() {
    for after_start in [false, true] {
        let f = fixture(1.);
        let api = Api::default();
        let serial = begin(&f, &api, 1);
        let raw = data(vec![uri().0]);
        if after_start {
            deliver(&f, &api, 1, serial, raw.clone());
            settle(&f);
            assert_eq!(f.driver.open_len(), 1);
        }
        f.host.renderer().set_css("button{display:none}");
        if after_start {
            f.driver.finish(0, vec![selected("hidden.txt")]);
        } else {
            deliver(&f, &api, 1, serial, raw.clone());
        }
        settle(&f);
        assert_eq!(raw.calls.load(Ordering::Relaxed), usize::from(after_start));
        assert_eq!(f.driver.open_len(), usize::from(after_start));
        assert!(f.events.borrow().is_empty());
        assert_eq!(f.files.stats().references, 0);
        assert!(!f.files.stats().dropping);
    }
}
