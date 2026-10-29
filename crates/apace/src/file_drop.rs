//! One current native URI transfer and opener, plus one hover identity. Native
//! metadata/data callbacks run outside document borrows. No visited IDs/paths.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use wingy::dpi::PhysicalPosition;
use wingy::{
    data_transfer::{DataTransferId, TypeHint, TypedData},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, AsyncRequestSerial, DndAction},
};
const MAX_URI_BYTES: usize = 4096;
const MAX_URI_TOTAL: usize = 128 * 1024;
struct Hover {
    id: DataTransferId,
    position: Option<PhysicalPosition<f64>>,
}
struct Guard {
    node: NodeId,
    revision: u64,
    live: Arc<AtomicBool>,
}
struct Transfer {
    node: NodeId,
    revision: u64,
    configuration: glory_render::FileConfig,
    live: Arc<AtomicBool>,
    stage: Stage,
    deadline: Instant,
    timer: Option<Delay>,
    reported: bool,
    _permit: Permit,
}
enum Stage {
    Waiting {
        id: DataTransferId,
        serial: Option<AsyncRequestSerial>,
    },
    Opening {
        future: crate::files::Reply<Vec<SelectedFile>>,
        started: bool,
    },
}
#[derive(Default)]
pub(super) struct DropState {
    hover: Option<Hover>,
    current: Option<Transfer>,
    guard: Option<Guard>,
}
/// A small testable facade over the native event-loop transfer calls.
pub(super) trait Transfers {
    fn has_uris(&self, id: DataTransferId) -> bool;
    fn accept_copy(&self, id: DataTransferId, accept: bool);
    fn fetch(&self, id: DataTransferId) -> Result<AsyncRequestSerial, FileRejection>;
}
struct Native<'a>(&'a dyn ActiveEventLoop);
impl Transfers for Native<'_> {
    fn has_uris(&self, id: DataTransferId) -> bool {
        self.0
            .data_transfer(id)
            .is_ok_and(|t| t.has_type(&TypeHint::UriList))
    }
    fn accept_copy(&self, id: DataTransferId, accept: bool) {
        let _ = self
            .0
            .set_valid_dnd_actions(id, if accept { &[DndAction::Copy] } else { &[] });
    }
    fn fetch(&self, id: DataTransferId) -> Result<AsyncRequestSerial, FileRejection> {
        self.0
            .fetch_data_transfer(id, &TypeHint::UriList)
            .map_err(|_| FileRejection::Unavailable)
    }
}
pub(in crate::document) fn native_event(
    renderer: &Renderer,
    event: &WindowEvent,
    event_loop: &dyn ActiveEventLoop,
) {
    receive(renderer, event, &Native(event_loop));
}
fn eligible(state: &State, node: NodeId, revision: u64) -> bool {
    state.files.drop_enabled
        && state.dom.control(node).ok().is_some_and(|control| state.dom.focus_visible(control))
        && available(state, node) && layers::visible(state.mapper.tree(), node)
        && focus_scope::allowed(state, node)
        && state.mapper.tree().node(node).is_ok_and(|n| matches!(n.descriptor.view().props, Props::File(props) if props.enabled && props.revision == revision && props.configuration.drop))
}
fn current(renderer: &Renderer, node: NodeId, revision: u64) -> bool {
    let mut state = renderer.0.borrow_mut();
    state.dom.resolve();
    eligible(&state, node, revision)
}
pub(super) fn retire(state: &mut State, node: NodeId) {
    if let Some(guard) = &state.files.drop.guard
        && guard.node == node
    {
        guard.live.store(false, Ordering::Release);
    }
}
pub(super) fn sync(state: &mut State) {
    if let Some(guard) = &state.files.drop.guard
        && !eligible(state, guard.node, guard.revision)
    {
        guard.live.store(false, Ordering::Release);
    }
}
fn cancel(renderer: &Renderer) {
    let mut state = renderer.0.borrow_mut();
    state.files.drop.hover = None;
    if let Some(guard) = &state.files.drop.guard {
        guard.live.store(false, Ordering::Release);
    }
    state.observation_pending = true;
}
fn hit(renderer: &Renderer, position: PhysicalPosition<f64>) -> Option<(NodeId, u64)> {
    if !position.x.is_finite() || !position.y.is_finite() {
        return None;
    }
    let mut state = renderer.0.borrow_mut();
    if !state.files.drop_enabled || state.files.batch == u64::MAX {
        return None;
    }
    layers::sync(&mut state, &[]).ok()?;
    state.dom.resolve();
    let scale = state.dom.doc.viewport().scale_f64();
    let (x, y) = (position.x / scale, position.y / scale);
    let (width, height) = state.dom.doc.viewport().logical_size();
    if x < 0. || y < 0. || x >= f64::from(width) || y >= f64::from(height) {
        return None;
    }
    let mut cursor = state
        .dom
        .doc
        .element_from_point(x as f32, y as f32)
        .and_then(|id| state.dom.owner(id));
    while let Some(node) = cursor {
        let entry = state.mapper.tree().node(node).ok()?;
        if let Props::File(props) = entry.descriptor.view().props {
            return eligible(&state, node, props.revision).then_some((node, props.revision));
        }
        cursor = entry.parent;
    }
    None
}
fn can_hover(
    renderer: &Renderer,
    position: PhysicalPosition<f64>,
    id: DataTransferId,
    api: &impl Transfers,
) -> bool {
    let Some((node, revision)) = hit(renderer, position) else {
        return false;
    };
    if !api.has_uris(id) {
        return false;
    }
    let mut state = renderer.0.borrow_mut();
    state.dom.resolve();
    eligible(&state, node, revision)
        && !state
            .files
            .config
            .as_ref()
            .expect("files config")
            .stats()
            .dropping
}
fn queue(
    state: &mut State,
    node: NodeId,
    revision: u64,
    outcome: FileOutcome,
) -> Result<(), Error> {
    state.dom.resolve();
    if !eligible(state, node, revision) {
        return Ok(());
    }
    let document = state.files.document.expect("configured drop document");
    let batch = state
        .files
        .batch
        .checked_add(1)
        .ok_or(Error::Unsupported("file observation identity exhausted"))?;
    let event = FileEvent {
        document,
        revision,
        request: None,
        batch,
        origin: FileOrigin::Drop,
        outcome,
    };
    if state.mapper.finish_files(node, document, &event)? {
        state.files.batch = batch;
        state.inputs.push(Input::Event {
            node,
            event: Event::Files(Box::new(event)),
        });
        state.observation_pending = true;
    }
    Ok(())
}
fn report(renderer: &Renderer, node: NodeId, revision: u64, rejection: FileRejection) {
    let mut state = renderer.0.borrow_mut();
    if let Err(error) = queue(&mut state, node, revision, FileOutcome::Rejected(rejection)) {
        state.input_error = Some(error);
    }
}
fn begin(renderer: &Renderer, id: DataTransferId, target: (NodeId, u64), api: &impl Transfers) {
    let (node, revision) = target;
    let mut state = renderer.0.borrow_mut();
    state.dom.resolve();
    if !eligible(&state, node, revision) {
        return;
    }
    let config = state.files.config.as_ref().expect("drop config").clone();
    let permit = match config.shared.drop_permit() {
        Ok(permit) => permit,
        Err(error) => {
            drop(state);
            report(renderer, node, revision, error);
            return;
        }
    };
    let Props::File(props) = state
        .mapper
        .tree()
        .node(node)
        .expect("drop node")
        .descriptor
        .view()
        .props
    else {
        unreachable!()
    };
    let configuration = props.configuration.clone();
    let live = Arc::new(AtomicBool::new(true));
    state.files.drop.guard = Some(Guard {
        node,
        revision,
        live: live.clone(),
    });
    state.files.drop.current = Some(Transfer {
        node,
        revision,
        configuration,
        live,
        stage: Stage::Waiting { id, serial: None },
        deadline: Instant::now() + config.timeout,
        timer: Some(Delay::new(config.timeout)),
        reported: false,
        _permit: permit,
    });
    state.observation_pending = true;
    drop(state);
    // Native calls may cause reentry. The guard/admission already exist, and no
    // engine borrow is held while the source application supplies metadata.
    let serial = api.fetch(id);
    let mut state = renderer.0.borrow_mut();
    match serial {
        Ok(serial) => {
            if let Some(current) = &mut state.files.drop.current
                && let Stage::Waiting { serial: value, .. } = &mut current.stage
            {
                *value = Some(serial);
            }
        }
        Err(error) => {
            let current = state.files.drop.current.take();
            let deliver = current
                .as_ref()
                .is_some_and(|t| t.live.load(Ordering::Acquire) && !t.reported);
            state.files.drop.guard = None;
            drop(state);
            drop(current);
            if deliver {
                report(renderer, node, revision, error);
            }
        }
    }
}
fn paths(uris: Vec<String>, maximum: usize) -> Result<Vec<std::path::PathBuf>, FileRejection> {
    if uris.is_empty() || uris.len() > maximum {
        return Err(FileRejection::Count);
    }
    let mut total = 0usize;
    for uri in &uris {
        total = total
            .checked_add(uri.len())
            .ok_or(FileRejection::Metadata)?;
        if uri.len() > MAX_URI_BYTES || total > MAX_URI_TOTAL || uri.chars().any(char::is_control) {
            return Err(FileRejection::Metadata);
        }
    }
    uris.into_iter()
        .map(|uri| {
            let url = apace_traits::net::Url::parse(&uri).map_err(|_| FileRejection::Type)?;
            if url.scheme() != "file"
                || url.host_str().is_some()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err(FileRejection::Type);
            }
            let path = url.to_file_path().map_err(|_| FileRejection::Type)?;
            let text = path.to_str().ok_or(FileRejection::Metadata)?;
            if !path.is_absolute()
                || text.len() > MAX_URI_BYTES
                || text.chars().any(char::is_control)
            {
                return Err(FileRejection::Metadata);
            }
            Ok(path)
        })
        .collect()
}
fn received(
    renderer: &Renderer,
    id: DataTransferId,
    serial: AsyncRequestSerial,
    value: &Arc<dyn TypedData>,
) {
    let mut transfer = {
        let mut state = renderer.0.borrow_mut();
        let matches = state.files.drop.current.as_ref().is_some_and(|t| matches!(t.stage, Stage::Waiting { id: current, serial: Some(ticket) } if current == id && serial == ticket));
        if !matches {
            return;
        }
        state
            .files
            .drop
            .current
            .take()
            .expect("matching native drop")
    };
    let valid = current(renderer, transfer.node, transfer.revision)
        && transfer.live.load(Ordering::Acquire)
        && !transfer.reported
        && Instant::now() < transfer.deadline;
    if !valid {
        if !transfer.reported
            && transfer.live.load(Ordering::Acquire)
            && Instant::now() >= transfer.deadline
        {
            report(
                renderer,
                transfer.node,
                transfer.revision,
                FileRejection::Failed,
            );
        }
        renderer.0.borrow_mut().files.drop.guard = None;
        return;
    }
    // The platform API returns owned URIs. Its allocation precedes our transfer
    // budget; no second copies/paths or file opens precede the checks below.
    let raw = if value.type_().hint() == Some(TypeHint::UriList) {
        value.try_as_uris()
    } else {
        Err(std::io::ErrorKind::InvalidData.into())
    };
    let current = current(renderer, transfer.node, transfer.revision)
        && transfer.live.load(Ordering::Acquire)
        && Instant::now() < transfer.deadline;
    if !current {
        renderer.0.borrow_mut().files.drop.guard = None;
        return;
    }
    let parsed = match raw {
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Deadlock
            ) =>
        {
            renderer.0.borrow_mut().files.drop.current = Some(transfer);
            return;
        }
        Err(_) => Err(FileRejection::Type),
        Ok(uris) => paths(uris, transfer.configuration.max_files),
    };
    match parsed {
        Ok(paths) => {
            let driver = renderer
                .0
                .borrow()
                .files
                .config
                .as_ref()
                .expect("drop config")
                .dropper
                .clone()
                .expect("drop driver");
            transfer.stage = Stage::Opening {
                future: Box::pin(async move { driver.open(paths).await }),
                started: false,
            };
            renderer.0.borrow_mut().files.drop.current = Some(transfer);
        }
        Err(error) => {
            renderer.0.borrow_mut().files.drop.guard = None;
            report(renderer, transfer.node, transfer.revision, error);
        }
    }
}
pub(super) fn receive(renderer: &Renderer, event: &WindowEvent, api: &impl Transfers) {
    match event {
        WindowEvent::DragEntered { id, position } => {
            renderer.0.borrow_mut().files.drop.hover = Some(Hover {
                id: *id,
                position: *position,
            });
            let accept = position.is_some_and(|position| can_hover(renderer, position, *id, api));
            api.accept_copy(*id, accept);
        }
        WindowEvent::DragPosition { id, position, .. } => {
            let matched = {
                let mut state = renderer.0.borrow_mut();
                if let Some(hover) = &mut state.files.drop.hover
                    && hover.id == *id
                {
                    hover.position = Some(*position);
                    true
                } else {
                    false
                }
            };
            let accept = matched && can_hover(renderer, *position, *id, api);
            api.accept_copy(*id, accept);
        }
        WindowEvent::DragDropped {
            id,
            proposed_action,
        } => {
            let hover = {
                let mut state = renderer.0.borrow_mut();
                if state.files.drop.hover.as_ref().is_some_and(|h| h.id == *id) {
                    state.files.drop.hover.take()
                } else {
                    None
                }
            };
            if proposed_action.is_some_and(|action| action != DndAction::Copy) {
                api.accept_copy(*id, false);
                return;
            }
            if let Some(target) = hover
                .and_then(|h| h.position)
                .and_then(|p| hit(renderer, p))
                && api.has_uris(*id)
            {
                begin(renderer, *id, target, api);
            }
        }
        WindowEvent::DragLeft { id } => {
            let mut state = renderer.0.borrow_mut();
            if state.files.drop.hover.as_ref().is_some_and(|h| h.id == *id) {
                state.files.drop.hover = None;
            }
        }
        WindowEvent::DataTransferReceived { id, serial, value } => {
            received(renderer, *id, *serial, value)
        }
        WindowEvent::Focused(false)
        | WindowEvent::CloseRequested
        | WindowEvent::Destroyed
        | WindowEvent::SurfaceResized(_)
        | WindowEvent::ScaleFactorChanged { .. } => cancel(renderer),
        _ => {}
    }
}
pub(super) fn poll(renderer: &Renderer, cx: &mut Context<'_>) {
    let Some(mut transfer) = renderer.0.borrow_mut().files.drop.current.take() else {
        return;
    };
    let valid = current(renderer, transfer.node, transfer.revision)
        && transfer.live.load(Ordering::Acquire);
    let expired = Instant::now() >= transfer.deadline
        || transfer
            .timer
            .as_mut()
            .is_some_and(|t| Pin::new(t).poll(cx).is_ready());
    if expired && !transfer.reported {
        if valid {
            report(
                renderer,
                transfer.node,
                transfer.revision,
                FileRejection::Failed,
            );
        }
        transfer.reported = true;
        transfer.live.store(false, Ordering::Release);
    }
    if !valid || expired {
        transfer.timer = None;
    }
    let result = match &mut transfer.stage {
        Stage::Waiting { .. } => {
            renderer.0.borrow_mut().files.drop.current = Some(transfer);
            return;
        }
        Stage::Opening { future, started } => {
            if !*started && (!valid || expired) {
                renderer.0.borrow_mut().files.drop.guard = None;
                return;
            }
            *started = true;
            future.as_mut().poll(cx)
        }
    };
    match result {
        Poll::Pending => {
            renderer.0.borrow_mut().files.drop.current = Some(transfer);
        }
        Poll::Ready(result) => {
            let (document, serial) = {
                let state = renderer.0.borrow();
                (
                    state.files.document.expect("drop document"),
                    state.files.serial,
                )
            };
            let prepared = result.and_then(|files| {
                prepare(
                    files,
                    &transfer.configuration,
                    document,
                    transfer.node,
                    transfer.revision,
                    serial,
                )
            });
            let mut state = renderer.0.borrow_mut();
            state.dom.resolve();
            let mut unused = None;
            if !transfer.reported
                && transfer.live.load(Ordering::Acquire)
                && Instant::now() < transfer.deadline
                && eligible(&state, transfer.node, transfer.revision)
            {
                let outcome = match prepared {
                    Ok((files, next)) => match select(&mut state, transfer.node, files, next) {
                        Ok(outcome) => outcome,
                        Err((error, files)) => {
                            unused = Some(files);
                            FileOutcome::Rejected(error)
                        }
                    },
                    Err(error) => FileOutcome::Rejected(error),
                };
                if let Err(error) = queue(&mut state, transfer.node, transfer.revision, outcome) {
                    state.input_error = Some(error);
                }
            } else {
                unused = prepared.ok().map(|(files, _)| files);
            }
            state.files.drop.guard = None;
            sync(&mut state);
            super::sync(&mut state);
            let retired = take_retired(&mut state);
            drop(state);
            drop(unused);
            drop(retired);
            drop(transfer);
        }
    }
}

#[cfg(test)]
#[path = "file_drop_tests.rs"]
mod tests;
