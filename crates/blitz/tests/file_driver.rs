#![cfg(all(feature = "files", not(target_arch = "wasm32")))]
use glory_blitz::{
    Config, Renderer,
    files::{FilePicker, FileReader, Files, Reply, SelectedFile},
};
use glory_render::*;
use glory_ui::{App, file};
use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
    time::Duration,
};
struct Gate<T> {
    result: Option<Result<T, FileRejection>>,
    waker: Option<Waker>,
}
impl<T> Default for Gate<T> {
    fn default() -> Self {
        Self {
            result: None,
            waker: None,
        }
    }
}
struct Wait<T>(Arc<Mutex<Gate<T>>>);
impl<T: Send + 'static> Future for Wait<T> {
    type Output = Result<T, FileRejection>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut gate = self.0.lock().unwrap();
        if let Some(result) = gate.result.take() {
            return Poll::Ready(result);
        }
        let old = gate.waker.replace(cx.waker().clone());
        drop(gate);
        drop(old);
        Poll::Pending
    }
}
impl<T> Drop for Wait<T> {
    fn drop(&mut self) {
        let wake = self.0.lock().unwrap().waker.take();
        drop(wake);
    }
}
fn finish<T>(gate: &Arc<Mutex<Gate<T>>>, result: Result<T, FileRejection>) {
    let wake = {
        let mut gate = gate.lock().unwrap();
        gate.result = Some(result);
        gate.waker.take()
    };
    if let Some(w) = wake {
        w.wake()
    }
}
type PickerGate = Arc<Mutex<Gate<Option<Vec<SelectedFile>>>>>;
#[derive(Default)]
struct Picker {
    calls: Mutex<Vec<(FileConfig, PickerGate)>>,
}
impl FilePicker for Picker {
    fn pick(&self, config: FileConfig) -> Reply<Option<Vec<SelectedFile>>> {
        let gate = Arc::new(Mutex::new(Gate::default()));
        self.calls.lock().unwrap().push((config, gate.clone()));
        Box::pin(Wait(gate))
    }
}
impl Picker {
    fn len(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
    fn finish(&self, index: usize, result: Result<Option<Vec<SelectedFile>>, FileRejection>) {
        let gate = self.calls.lock().unwrap()[index].1.clone();
        finish(&gate, result)
    }
}
type ReadGate = Arc<Mutex<Gate<Vec<u8>>>>;
#[derive(Default)]
struct Reader {
    calls: Mutex<Vec<(u64, u32, ReadGate)>>,
    drops: Arc<AtomicUsize>,
}
impl FileReader for Reader {
    fn read(&self, offset: u64, length: u32) -> Reply<Vec<u8>> {
        let gate = Arc::new(Mutex::new(Gate::default()));
        self.calls
            .lock()
            .unwrap()
            .push((offset, length, gate.clone()));
        Box::pin(Wait(gate))
    }
}
impl Drop for Reader {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}
impl Reader {
    fn finish(&self, index: usize, result: Result<Vec<u8>, FileRejection>) {
        let gate = self.calls.lock().unwrap()[index].2.clone();
        finish(&gate, result)
    }
    fn len(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}
fn selected(reader: Arc<Reader>, name: &str, size: u64) -> SelectedFile {
    SelectedFile {
        name: name.into(),
        media_type: "text/plain".into(),
        size,
        reader,
    }
}
struct Fixture {
    app: App,
    renderer: Renderer,
    service: Files,
    picker: Arc<Picker>,
    id: NodeId,
    events: Rc<RefCell<Vec<FileEvent>>>,
}
fn with_service(service: Files, picker: Arc<Picker>, config: FileConfig) -> Fixture {
    let mut renderer = None;
    let app = App::new(|runtime| {
        let r = Renderer::with_config(
            runtime,
            Config {
                files: Some(service.clone()),
                ..Default::default()
            },
        );
        renderer = Some(r.clone());
        r
    });
    let node = app.scope().cage(None);
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    app.mount(
        file()
            .node_ref(node)
            .configuration(config)
            .on(EventKind::Files, move |event| {
                if let Event::Files(event) = event {
                    output.borrow_mut().push(*event)
                }
            }),
    )
    .unwrap();
    Fixture {
        app,
        renderer: renderer.unwrap(),
        service,
        picker,
        id: node.get().unwrap(),
        events,
    }
}
fn fixture() -> Fixture {
    let picker = Arc::new(Picker::default());
    with_service(Files::new(picker.clone()), picker, FileConfig::default())
}
fn pick(f: &Fixture) -> u64 {
    let request = f.app.scope().pick_files(f.id).unwrap();
    f.app.flush().unwrap();
    request
}
fn token(f: &Fixture) -> FileToken {
    let events = f.events.borrow();
    let FileOutcome::Selected(files) = &events.last().unwrap().outcome else {
        panic!("selected")
    };
    files[0].token
}
fn accept(f: &Fixture, reader: Arc<Reader>, size: u64) {
    pick(f);
    f.app.pump().unwrap();
    let index = f.picker.len() - 1;
    f.picker
        .finish(index, Ok(Some(vec![selected(reader, "中文.txt", size)])));
    f.app.pump().unwrap();
}
type ReadReply = Rc<RefCell<Option<Result<QueryValue, Error>>>>;
fn read(f: &Fixture, file: FileToken, offset: u64, length: u32) -> ReadReply {
    let reply = Rc::new(RefCell::new(None));
    let output = reply.clone();
    f.app
        .request(
            Query::FileRead(FileRead {
                file,
                offset,
                length,
            }),
            move |result| *output.borrow_mut() = Some(result),
        )
        .unwrap();
    reply
}
#[test]
fn picker_is_explicit_lazy_versioned_and_cancellation_preserves_previous_selection() {
    let f = fixture();
    let caps = f.renderer.capabilities().files.unwrap();
    assert!(caps.picker && caps.read && !caps.drop && caps.preview);
    assert_eq!(pick(&f), 1);
    assert_eq!(f.picker.len(), 0);
    f.app.pump().unwrap();
    assert_eq!(f.picker.len(), 1);
    let reader = Arc::new(Reader::default());
    f.picker
        .finish(0, Ok(Some(vec![selected(reader, "A中😀.txt", 7)])));
    f.app.pump().unwrap();
    let file = token(&f);
    assert_eq!(file.document, caps.document);
    assert_eq!(file.serial, 1);
    assert_eq!(f.service.stats().references, 1);
    assert_eq!(pick(&f), 2);
    f.app.pump().unwrap();
    f.picker.finish(1, Ok(None));
    f.app.pump().unwrap();
    assert_eq!(
        f.events.borrow().last().unwrap().outcome,
        FileOutcome::Cancelled
    );
    assert!(f.app.tree().selected_file(file).is_ok());
    f.app.scope().clear_files(f.id).unwrap();
    f.app.flush().unwrap();
    assert!(f.app.tree().selected_file(file).is_err());
    assert_eq!(f.service.stats().references, 0);
}
#[test]
fn file_reader_returns_exact_bounded_chunks_at_the_original_watermark_and_never_reopens_names() {
    let f = fixture();
    let reader = Arc::new(Reader::default());
    accept(&f, reader.clone(), 8);
    let file = token(&f);
    let reply = read(&f, file, 2, 4);
    assert_eq!(reader.len(), 0);
    f.app.pump().unwrap();
    assert_eq!(reader.len(), 1);
    reader.finish(0, Ok(vec![0, 1, 2, 3]));
    f.app.pump().unwrap();
    assert_eq!(
        reply.borrow_mut().take().unwrap().unwrap(),
        QueryValue::FileBytes(Box::new(FileChunk {
            file,
            offset: 2,
            bytes: vec![0, 1, 2, 3]
        }))
    );
    let bad = Rc::new(RefCell::new(None));
    let output = bad.clone();
    f.app
        .request(
            Query::FileRead(FileRead {
                file,
                offset: 7,
                length: 2,
            }),
            move |r| *output.borrow_mut() = Some(r),
        )
        .unwrap();
    f.app.pump().unwrap();
    assert!(bad.borrow().as_ref().is_some_and(Result::is_err));
    assert_eq!(reader.len(), 1);
    let wrong = read(&f, file, 0, 4);
    f.app.pump().unwrap();
    reader.finish(1, Ok(vec![0; 5]));
    f.app.pump().unwrap();
    assert!(matches!(
        wrong.borrow().as_ref(),
        Some(Err(Error::InvalidResponse(_)))
    ));
}
#[test]
fn revoked_reads_complete_queries_once_without_refunding_four_native_operations() {
    let f = fixture();
    let reader = Arc::new(Reader::default());
    accept(&f, reader.clone(), 8);
    let old = token(&f);
    let replies = (0..4).map(|_| read(&f, old, 0, 4)).collect::<Vec<_>>();
    f.app.pump().unwrap();
    assert_eq!(reader.len(), 4);
    assert_eq!(f.service.stats().reads, 4);
    f.app.scope().clear_files(f.id).unwrap();
    f.app.flush().unwrap();
    f.app.pump().unwrap();
    assert!(
        replies
            .iter()
            .all(|r| r.borrow().as_ref().is_some_and(Result::is_err))
    );
    assert_eq!(f.service.stats().reads, 4);
    let next = Arc::new(Reader::default());
    accept(&f, next.clone(), 8);
    let current = token(&f);
    assert!(current.serial > old.serial);
    let rejected = read(&f, current, 0, 4);
    f.app.pump().unwrap();
    assert!(rejected.borrow().as_ref().is_some_and(Result::is_err));
    assert_eq!(next.len(), 0);
    for i in 0..4 {
        reader.finish(i, Ok(vec![7; 4]));
    }
    f.app.pump().unwrap();
    assert_eq!(f.service.stats().reads, 0);
    let accepted = read(&f, current, 0, 4);
    f.app.pump().unwrap();
    next.finish(0, Ok(vec![1; 4]));
    f.app.pump().unwrap();
    assert!(accepted.borrow().as_ref().is_some_and(Result::is_ok));
}
#[test]
fn retiring_a_picker_before_poll_never_calls_the_driver_and_after_start_keeps_its_permit() {
    let f = fixture();
    pick(&f);
    f.app
        .scope()
        .set_file_configuration(
            f.id,
            FileConfig {
                max_file_bytes: 4,
                ..Default::default()
            },
        )
        .unwrap();
    f.app.flush().unwrap();
    f.app.pump().unwrap();
    assert_eq!(f.picker.len(), 0);
    pick(&f);
    f.app.pump().unwrap();
    f.app.scope().clear_files(f.id).unwrap();
    f.app.flush().unwrap();
    pick(&f);
    f.app.pump().unwrap();
    assert_eq!(f.picker.len(), 1);
    assert_eq!(
        f.events.borrow().last().unwrap().outcome,
        FileOutcome::Rejected(FileRejection::Busy)
    );
    f.picker.finish(
        0,
        Ok(Some(vec![selected(
            Arc::new(Reader::default()),
            "late.txt",
            1,
        )])),
    );
    f.app.pump().unwrap();
    assert_eq!(f.service.stats().references, 0);
    assert!(!f.service.stats().picker);
}
#[test]
fn shared_service_budgets_cross_documents_and_foreign_tokens_never_enter_a_reader() {
    let a = fixture();
    let b = with_service(a.service.clone(), a.picker.clone(), FileConfig::default());
    assert_ne!(
        a.renderer.capabilities().files,
        b.renderer.capabilities().files
    );
    let reader = Arc::new(Reader::default());
    accept(&a, reader.clone(), 8);
    let file = token(&a);
    assert!(
        b.app
            .request(
                Query::FileRead(FileRead {
                    file,
                    offset: 0,
                    length: 1
                }),
                |_| {}
            )
            .is_err()
    );
    pick(&a);
    a.app.pump().unwrap();
    pick(&b);
    b.app.pump().unwrap();
    assert_eq!(
        b.events.borrow().last().unwrap().outcome,
        FileOutcome::Rejected(FileRejection::Busy)
    );
    a.picker.finish(1, Ok(None));
    a.app.pump().unwrap();
    a.app.clear().unwrap();
    assert_eq!(a.service.stats().references, 0);
}
#[test]
fn selection_count_type_metadata_and_bytes_refusals_keep_the_previous_grant() {
    let picker = Arc::new(Picker::default());
    let f = with_service(
        Files::new(picker.clone()),
        picker,
        FileConfig {
            filters: vec![FileFilter::Extension(".txt".into())],
            max_file_bytes: 4,
            ..Default::default()
        },
    );
    let reader = Arc::new(Reader::default());
    accept(&f, reader.clone(), 4);
    let old = token(&f);
    for (name, size, expected) in [
        ("wrong.png", 1, FileRejection::Type),
        ("large.txt", 5, FileRejection::Bytes),
        ("", 1, FileRejection::Metadata),
    ] {
        pick(&f);
        f.app.pump().unwrap();
        f.picker.finish(
            f.picker.len() - 1,
            Ok(Some(vec![selected(reader.clone(), name, size)])),
        );
        f.app.pump().unwrap();
        assert_eq!(
            f.events.borrow().last().unwrap().outcome,
            FileOutcome::Rejected(expected)
        );
        assert!(f.app.tree().selected_file(old).is_ok());
    }
    assert_eq!(f.service.stats().references, 1);
}
#[test]
fn deadline_releases_receipt_but_keeps_native_read_and_picker_admission_until_settlement() {
    let picker = Arc::new(Picker::default());
    let service = Files::with_timeout(picker.clone(), Duration::from_millis(100)).unwrap();
    let f = with_service(service, picker, FileConfig::default());
    pick(&f);
    f.app.pump().unwrap();
    std::thread::sleep(Duration::from_millis(150));
    f.app.pump().unwrap();
    assert!(f.service.stats().picker);
    assert_eq!(
        f.events.borrow().last().unwrap().outcome,
        FileOutcome::Rejected(FileRejection::Failed)
    );
    f.picker.finish(0, Ok(None));
    f.app.pump().unwrap();
    assert!(!f.service.stats().picker);
    assert_eq!(f.events.borrow().len(), 1);
}

fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[17, 137, 59, 255].repeat((width * height) as usize))
            .unwrap();
    }
    bytes
}
fn preview(f: &Fixture, file: FileToken) -> ReadReply {
    let reply = Rc::new(RefCell::new(None));
    let output = reply.clone();
    f.app
        .request(Query::FilePreview(file), move |result| {
            *output.borrow_mut() = Some(result)
        })
        .unwrap();
    reply
}
fn accept_image(f: &Fixture, reader: Arc<Reader>, size: u64) {
    pick(f);
    f.app.pump().unwrap();
    f.picker.finish(
        f.picker.len() - 1,
        Ok(Some(vec![SelectedFile {
            name: "image-中文.png".into(),
            media_type: "image/png".into(),
            size,
            reader,
        }])),
    );
    f.app.pump().unwrap();
}
#[test]
fn preview_header_admission_private_resource_decode_cache_and_revocation() {
    let picker = Arc::new(Picker::default());
    let f = with_service(
        Files::new(picker.clone()),
        picker,
        FileConfig {
            preview: Some(Default::default()),
            ..Default::default()
        },
    );
    let bytes = png_bytes(32, 24);
    let reader = Arc::new(Reader::default());
    accept_image(&f, reader.clone(), bytes.len() as u64);
    let file = token(&f);
    let result = preview(&f, file);
    f.app.pump().unwrap();
    assert_eq!(reader.calls.lock().unwrap()[0].0, 0);
    reader.finish(0, Ok(bytes.clone()));
    f.app.pump().unwrap();
    let QueryValue::FilePreview(Some(preview)) = result.borrow_mut().take().unwrap().unwrap()
    else {
        panic!("preview")
    };
    assert_eq!((preview.image.width, preview.image.height), (32, 24));
    assert_eq!(preview.source, FilePreview::native_source(file));
    assert_eq!(f.service.stats().preview_bytes, bytes.len());
    let again = self::preview(&f, file);
    f.app.pump().unwrap();
    assert!(matches!(
        again.borrow().as_ref(),
        Some(Ok(QueryValue::FilePreview(Some(_))))
    ));
    assert_eq!(
        reader.len(),
        1,
        "current previews reuse the same bounded body"
    );
    let image = f.app.scope().cage(None);
    f.app
        .mount(
            glory_ui::image()
                .node_ref(image)
                .source(preview.source.clone())
                .alt("Selected file preview"),
        )
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while f.renderer.resource_stats().cached_images == 0 {
        f.app.pump().unwrap();
        assert!(
            std::time::Instant::now() < deadline,
            "native private image did not decode"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(f.renderer.resource_stats().pending_images, 0);
    f.app.scope().clear_files(f.id).unwrap();
    f.app.flush().unwrap();
    assert_eq!(f.service.stats().preview_bytes, 0);
    let revoked = self::preview(&f, file);
    f.app.pump().unwrap();
    assert!(matches!(revoked.borrow().as_ref(), Some(Err(_))));
    assert_eq!(f.renderer.resource_stats().cached_images, 0);
    let retry = f.app.scope().cage(None);
    f.app
        .mount(
            glory_ui::image()
                .node_ref(retry)
                .source(preview.source.clone())
                .alt("Revoked preview"),
        )
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while f.renderer.resource_stats().pending_images != 0 {
        f.app.pump().unwrap();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let physical = f
        .renderer
        .engine_id(retry.get().unwrap(), glory_html::Part::Main)
        .unwrap();
    f.renderer.inspect(|doc| {
        assert!(
            doc.get_node(physical)
                .unwrap()
                .element_data()
                .unwrap()
                .raster_image_data()
                .is_none()
        )
    });
}
#[test]
fn preview_cancel_stops_new_chunks_after_the_current_os_read_settles() {
    let picker = Arc::new(Picker::default());
    let f = with_service(
        Files::new(picker.clone()),
        picker,
        FileConfig {
            preview: Some(Default::default()),
            ..Default::default()
        },
    );
    let mut bytes = png_bytes(32, 24);
    bytes.resize(140_000, 0);
    let reader = Arc::new(Reader::default());
    accept_image(&f, reader.clone(), bytes.len() as u64);
    let result = preview(&f, token(&f));
    f.app.pump().unwrap();
    reader.finish(0, Ok(bytes[..65536].to_vec()));
    f.app.pump().unwrap();
    assert_eq!(reader.len(), 2);
    assert_eq!(f.service.stats().preview_bytes, bytes.len());
    f.app.scope().clear_files(f.id).unwrap();
    f.app.flush().unwrap();
    f.app.pump().unwrap();
    assert!(matches!(result.borrow().as_ref(), Some(Err(_))));
    assert_eq!(f.service.stats().reads, 1);
    reader.finish(1, Ok(bytes[65536..131072].to_vec()));
    f.app.pump().unwrap();
    assert_eq!(
        reader.len(),
        2,
        "revocation forbids starting the next chunk"
    );
    assert_eq!(f.service.stats().reads, 0);
    assert_eq!(f.service.stats().preview_bytes, 0);
}
#[test]
fn invalid_image_header_and_unsupported_type_do_not_create_a_preview_body() {
    let picker = Arc::new(Picker::default());
    let f = with_service(
        Files::new(picker.clone()),
        picker,
        FileConfig {
            preview: Some(FilePreviewLimits {
                pixels: 4,
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    let reader = Arc::new(Reader::default());
    let bytes = png_bytes(32, 24);
    accept_image(&f, reader.clone(), bytes.len() as u64);
    let result = preview(&f, token(&f));
    f.app.pump().unwrap();
    reader.finish(0, Ok(bytes));
    f.app.pump().unwrap();
    assert!(matches!(
        result.borrow().as_ref(),
        Some(Ok(QueryValue::FilePreview(None)))
    ));
    assert_eq!(f.service.stats().preview_bytes, 0);
    accept(&f, reader.clone(), 8);
    let result = preview(&f, token(&f));
    f.app.pump().unwrap();
    assert!(matches!(
        result.borrow().as_ref(),
        Some(Ok(QueryValue::FilePreview(None)))
    ));
    assert_eq!(
        reader.len(),
        1,
        "unsupported types do not enter a file reader"
    );
}

#[test]
fn foreign_picker_reader_futures_and_retired_handles_run_outside_document_borrows() {
    use std::cell::Cell;
    thread_local! {
        static DOCUMENT: RefCell<Option<Renderer>> = const { RefCell::new(None) };
        static READER_DROPPED: Cell<bool> = const { Cell::new(false) };
    }
    fn inspect() {
        DOCUMENT.with(|doc| {
            assert!(doc.borrow().as_ref().unwrap().stats().nodes > 0);
        });
    }
    struct Done<T>(Option<T>);
    impl<T: Unpin> Future for Done<T> {
        type Output = Result<T, FileRejection>;
        fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            inspect();
            Poll::Ready(Ok(self.0.take().unwrap()))
        }
    }
    impl<T> Drop for Done<T> {
        fn drop(&mut self) {
            inspect();
        }
    }
    struct Inspect;
    impl FileReader for Inspect {
        fn read(&self, _: u64, length: u32) -> Reply<Vec<u8>> {
            inspect();
            Box::pin(Done(Some(vec![7; length as usize])))
        }
    }
    impl Drop for Inspect {
        fn drop(&mut self) {
            inspect();
            READER_DROPPED.set(true);
        }
    }
    struct Driver;
    impl FilePicker for Driver {
        fn pick(&self, _: FileConfig) -> Reply<Option<Vec<SelectedFile>>> {
            inspect();
            Box::pin(Done(Some(Some(vec![SelectedFile {
                name: "display-only.txt".into(),
                media_type: "text/plain".into(),
                size: 4,
                reader: Arc::new(Inspect),
            }]))))
        }
    }
    let mut renderer = None;
    let app = App::new(|runtime| {
        let r = Renderer::with_config(
            runtime,
            Config {
                files: Some(Files::new(Arc::new(Driver))),
                ..Default::default()
            },
        );
        renderer = Some(r.clone());
        r
    });
    let node = app.scope().cage(None);
    let current = app.scope().cage(None);
    app.mount(file().node_ref(node).on(EventKind::Files, move |event| {
        if let Event::Files(event) = event
            && let FileOutcome::Selected(files) = event.outcome
        {
            current.revise(|v| *v = Some(files[0].token));
        }
    }))
    .unwrap();
    DOCUMENT.with(|doc| *doc.borrow_mut() = renderer);
    let id = node.get().unwrap();
    app.scope().pick_files(id).unwrap();
    app.flush().unwrap();
    app.pump().unwrap();
    let result = app.scope().cage(None);
    app.request(
        Query::FileRead(FileRead {
            file: current.get().unwrap(),
            offset: 0,
            length: 4,
        }),
        move |reply| result.revise(|v| *v = Some(reply)),
    )
    .unwrap();
    app.pump().unwrap();
    assert!(result.get().unwrap().is_ok());
    app.scope().clear_files(id).unwrap();
    app.flush().unwrap();
    assert!(READER_DROPPED.get());
    DOCUMENT.with(|doc| {
        doc.borrow_mut().take();
    });
}
