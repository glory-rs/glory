//! Document-owned opaque references. Poll/drop foreign futures outside the
//! engine borrow; retain actual operations after their receipt is revoked.
use super::*;
use crate::files::{Cost, Files, Lease, Permit, SelectedFile};
use futures_timer::Delay;
use glory_render::{
    Action, FileCapabilities, FileChunk, FileDocumentId, FileEvent, FileInfo, FileOrigin,
    FileOutcome, FilePreview, FileRejection, FileToken, Props,
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
    time::Instant,
};
struct Stored {
    info: FileInfo,
    reader: Arc<dyn crate::files::FileReader>,
}
pub(super) struct Selection {
    files: Vec<Stored>,
    lease: Lease,
}
struct Pick {
    node: NodeId,
    revision: u64,
    request: u64,
    configuration: glory_render::FileConfig,
    future: crate::files::Reply<Option<Vec<SelectedFile>>>,
    started: bool,
    reported: bool,
    deadline: Instant,
    timer: Option<Delay>,
    _permit: Permit,
}
struct ReadConsumer {
    operation: Query,
    query: QueryToken,
    applied: u64,
}
enum ReadResult {
    Chunk(FileChunk),
    Preview(Option<(FilePreview, Arc<crate::files::preview::Body>)>),
}
struct ReadJob {
    consumer: Option<ReadConsumer>,
    future: crate::files::Reply<ReadResult>,
    started: bool,
    deadline: Instant,
    timer: Option<Delay>,
    _permit: Permit,
    live: Arc<std::sync::atomic::AtomicBool>,
}
pub(super) struct Io {
    config: Option<Files>,
    document: Option<FileDocumentId>,
    serial: u64,
    batch: u64,
    selections: HashMap<NodeId, Selection>,
    picker: Option<Pick>,
    reads: Vec<ReadJob>,
    retired: Vec<Selection>,
    previews: Arc<crate::files::preview::Registry>,
}
impl Io {
    pub fn install_provider(&self, config: &mut Config) {
        if self.document.is_some() {
            config.provider = Some(Arc::new(crate::files::preview::Provider {
                registry: self.previews.clone(),
                fallback: config.provider.take(),
            }));
        }
    }
    pub fn close(&self) {
        self.previews.shutdown();
    }
    pub fn new(config: Option<Files>) -> Self {
        let document = config.as_ref().and_then(|_| FileDocumentId::new().ok());
        Self {
            config,
            document,
            serial: 0,
            batch: 0,
            selections: HashMap::new(),
            picker: None,
            reads: Vec::new(),
            retired: Vec::new(),
            previews: Arc::new(crate::files::preview::Registry::default()),
        }
    }
    pub fn capabilities(&self) -> Option<FileCapabilities> {
        self.document.map(|document| FileCapabilities {
            document,
            picker: true,
            drop: false,
            read: true,
            preview: true,
        })
    }
}
pub(super) fn check(
    node: glory_render::DescriptorView<'_>,
    patch: Option<&glory_render::Patch>,
    enabled: bool,
) -> Result<(), Error> {
    if let Props::File(props) = node.props {
        if !enabled {
            return Err(Error::Unsupported("native file picker controls"));
        }
        let default = glory_render::FileConfig::default();
        let config = match patch {
            Some(glory_render::Patch::File {
                configuration: glory_render::Change::Set(config),
                ..
            }) => config.as_ref(),
            Some(glory_render::Patch::File {
                configuration: glory_render::Change::Clear,
                ..
            }) => &default,
            _ => &props.configuration,
        };
        if config.drop {
            return Err(Error::Unsupported("native file drop"));
        }
    }
    Ok(())
}
fn available(state: &State, node: NodeId) -> bool {
    state.mapper.tree().active(node)
        && state.mapper.tree().enabled(node)
        && !state.mapper.tree().inert(node)
        && state.layers.allowed(state.mapper.tree(), node)
}
fn pending(state: &State, pick: &Pick) -> bool {
    state
        .mapper
        .tree()
        .file_pending(pick.node, pick.request, pick.revision)
        .unwrap_or(false)
}
fn finish(
    state: &mut State,
    node: NodeId,
    revision: u64,
    request: u64,
    outcome: FileOutcome,
) -> Result<(), Error> {
    let document = state
        .files
        .document
        .ok_or(Error::Unsupported("native file access"))?;
    let next = state.files.batch.checked_add(1);
    let batch = next.unwrap_or(1); // A request-specific refusal need not advance selection's batch.
    let event = FileEvent {
        document,
        revision,
        request: Some(request),
        batch,
        origin: FileOrigin::Picker,
        outcome,
    };
    if state.mapper.finish_files(node, document, &event)? {
        if next.is_some() {
            state.files.batch = batch;
        }
        state.inputs.push(Input::Event {
            node,
            event: Event::Files(Box::new(event)),
        });
    }
    Ok(())
}
pub(super) fn action(state: &mut State, action: Action) -> Result<(), Error> {
    let config = state
        .files
        .config
        .clone()
        .filter(|_| state.files.document.is_some())
        .ok_or(Error::Unsupported("native file access"))?;
    if !state.layers.allowed(state.mapper.tree(), action.node()) {
        return Err(Error::Unsupported("file control outside modal scope"));
    }
    state.mapper.admit_file_action(action)?;
    if let Action::PickFiles {
        node,
        revision,
        request,
    } = action
    {
        let permit = if state.files.picker.is_some() {
            Err(FileRejection::Busy)
        } else {
            config.shared.permit(true)
        };
        let permit = match permit {
            Ok(permit) => permit,
            Err(error) => {
                finish(state, node, revision, request, FileOutcome::Rejected(error))?;
                state.observation_pending = true;
                return Ok(());
            }
        };
        let Props::File(props) = state.mapper.tree().node(node)?.descriptor.view().props else {
            unreachable!()
        };
        let configuration = props.configuration.clone();
        let declaration = configuration.clone();
        let future = Box::pin(async move { config.driver.pick(declaration).await });
        state.files.picker = Some(Pick {
            node,
            revision,
            request,
            configuration,
            future,
            started: false,
            reported: false,
            deadline: Instant::now() + config.timeout,
            timer: Some(Delay::new(config.timeout)),
            _permit: permit,
        });
    }
    sync(state);
    state.observation_pending = true;
    Ok(())
}
pub(super) fn sync(state: &mut State) {
    let nodes = state.files.selections.keys().copied().collect::<Vec<_>>();
    for id in nodes {
        if state.files.selections.get(&id).is_some_and(|s| {
            s.files
                .iter()
                .all(|s| state.mapper.tree().selected_file(s.info.token).is_ok())
        }) {
            continue;
        }
        let Some(mut selection) = state.files.selections.remove(&id) else {
            continue;
        };
        let mut kept = Vec::new();
        let mut removed = Vec::new();
        for file in selection.files.drain(..) {
            if state.mapper.tree().selected_file(file.info.token).is_ok() {
                kept.push(file);
            } else {
                removed.push(file);
            }
        }
        if kept.is_empty() {
            selection.files = removed;
            state.files.retired.push(selection);
        } else {
            let cost = Cost {
                references: kept.len(),
                metadata: kept
                    .iter()
                    .map(|s| s.info.name.len() + s.info.media_type.len())
                    .sum(),
                bytes: kept.iter().map(|s| s.info.size).sum(),
            };
            let config = state.files.config.as_ref().expect("configured files");
            let lease = config
                .shared
                .select(Some(&mut selection.lease), cost)
                .expect("smaller selection fits");
            state
                .files
                .selections
                .insert(id, Selection { files: kept, lease });
            selection.files = removed;
            state.files.retired.push(selection);
        }
    }
    let revoked = state
        .files
        .previews
        .retain(|file| state.mapper.tree().selected_file(file).is_ok());
    for url in revoked {
        state.dom.doc.revoke_image_url(&url);
    }
    for job in &mut state.files.reads {
        if job.consumer.as_ref().is_some_and(|c| {
            state
                .mapper
                .tree()
                .selected_file(operation_file(c.operation))
                .is_err()
        }) {
            job.live.store(false, std::sync::atomic::Ordering::Release);
            let consumer = job.consumer.take().expect("revoked read");
            state.inputs.push(Input::Response {
                token: consumer.query,
                applied: consumer.applied,
                result: Err(Error::Unsupported("file read rights revoked")),
            });
        }
    }
}
pub(super) fn take_retired(state: &mut State) -> Vec<Selection> {
    std::mem::take(&mut state.files.retired)
}
fn operation_file(operation: Query) -> FileToken {
    match operation {
        Query::FileRead(read) => read.file,
        Query::FilePreview(file) => file,
        _ => unreachable!(),
    }
}
pub(super) fn request(state: &mut State, operation: Query, query: QueryToken) -> Result<(), Error> {
    let file = operation_file(operation);
    let config = state
        .files
        .config
        .as_ref()
        .ok_or(Error::Unsupported("native file access"))?;
    if !available(state, file.node) {
        return Err(Error::Unsupported("file control unavailable"));
    }
    operation.validate_target(&state.mapper.tree().node(file.node)?.descriptor)?;
    let info = state.mapper.tree().selected_file(file)?.clone();
    if state.files.document != Some(file.document) {
        return Err(Error::Unsupported("foreign file document"));
    }
    let reader = state
        .files
        .selections
        .get(&file.node)
        .and_then(|s| s.files.iter().find(|s| s.info.token == file))
        .ok_or(Error::Unsupported("file token retired"))?
        .reader
        .clone();
    let preview = if let Query::FilePreview(_) = operation {
        let Props::File(props) = state.mapper.tree().node(file.node)?.descriptor.view().props
        else {
            unreachable!()
        };
        let limits = props
            .configuration
            .preview
            .ok_or(Error::Unsupported("file previews disabled"))?;
        if let Some(preview) = state.files.previews.get(file) {
            state.inputs.push(Input::Response {
                token: query,
                applied: state.receipt.applied,
                result: Ok(QueryValue::FilePreview(Some(Box::new(preview)))),
            });
            state.observation_pending = true;
            return Ok(());
        }
        Some(limits)
    } else {
        None
    };
    if let Query::FileRead(read) = operation {
        read.validate_file(&info)?;
    }
    let permit = config
        .shared
        .permit(false)
        .map_err(|_| Error::Unsupported("file read capacity is full"))?;
    let shared = config.shared.clone();
    let timeout = config.timeout;
    let live = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let current = live.clone();
    let future = Box::pin(async move {
        if let Query::FileRead(read) = operation {
            let bytes = reader.read(read.offset, read.length).await?;
            return Ok(ReadResult::Chunk(FileChunk {
                file,
                offset: read.offset,
                bytes,
            }));
        }
        let limits = preview.expect("preview request");
        if !matches!(info.media_type.as_str(), "image/png" | "image/jpeg") {
            return Ok(ReadResult::Preview(None));
        }
        if info.size > limits.encoded_bytes {
            return Err(FileRejection::Bytes);
        }
        let length = info.size.min(glory_render::MAX_FILE_HEADER_BYTES as u64) as u32;
        let header = reader.read(0, length).await?;
        if header.len() != length as usize {
            return Err(FileRejection::Bytes);
        }
        if !current.load(std::sync::atomic::Ordering::Acquire) {
            return Err(FileRejection::Unavailable);
        }
        let image = match glory_render::inspect_file_preview(
            &header,
            info.size,
            &info.media_type,
            limits,
        ) {
            Ok(image) => image,
            Err(_) => return Ok(ReadResult::Preview(None)),
        };
        let charge = shared.preview(info.size as usize)?;
        let mut bytes = Vec::with_capacity(info.size as usize);
        bytes.extend_from_slice(&header);
        drop(header);
        while (bytes.len() as u64) < info.size {
            if !current.load(std::sync::atomic::Ordering::Acquire) {
                return Err(FileRejection::Unavailable);
            }
            let offset = bytes.len() as u64;
            let length =
                (info.size - offset).min(u64::from(glory_render::MAX_FILE_CHUNK_BYTES)) as u32;
            let chunk = reader.read(offset, length).await?;
            if chunk.len() != length as usize {
                return Err(FileRejection::Bytes);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(ReadResult::Preview(Some((
            FilePreview {
                image,
                file,
                source: FilePreview::native_source(file),
            },
            Arc::new(crate::files::preview::Body {
                bytes: bytes.into(),
                _charge: charge,
            }),
        ))))
    });
    state.files.reads.push(ReadJob {
        consumer: Some(ReadConsumer {
            operation,
            query,
            applied: state.receipt.applied,
        }),
        future,
        started: false,
        deadline: Instant::now() + timeout,
        timer: Some(Delay::new(timeout)),
        _permit: permit,
        live,
    });
    state.observation_pending = true;
    Ok(())
}

fn prepare(
    raw: Vec<SelectedFile>,
    props: &glory_render::FileConfig,
    document: FileDocumentId,
    node: NodeId,
    revision: u64,
    serial: u64,
) -> Result<(Vec<Stored>, u64), FileRejection> {
    if raw.is_empty() || raw.len() > props.max_files {
        return Err(FileRejection::Count);
    }
    let next = serial
        .checked_add(raw.len() as u64)
        .ok_or(FileRejection::Failed)?;
    let mut files = Vec::new();
    let mut bytes = 0u64;
    let mut metadata = 0usize;
    for (index, file) in raw.into_iter().enumerate() {
        if file.name.len() > glory_render::MAX_FILE_NAME_BYTES
            || file.media_type.len() > glory_render::MAX_FILE_MIME_BYTES
        {
            return Err(FileRejection::Metadata);
        }
        let name = normalize_text(&file.name, true).into_owned();
        let media_type = file.media_type.to_ascii_lowercase();
        let info = FileInfo {
            token: FileToken {
                document,
                node,
                revision,
                serial: serial + index as u64 + 1,
            },
            name,
            media_type,
            size: file.size,
        };
        if info.size > props.max_file_bytes {
            return Err(FileRejection::Bytes);
        }
        info.validate().map_err(|_| FileRejection::Metadata)?;
        bytes = bytes.checked_add(info.size).ok_or(FileRejection::Bytes)?;
        metadata = metadata
            .checked_add(info.name.len() + info.media_type.len())
            .ok_or(FileRejection::Metadata)?;
        if info.size > props.max_file_bytes || bytes > props.max_total_bytes {
            return Err(FileRejection::Bytes);
        }
        if metadata > glory_render::MAX_FILE_METADATA_BYTES {
            return Err(FileRejection::Metadata);
        }
        if !props.filters.is_empty() && !props.filters.iter().any(|f| f.matches(&info)) {
            return Err(FileRejection::Type);
        }
        files.push(Stored {
            info,
            reader: file.reader,
        });
    }
    Ok((files, next))
}
pub(super) fn poll(renderer: &Renderer) {
    let (picker, reads, wake) = {
        let mut state = renderer.0.borrow_mut();
        sync(&mut state);
        let retired = take_retired(&mut state);
        let picker = state.files.picker.take();
        let reads = std::mem::take(&mut state.files.reads);
        let wake = state.dom.resource_wake.clone();
        drop(state);
        drop(retired);
        (picker, reads, wake)
    };
    let waker = Waker::from(wake);
    let mut cx = Context::from_waker(&waker);
    if let Some(mut pick) = picker {
        let valid = {
            let state = renderer.0.borrow();
            pending(&state, &pick) && available(&state, pick.node)
        };
        let result = if !pick.started && (!valid || Instant::now() >= pick.deadline) {
            Poll::Ready(Err(FileRejection::Failed))
        } else {
            pick.started = true;
            pick.future.as_mut().poll(&mut cx)
        };
        let expired = !pick.reported
            && (Instant::now() >= pick.deadline
                || pick
                    .timer
                    .as_mut()
                    .is_some_and(|t| Pin::new(t).poll(&mut cx).is_ready()));
        match result {
            Poll::Pending => {
                let timer = if expired { pick.timer.take() } else { None };
                let mut state = renderer.0.borrow_mut();
                if expired
                    && pending(&state, &pick)
                    && let Err(error) = finish(
                        &mut state,
                        pick.node,
                        pick.revision,
                        pick.request,
                        FileOutcome::Rejected(FileRejection::Failed),
                    )
                {
                    state.input_error = Some(error);
                }
                pick.reported |= expired;
                state.files.picker = Some(pick);
                drop(state);
                drop(timer);
            }
            Poll::Ready(result) => {
                let (document, serial) = {
                    let state = renderer.0.borrow();
                    (
                        state.files.document.expect("file document"),
                        state.files.serial,
                    )
                };
                let prepared = match result {
                    Ok(Some(raw)) => prepare(
                        raw,
                        &pick.configuration,
                        document,
                        pick.node,
                        pick.revision,
                        serial,
                    )
                    .map(Some),
                    Ok(None) => Ok(None),
                    Err(error) => Err(error),
                };
                let mut state = renderer.0.borrow_mut();
                let mut unused = None;
                if !pick.reported && pending(&state, &pick) {
                    let outcome = if expired || !available(&state, pick.node) {
                        unused = prepared.ok().flatten();
                        FileOutcome::Rejected(FileRejection::Failed)
                    } else {
                        match prepared {
                            Err(error) => FileOutcome::Rejected(error),
                            Ok(None) => FileOutcome::Cancelled,
                            Ok(Some((files, next))) if state.files.batch == u64::MAX => {
                                unused = Some((files, next));
                                FileOutcome::Rejected(FileRejection::Failed)
                            }
                            Ok(Some((files, next))) => {
                                let cost = Cost {
                                    references: files.len(),
                                    metadata: files
                                        .iter()
                                        .map(|s| s.info.name.len() + s.info.media_type.len())
                                        .sum(),
                                    bytes: files.iter().map(|s| s.info.size).sum(),
                                };
                                let shared = state
                                    .files
                                    .config
                                    .as_ref()
                                    .expect("files service")
                                    .shared
                                    .clone();
                                let lease = shared.select(
                                    state
                                        .files
                                        .selections
                                        .get_mut(&pick.node)
                                        .map(|s| &mut s.lease),
                                    cost,
                                );
                                match lease {
                                    Err(error) => {
                                        unused = Some((files, next));
                                        FileOutcome::Rejected(error)
                                    }
                                    Ok(lease) => {
                                        let infos = files.iter().map(|f| f.info.clone()).collect();
                                        if let Some(previous) = state
                                            .files
                                            .selections
                                            .insert(pick.node, Selection { files, lease })
                                        {
                                            state.files.retired.push(previous);
                                        }
                                        state.files.serial = next;
                                        FileOutcome::Selected(infos)
                                    }
                                }
                            }
                        }
                    };
                    if let Err(error) =
                        finish(&mut state, pick.node, pick.revision, pick.request, outcome)
                    {
                        state.input_error = Some(error);
                    }
                } else {
                    unused = prepared.ok().flatten();
                }
                let retired = take_retired(&mut state);
                drop(state);
                drop(retired);
                drop(unused);
                drop(pick);
            }
        }
    }
    poll_reads(renderer, reads, &mut cx);
}
fn poll_reads(renderer: &Renderer, mut reads: Vec<ReadJob>, cx: &mut Context<'_>) {
    let mut pending_reads = Vec::new();
    for mut job in reads.drain(..) {
        if !job.started && (job.consumer.is_none() || Instant::now() >= job.deadline) {
            if let Some(consumer) = job.consumer.take() {
                renderer.0.borrow_mut().inputs.push(Input::Response {
                    token: consumer.query,
                    applied: consumer.applied,
                    result: Err(Error::Unsupported("file read expired")),
                });
            }
            continue;
        }
        if job.consumer.is_none() || Instant::now() >= job.deadline {
            job.live.store(false, std::sync::atomic::Ordering::Release);
        }
        job.started = true;
        let result = job.future.as_mut().poll(cx);
        let expired = job.consumer.is_some()
            && (Instant::now() >= job.deadline
                || job
                    .timer
                    .as_mut()
                    .is_some_and(|t| Pin::new(t).poll(cx).is_ready()));
        let mut state = renderer.0.borrow_mut();
        if job.consumer.as_ref().is_some_and(|c| {
            state
                .mapper
                .tree()
                .selected_file(operation_file(c.operation))
                .is_err()
        }) || expired
        {
            job.live.store(false, std::sync::atomic::Ordering::Release);
            if let Some(c) = job.consumer.take() {
                state.inputs.push(Input::Response {
                    token: c.query,
                    applied: c.applied,
                    result: Err(Error::Unsupported("file read rights revoked or expired")),
                });
            }
        }
        match result {
            Poll::Pending => {
                drop(state);
                if expired || job.consumer.is_none() {
                    job.timer = None;
                }
                pending_reads.push(job);
            }
            Poll::Ready(result) => {
                if let Some(c) = job.consumer.take() {
                    let result = result
                        .and_then(|result| match result {
                            ReadResult::Chunk(chunk) => Ok(QueryValue::FileBytes(Box::new(chunk))),
                            ReadResult::Preview(None) => Ok(QueryValue::FilePreview(None)),
                            ReadResult::Preview(Some((preview, body))) => state
                                .files
                                .previews
                                .publish(preview, body)
                                .map(|preview| QueryValue::FilePreview(Some(Box::new(preview)))),
                        })
                        .map_err(|_| Error::Unsupported("native file read or preview failed"))
                        .and_then(|value| {
                            c.operation.validate_response(&value)?;
                            Ok(value)
                        });
                    state.inputs.push(Input::Response {
                        token: c.query,
                        applied: c.applied,
                        result,
                    });
                }
                drop(state);
                drop(job);
            }
        }
    }
    renderer.0.borrow_mut().files.reads.extend(pending_reads);
}
