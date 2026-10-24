//! Bounded current browser file references. Native listeners only snapshot raw
//! handles; semantic validation/publication happens in the host observation.
use super::{
    backend,
    dom::Dom,
    file_document::{Cost, DocumentFiles, DropPermit, ReadPermit, SelectionLease},
};
use crate::file_reads::{Consumer as ReadConsumer, Reads};
use glory_render::{memory::Memory, *};
use std::{
    collections::HashMap,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Element, EventTarget, File, HtmlInputElement};

struct Stored {
    info: FileInfo,
    file: File,
    preview: Option<FilePreview>,
}
struct Selection {
    revision: u64,
    files: Vec<Stored>,
    lease: SelectionLease,
}
pub(super) struct Files {
    pub document: FileDocumentId,
    shared: Rc<DocumentFiles>,
    selections: HashMap<NodeId, Selection>,
    batch: u64,
    serial: u64,
    reads: Reads<ReadCompletion>,
    drop: Option<(NodeId, u64, Raw, Option<DropPermit>)>,
    closed: bool,
}
enum Raw {
    Selected(Vec<File>),
    Cancelled,
    Rejected(FileRejection),
}
#[derive(Clone)]
struct PickConsumer {
    dom: Weak<Dom>,
    document: FileDocumentId,
    node: NodeId,
    revision: u64,
    request: u64,
}
struct Listener {
    target: EventTarget,
    name: &'static str,
    callback: Closure<dyn FnMut(web_sys::Event)>,
}
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.name, self.callback.as_ref().unchecked_ref());
    }
}
pub(super) struct Picker {
    ticket: u64,
    consumer: Option<PickConsumer>,
    input: HtmlInputElement,
    _listeners: Vec<Listener>,
    ready: Option<Raw>,
}
impl Files {
    pub fn new(document: &web_sys::Document) -> Result<Self, Error> {
        Ok(Self {
            document: FileDocumentId::new()?,
            shared: DocumentFiles::acquire(document),
            selections: HashMap::new(),
            batch: 0,
            serial: 0,
            reads: Reads::default(),
            drop: None,
            closed: false,
        })
    }
    fn next_batch(&mut self) -> Result<u64, Error> {
        self.batch = self
            .batch
            .checked_add(1)
            .ok_or(Error::Unsupported("file observation identity exhausted"))?;
        Ok(self.batch)
    }
    fn remove(&mut self, node: NodeId) -> Option<Selection> {
        self.selections.remove(&node)
    }
    fn stored(&self, token: FileToken) -> Result<&Stored, Error> {
        if token.document != self.document {
            return Err(Error::Unsupported("file token belongs to another consumer"));
        }
        self.selections
            .get(&token.node)
            .filter(|selection| selection.revision == token.revision)
            .and_then(|selection| {
                selection
                    .files
                    .iter()
                    .find(|stored| stored.info.token == token)
            })
            .ok_or(Error::Unsupported("file token is no longer selected"))
    }
    fn select(
        &mut self,
        node: NodeId,
        props: &FileProps,
        raw: Vec<File>,
    ) -> Result<(Vec<FileInfo>, Option<Selection>), FileRejection> {
        if raw.len() > props.configuration.max_files {
            return Err(FileRejection::Count);
        }
        let serial = self
            .serial
            .checked_add(raw.len() as u64)
            .ok_or(FileRejection::Failed)?;
        let mut files = Vec::with_capacity(raw.len());
        let mut total = 0u64;
        let mut metadata = 0usize;
        for (index, file) in raw.into_iter().enumerate() {
            let name =
                string(&file, "name", MAX_FILE_NAME_BYTES).map_err(|_| FileRejection::Metadata)?;
            let name = normalize_text(&name, true).into_owned();
            let media_type = string(&file, "type", MAX_FILE_MIME_BYTES)
                .map_err(|_| FileRejection::Metadata)?
                .to_ascii_lowercase();
            let size = js_sys::Reflect::get(&file, &"size".into())
                .map_err(|_| FileRejection::Metadata)?
                .as_f64()
                .filter(|v| {
                    v.is_finite() && *v >= 0. && v.fract() == 0. && *v <= 9_007_199_254_740_991.
                })
                .ok_or(FileRejection::Bytes)?;
            if size > props.configuration.max_file_bytes as f64 {
                return Err(FileRejection::Bytes);
            }
            let size = size as u64;
            total = total.checked_add(size).ok_or(FileRejection::Bytes)?;
            if total > props.configuration.max_total_bytes {
                return Err(FileRejection::Bytes);
            }
            metadata = metadata
                .checked_add(name.len() + media_type.len())
                .ok_or(FileRejection::Metadata)?;
            if metadata > MAX_FILE_METADATA_BYTES {
                return Err(FileRejection::Metadata);
            }
            let info = FileInfo {
                token: FileToken {
                    document: self.document,
                    node,
                    revision: props.revision,
                    serial: self.serial + index as u64 + 1,
                },
                name,
                media_type,
                size,
            };
            info.validate().map_err(|_| FileRejection::Metadata)?;
            if !props.configuration.filters.is_empty()
                && !props
                    .configuration
                    .filters
                    .iter()
                    .any(|filter| filter.matches(&info))
            {
                return Err(FileRejection::Type);
            }
            files.push(Stored {
                info,
                file,
                preview: None,
            });
        }
        let lease = self
            .shared
            .select(
                self.selections
                    .get_mut(&node)
                    .map(|selection| &mut selection.lease),
                Cost {
                    references: files.len(),
                    metadata,
                    bytes: total,
                },
            )
            .map_err(|_| FileRejection::Busy)?;
        self.serial = serial;
        let infos = files.iter().map(|stored| stored.info.clone()).collect();
        let previous = self.selections.insert(
            node,
            Selection {
                revision: props.revision,
                files,
                lease,
            },
        );
        Ok((infos, previous))
    }
}
fn string(value: &JsValue, name: &str, max: usize) -> Result<String, Error> {
    let value = js_sys::Reflect::get(value, &name.into())
        .map_err(backend)?
        .dyn_into::<js_sys::JsString>()
        .map_err(|_| Error::InvalidProps("file metadata is not a string"))?;
    if value.length() as usize > max {
        return Err(Error::InvalidProps("file metadata UTF-16 budget"));
    }
    let value = value
        .as_string()
        .ok_or(Error::InvalidProps("file metadata encoding"))?;
    if value.len() > max {
        return Err(Error::InvalidProps("file metadata UTF-8 budget"));
    }
    Ok(value)
}
fn snapshot(input: &HtmlInputElement) -> Raw {
    let Some(files) = input.files() else {
        return Raw::Cancelled;
    };
    if files.length() == 0 {
        return Raw::Cancelled;
    }
    if files.length() as usize > MAX_FILE_COUNT {
        return Raw::Rejected(FileRejection::Count);
    }
    Raw::Selected(
        (0..files.length())
            .filter_map(|index| files.item(index))
            .collect(),
    )
}
fn revoke(selection: &Selection) {
    for stored in &selection.files {
        if let Some(preview) = &stored.preview {
            let _ = web_sys::Url::revoke_object_url(&preview.source);
        }
    }
}
fn release_picker(shared: &DocumentFiles, ticket: u64) -> Option<Picker> {
    let mut slot = shared.picker.borrow_mut();
    if slot.as_ref().is_some_and(|picker| picker.ticket == ticket) {
        slot.take()
    } else {
        None
    }
}
fn picker_event(shared: Rc<DocumentFiles>, ticket: u64, event: web_sys::Event) {
    if !event.is_trusted() {
        return;
    }
    let consumer = {
        let mut slot = shared.picker.borrow_mut();
        let Some(picker) = slot
            .as_mut()
            .filter(|picker| picker.ticket == ticket && picker.ready.is_none())
        else {
            return;
        };
        picker.ready = Some(if event.type_() == "cancel" {
            Raw::Cancelled
        } else {
            snapshot(&picker.input)
        });
        picker.consumer.clone()
    };
    if let Some(dom) = consumer
        .and_then(|consumer| consumer.dom.upgrade())
        .filter(|dom| !dom.files.borrow().closed)
    {
        dom.frame.input();
    } else {
        // Releasing listeners from inside their own invocation is unsafe; release
        // the current, finished chooser on the next microtask instead.
        wasm_bindgen_futures::spawn_local(async move {
            drop(release_picker(&shared, ticket));
        });
    }
}
impl Dom {
    pub(super) fn pick_files(
        self: &Rc<Self>,
        node: NodeId,
        revision: u64,
        request: u64,
    ) -> Result<(), FileRejection> {
        let (document, shared) = {
            let files = self.files.borrow();
            (files.document, files.shared.clone())
        };
        if shared.picker.borrow().is_some() {
            return Err(FileRejection::Busy);
        }
        let ticket = shared
            .next_picker
            .get()
            .checked_add(1)
            .ok_or(FileRejection::Failed)?;
        let input: HtmlInputElement = self
            .entry(node)
            .map_err(|_| FileRejection::Failed)?
            .part(glory_html::Part::Picker)
            .map_err(|_| FileRejection::Failed)?
            .dyn_into()
            .map_err(|_| FileRejection::Failed)?;
        let method = js_sys::Reflect::get(&input, &"showPicker".into())
            .map_err(|_| FileRejection::Unavailable)?
            .dyn_into::<js_sys::Function>()
            .map_err(|_| FileRejection::Unavailable)?;
        input.set_value("");
        let mut listeners = Vec::new();
        for name in ["change", "cancel"] {
            let shared = shared.clone();
            let callback = Closure::new(move |event: web_sys::Event| {
                picker_event(shared.clone(), ticket, event)
            });
            let target: EventTarget = input.clone().into();
            target
                .add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())
                .map_err(|_| FileRejection::Failed)?;
            listeners.push(Listener {
                target,
                name,
                callback,
            });
        }
        shared.next_picker.set(ticket);
        *shared.picker.borrow_mut() = Some(Picker {
            ticket,
            consumer: Some(PickConsumer {
                dom: Rc::downgrade(self),
                document,
                node,
                revision,
                request,
            }),
            input: input.clone(),
            _listeners: listeners,
            ready: None,
        });
        if method.call0(&input).is_err() {
            drop(release_picker(&shared, ticket));
            return Err(FileRejection::Unavailable);
        }
        Ok(())
    }
    pub(super) fn reject_picker(
        &self,
        mapper: &mut glory_html::Mapper,
        node: NodeId,
        revision: u64,
        request: u64,
        reason: FileRejection,
    ) -> Result<(), Error> {
        let document = self.files.borrow().document;
        let event = FileEvent {
            document,
            revision,
            request: Some(request),
            batch: 1,
            origin: FileOrigin::Picker,
            outcome: FileOutcome::Rejected(reason),
        };
        if mapper.finish_files(node, document, &event)? {
            self.inputs.borrow_mut().push(Input::Event {
                node,
                event: Event::Files(Box::new(event)),
            });
            self.frame.input();
        }
        Ok(())
    }
    /// Called only after complete subtree validation/adoption. The validation
    /// plan stores bounded numeric identities, never speculative File handles.
    pub(super) fn adopt_files(
        &self,
        mapper: &mut glory_html::Mapper,
        nodes: Vec<NodeId>,
    ) -> Result<(), Error> {
        for node in nodes {
            let revision = match mapper.tree().node(node)?.descriptor.view().props {
                Props::File(props) => props.revision,
                _ => return Err(Error::InvalidProps("file adoption target")),
            };
            let input: HtmlInputElement = self
                .entry(node)?
                .part(glory_html::Part::Picker)?
                .dyn_into()
                .map_err(|value: web_sys::Node| backend(value.into()))?;
            self.publish_files(
                mapper,
                node,
                revision,
                None,
                FileOrigin::Adopted,
                snapshot(&input),
            )?;
            input.set_value("");
        }
        Ok(())
    }
    pub(super) fn read_files(&self, mapper: &mut glory_html::Mapper) -> Result<(), Error> {
        let (document, shared) = {
            let files = self.files.borrow();
            (files.document, files.shared.clone())
        };
        let ticket = shared
            .picker
            .borrow()
            .as_ref()
            .filter(|picker| {
                picker.ready.is_some()
                    && picker
                        .consumer
                        .as_ref()
                        .is_some_and(|consumer| consumer.document == document)
            })
            .map(|picker| picker.ticket);
        if let Some(ticket) = ticket {
            let mut picker = release_picker(&shared, ticket).expect("current ready picker");
            let consumer = picker.consumer.take();
            let raw = picker.ready.take();
            // The bounded raw snapshot owns accepted candidates. Do not leave
            // rejected OS FileLists retained by the hidden input.
            picker.input.set_value("");
            drop(picker);
            if let (Some(consumer), Some(raw)) = (consumer, raw)
                && mapper
                    .tree()
                    .file_pending(consumer.node, consumer.request, consumer.revision)
                    .unwrap_or(false)
            {
                self.publish_files(
                    mapper,
                    consumer.node,
                    consumer.revision,
                    Some(consumer.request),
                    FileOrigin::Picker,
                    raw,
                )?;
            }
        }
        let raw_drop = self.files.borrow_mut().drop.take();
        if let Some((node, revision, raw, permit)) = raw_drop {
            if mapper.tree().node(node).is_ok() {
                self.publish_files(mapper, node, revision, None, FileOrigin::Drop, raw)?;
            }
            drop(permit);
        }
        self.read_file_results(mapper.tree());
        Ok(())
    }
    fn publish_files(
        &self,
        mapper: &mut glory_html::Mapper,
        node: NodeId,
        revision: u64,
        request: Option<u64>,
        origin: FileOrigin,
        raw: Raw,
    ) -> Result<(), Error> {
        let model = mapper.tree().node(node)?;
        let Props::File(props) = model.descriptor.view().props else {
            return Ok(());
        };
        if props.revision != revision || origin == FileOrigin::Drop && !props.configuration.drop {
            return Ok(());
        }
        let batch_result = self.files.borrow_mut().next_batch();
        let batch = match batch_result {
            Ok(batch) => batch,
            Err(error) => {
                if let Some(request) = request {
                    return self.reject_picker(
                        mapper,
                        node,
                        revision,
                        request,
                        FileRejection::Failed,
                    );
                }
                return Err(error);
            }
        };
        let allowed =
            mapper.tree().active(node) && mapper.tree().enabled(node) && !mapper.tree().inert(node);
        let mut previous = None;
        let outcome = if !allowed {
            FileOutcome::Rejected(FileRejection::Failed)
        } else {
            match raw {
                Raw::Cancelled => FileOutcome::Cancelled,
                Raw::Rejected(reason) => FileOutcome::Rejected(reason),
                Raw::Selected(raw) => match self.files.borrow_mut().select(node, props, raw) {
                    Ok((files, old)) => {
                        previous = old;
                        FileOutcome::Selected(files)
                    }
                    Err(reason) => FileOutcome::Rejected(reason),
                },
            }
        };
        if let Some(old) = previous {
            revoke(&old);
            drop(old);
        }
        let document = self.files.borrow().document;
        let event = FileEvent {
            document,
            revision,
            request,
            batch,
            origin,
            outcome,
        };
        if mapper.finish_files(node, document, &event)? {
            self.inputs.borrow_mut().push(Input::Event {
                node,
                event: Event::Files(Box::new(event)),
            });
            self.frame.input();
        }
        Ok(())
    }
}

enum ReadKind {
    Bytes(FileRead),
    Preview {
        token: FileToken,
        size: u64,
        media_type: String,
        limits: FilePreviewLimits,
    },
}
struct ReadCompletion {
    kind: ReadKind,
    result: Result<Vec<u8>, Error>,
    _permit: ReadPermit,
}
fn retire_picker(
    shared: &DocumentFiles,
    document: FileDocumentId,
    node: Option<NodeId>,
) -> Option<Picker> {
    let mut slot = shared.picker.borrow_mut();
    let picker = slot.as_mut()?;
    if !picker.consumer.as_ref().is_some_and(|consumer| {
        consumer.document == document && node.is_none_or(|node| node == consumer.node)
    }) {
        return None;
    }
    picker.consumer = None;
    if picker.ready.is_some() {
        slot.take()
    } else {
        None
    }
}
impl Dom {
    pub(super) fn update_files(&self, tree: &Memory) {
        let mut retired = Vec::new();
        let mut removed = Vec::new();
        let (document, shared, read_results) = {
            let mut state = self.files.borrow_mut();
            let nodes: Vec<_> = state.selections.keys().copied().collect();
            for node in nodes {
                let Some(mut selection) = state.selections.remove(&node) else {
                    continue;
                };
                let valid=tree.node(node).is_ok_and(|node|matches!(node.descriptor.view().props,Props::File(props) if props.revision==selection.revision));
                if !valid {
                    removed.push(selection);
                    continue;
                }
                let mut kept = Vec::new();
                for file in selection.files.drain(..) {
                    if tree.selected_file(file.info.token).is_ok() {
                        kept.push(file);
                    } else {
                        retired.push(file);
                    }
                }
                if !kept.is_empty() {
                    let cost = Cost {
                        references: kept.len(),
                        metadata: kept
                            .iter()
                            .map(|file| file.info.name.len() + file.info.media_type.len())
                            .sum(),
                        bytes: kept.iter().map(|file| file.info.size).sum(),
                    };
                    let lease = state
                        .shared
                        .select(Some(&mut selection.lease), cost)
                        .expect("retaining fewer selected files fits its old charge");
                    state.selections.insert(
                        node,
                        Selection {
                            revision: selection.revision,
                            files: kept,
                            lease,
                        },
                    );
                }
            }
            let read_results = state
                .reads
                .revoke_invalid(|file| tree.selected_file(file).is_ok());
            (state.document, state.shared.clone(), read_results)
        };
        self.retired_file_reads(read_results);
        for selection in &removed {
            revoke(selection);
        }
        for stored in &retired {
            if let Some(preview) = &stored.preview {
                let _ = web_sys::Url::revoke_object_url(&preview.source);
            }
        }
        drop(removed);
        drop(retired);
        let pending = shared
            .picker
            .borrow()
            .as_ref()
            .and_then(|picker| picker.consumer.clone())
            .filter(|consumer| consumer.document == document);
        if let Some(consumer) = pending
            && !tree
                .file_pending(consumer.node, consumer.request, consumer.revision)
                .unwrap_or(false)
        {
            drop(retire_picker(&shared, document, Some(consumer.node)));
        }
    }
    pub(super) fn clear_file_control(&self, node: NodeId) {
        let (selection, retired, document, shared) = {
            let mut files = self.files.borrow_mut();
            (
                files.remove(node),
                files.reads.revoke_invalid(|file| file.node != node),
                files.document,
                files.shared.clone(),
            )
        };
        if let Some(selection) = selection {
            revoke(&selection);
            drop(selection);
        }
        self.retired_file_reads(retired);
        drop(retire_picker(&shared, document, Some(node)));
        if let Ok(entry) = self.entry(node)
            && let Ok(input) = entry.part(glory_html::Part::Picker).and_then(|node| {
                node.dyn_into::<HtmlInputElement>()
                    .map_err(|_| Error::Unsupported("file input part"))
            })
        {
            input.set_value("");
        }
    }
    fn retired_file_reads(&self, retired: Vec<(ReadConsumer, Option<ReadCompletion>)>) {
        for (consumer, result) in retired {
            drop(result);
            self.inputs.borrow_mut().push(Input::Response {
                token: consumer.query,
                applied: consumer.applied,
                result: Err(Error::Unsupported("file read rights revoked")),
            });
        }
    }
    pub(super) fn close_files(&self) {
        let (selections, retired, document, shared) = {
            let mut files = self.files.borrow_mut();
            files.closed = true;
            files.drop = None;
            let retired = files.reads.retire_all();
            (
                std::mem::take(&mut files.selections),
                retired,
                files.document,
                files.shared.clone(),
            )
        };
        for selection in selections.values() {
            revoke(selection);
        }
        drop(selections);
        drop(retired);
        drop(retire_picker(&shared, document, None));
    }
    pub(super) fn request_file(
        self: &Rc<Self>,
        tree: &Memory,
        query: Query,
        token: QueryToken,
        applied: u64,
    ) -> Result<(), Error> {
        let file = match query {
            Query::FileRead(read) => read.file,
            Query::FilePreview(file) => file,
            _ => return Err(Error::Unsupported("file query kind")),
        };
        query.validate_target(&tree.node(file.node)?.descriptor)?;
        if !tree.active(file.node) || !tree.enabled(file.node) || tree.inert(file.node) {
            return Err(Error::Unsupported(
                "file control is inactive, disabled or inert",
            ));
        }
        let info = tree.selected_file(file)?;
        let (raw, kind, length, offset, shared) = {
            let state = self.files.borrow();
            let stored = state.stored(file)?;
            let (kind, length, offset) = match query {
                Query::FileRead(read) => {
                    read.validate_file(info)?;
                    (ReadKind::Bytes(read), read.length as usize, read.offset)
                }
                Query::FilePreview(_) => {
                    if let Some(preview) = &stored.preview {
                        self.inputs.borrow_mut().push(Input::Response {
                            token,
                            applied,
                            result: Ok(QueryValue::FilePreview(Some(Box::new(preview.clone())))),
                        });
                        self.frame.input();
                        return Ok(());
                    }
                    let Props::File(props) = tree.node(file.node)?.descriptor.view().props else {
                        unreachable!()
                    };
                    let limits = props
                        .configuration
                        .preview
                        .ok_or(Error::Unsupported("file previews are not enabled"))?;
                    if !matches!(info.media_type.as_str(), "image/png" | "image/jpeg") {
                        self.inputs.borrow_mut().push(Input::Response {
                            token,
                            applied,
                            result: Ok(QueryValue::FilePreview(None)),
                        });
                        self.frame.input();
                        return Ok(());
                    }
                    if info.size > limits.encoded_bytes {
                        return Err(Error::Unsupported("image preview encoded byte budget"));
                    }
                    (
                        ReadKind::Preview {
                            token: file,
                            size: info.size,
                            media_type: info.media_type.clone(),
                            limits,
                        },
                        info.size.min(MAX_FILE_HEADER_BYTES as u64) as usize,
                        0,
                    )
                }
                _ => unreachable!(),
            };
            (
                stored.file.clone(),
                kind,
                length,
                offset,
                state.shared.clone(),
            )
        };
        let permit = shared.read()?;
        let ticket = self.files.borrow_mut().reads.begin(ReadConsumer {
            file,
            query: token,
            applied,
        })?;
        let promise = (|| {
            let blob = raw
                .slice_with_f64_and_f64(offset as f64, (offset + length as u64) as f64)
                .map_err(backend)?;
            let method = js_sys::Reflect::get(&blob, &"arrayBuffer".into())
                .map_err(backend)?
                .dyn_into::<js_sys::Function>()
                .map_err(|_| Error::Unsupported("Blob.arrayBuffer"))?;
            method
                .call0(&blob)
                .map_err(backend)?
                .dyn_into::<js_sys::Promise>()
                .map_err(|_| Error::Backend("Blob.arrayBuffer returned no Promise".into()))
        })();
        match promise {
            Err(error) => {
                let removed = self.files.borrow_mut().reads.finish(
                    ticket,
                    ReadCompletion {
                        kind,
                        result: Err(error),
                        _permit: permit,
                    },
                );
                drop(removed);
                self.frame.input();
            }
            Ok(promise) => {
                let weak = Rc::downgrade(self);
                wasm_bindgen_futures::spawn_local(async move {
                    let result = match JsFuture::from(promise).await {
                        Err(error) => Err(backend(error)),
                        Ok(buffer) => match buffer.dyn_into::<js_sys::ArrayBuffer>() {
                            Err(_) => {
                                Err(Error::Backend("file read returned no ArrayBuffer".into()))
                            }
                            Ok(buffer) => {
                                if buffer.byte_length() as usize != length
                                    || length > MAX_FILE_CHUNK_BYTES as usize
                                {
                                    Err(Error::InvalidResponse(
                                        "file read byte length exceeds its admitted range",
                                    ))
                                } else {
                                    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
                                }
                            }
                        },
                    };
                    let Some(dom) = weak.upgrade() else {
                        return;
                    };
                    let removed = dom.files.borrow_mut().reads.finish(
                        ticket,
                        ReadCompletion {
                            kind,
                            result,
                            _permit: permit,
                        },
                    );
                    drop(removed);
                    dom.frame.input();
                });
            }
        }
        Ok(())
    }
    fn read_file_results(&self, tree: &Memory) {
        let ready = self.files.borrow_mut().reads.take_ready();
        for (consumer, completion) in ready {
            let result = (|| {
                tree.selected_file(consumer.file)?;
                let bytes = completion.result?;
                match completion.kind {
                    ReadKind::Bytes(read) => Ok(QueryValue::FileBytes(Box::new(FileChunk {
                        file: read.file,
                        offset: read.offset,
                        bytes,
                    }))),
                    ReadKind::Preview {
                        token,
                        size,
                        media_type,
                        limits,
                    } => {
                        let image = match inspect_file_preview(&bytes, size, &media_type, limits) {
                            Ok(image) => image,
                            Err(_) => return Ok(QueryValue::FilePreview(None)),
                        };
                        let mut state = self.files.borrow_mut();
                        let selection = state
                            .selections
                            .get_mut(&token.node)
                            .ok_or(Error::Unsupported("file preview target retired"))?;
                        let stored = selection
                            .files
                            .iter_mut()
                            .find(|stored| stored.info.token == token)
                            .ok_or(Error::Unsupported("file preview token retired"))?;
                        if stored.preview.is_none() {
                            let source = web_sys::Url::create_object_url_with_blob(&stored.file)
                                .map_err(backend)?;
                            stored.preview = Some(FilePreview {
                                file: token,
                                image,
                                source,
                            });
                        }
                        Ok(QueryValue::FilePreview(
                            stored.preview.clone().map(Box::new),
                        ))
                    }
                }
            })();
            self.inputs.borrow_mut().push(Input::Response {
                token: consumer.query,
                applied: consumer.applied,
                result,
            });
        }
    }
}

pub(super) fn receive(dom: &Dom, name: &str, event: &web_sys::Event) -> Result<bool, Error> {
    if !matches!(name, "dragenter" | "dragover" | "drop") {
        return Ok(false);
    }
    let Some(element) = event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())
        .and_then(|element| element.closest("[data-gly-kind=file]").ok().flatten())
    else {
        return Ok(false);
    };
    let Some((node, _)) = super::events::target(dom, &element) else {
        return Ok(false);
    };
    if !element.has_attribute("data-gly-file-drop") {
        return Ok(false);
    }
    event.prevent_default();
    if name != "drop"
        || !event.is_trusted()
        || element.closest("[inert]").ok().flatten().is_some()
        || element.matches(":disabled").unwrap_or(true)
    {
        return Ok(true);
    }
    let Some(drag) = event.dyn_ref::<web_sys::DragEvent>() else {
        return Ok(true);
    };
    let Some(list) = drag.data_transfer().and_then(|transfer| transfer.files()) else {
        return Ok(true);
    };
    if list.length() == 0 {
        return Ok(true);
    }
    let previous = dom.files.borrow_mut().drop.take();
    drop(previous);
    let shared = dom.files.borrow().shared.clone();
    let permit = shared.raw_drop();
    let raw = if permit.is_none() {
        Raw::Rejected(FileRejection::Busy)
    } else if list.length() as usize > MAX_FILE_COUNT {
        Raw::Rejected(FileRejection::Count)
    } else {
        Raw::Selected(
            (0..list.length())
                .filter_map(|index| list.item(index))
                .collect(),
        )
    };
    let revision = dom.entry(node)?.image_revision();
    dom.files.borrow_mut().drop = Some((node, revision, raw, permit));
    dom.frame.input();
    Ok(true)
}
