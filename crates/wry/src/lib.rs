//! Native Rust UI with an incremental, document-local WebView interpreter.
//! No WASM, second reactive runtime, HTML snapshot replacement or VDOM diff.
#![cfg(not(target_arch = "wasm32"))]

mod files;
#[cfg(any(feature = "shell", test))]
mod host_inbox;
mod keys;
mod local_media;
mod protocol;
mod rich;
pub use local_media::{LocalMedia, MediaKind};
#[cfg(all(
    feature = "shell",
    any(target_os = "macos", target_os = "windows", target_os = "linux")
))]
pub mod shell;
#[cfg(test)]
mod tests;

use glory_html::{Mapper, Op, Sink};
use glory_render::{
    memory::{Memory, Subtree},
    *,
};
use protocol::{Payload, WireEvent};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

/// The interpreter is shared by the native shell and browser transport tests.
pub const INTERPRETER: &str = concat!(
    "function(config){return (",
    include_str!("interpreter.js"),
    ")(config,{",
    "createLayers:",
    include_str!("layers.js"),
    ",createKeyboard:",
    include_str!("keys.js"),
    ",createFiles:",
    include_str!("files.js"),
    ",createPlots:",
    include_str!("plots.js"),
    ",createMotion:",
    include_str!("motion.js"),
    ",createRichEditor:",
    include_str!("rich_editor.js"),
    "});}"
);
mod layers;

#[derive(Default)]
struct Live {
    queries: BTreeSet<QueryToken>,
    interests: BTreeSet<EventKind>,
}
#[derive(Clone, Copy)]
struct Pending {
    preview: Option<FileImageInfo>,
    query: Query,
    applied: u64,
}
struct Transport {
    session: String,
    commands: Vec<String>,
    bytes: usize,
    ready: bool,
    sequence: u64,
    flight: Option<u64>,
    fault: Option<Error>,
    closed: bool,
}
impl Transport {
    fn check(&self) -> Result<(), Error> {
        if self.closed {
            return Err(Error::Backend("Wry document closed".into()));
        }
        if let Some(error) = &self.fault {
            return Err(error.clone());
        }
        Ok(())
    }
    fn push(&mut self, command: Value) -> Result<(), Error> {
        self.check()?;
        let text = command.to_string();
        let bytes = self.bytes.saturating_add(text.len() + 1);
        if bytes > protocol::MAX_PENDING_BYTES {
            let error = Error::Backend("Wry pending command budget exceeded".into());
            self.fault = Some(error.clone());
            return Err(error);
        }
        self.bytes = bytes;
        self.commands.push(text);
        Ok(())
    }
}
struct State {
    files: Option<FileDocumentId>,
    local_media: bool,
    text_clipboard: bool,
    mapper: Mapper,
    transport: Transport,
    nodes: NodeMap<Live>,
    pending: BTreeMap<QueryToken, Pending>,
    inputs: Vec<Input>,
    input_bytes: usize,
    edit_receipts: BTreeMap<NodeId, Option<bool>>,
}
struct Output<'a> {
    transport: &'a mut Transport,
    nodes: &'a mut NodeMap<Live>,
    pending: &'a mut BTreeMap<QueryToken, Pending>,
    inputs: &'a mut Vec<Input>,
}
impl Sink for Output<'_> {
    fn apply(&mut self, op: Op) -> Result<(), Error> {
        match &op {
            Op::Create { id, .. } => {
                self.nodes.insert(*id, Live::default());
            }
            Op::Dispose { subtree, .. } => {
                for &id in subtree {
                    if let Some(live) = self.nodes.remove(&id) {
                        for token in live.queries {
                            if let Some(pending) = self.pending.remove(&token) {
                                self.inputs.push(Input::Response {
                                    token,
                                    applied: pending.applied,
                                    result: Err(Error::NodeGone(id)),
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        let command = protocol::op(op)?;
        self.transport.push(command)?;
        Ok(())
    }
}

/// Cloneable handle to one native semantic document, confined to its UI thread.
#[derive(Clone)]
pub struct Renderer(Rc<RefCell<State>>);
impl Renderer {
    pub fn new(runtime: RuntimeId) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let document = NEXT
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .expect("Wry document identities exhausted");
        Self(Rc::new(RefCell::new(State {
            mapper: Mapper::new(runtime),
            files: None,
            local_media: false,
            text_clipboard: false,
            transport: Transport {
                session: format!("{}.{}", runtime.value(), document),
                commands: Vec::new(),
                bytes: 0,
                ready: false,
                sequence: 0,
                flight: None,
                fault: None,
                closed: false,
            },
            nodes: NodeMap::default(),
            pending: BTreeMap::new(),
            inputs: Vec::new(),
            input_bytes: 0,
            edit_receipts: BTreeMap::new(),
        })))
    }
    /// Explicit write-only clipboard opt-in, before shell initialization. The platform may still refuse a
    /// request; completion carries the actual outcome and original revision.
    pub fn enable_text_clipboard(&self) {
        self.0.borrow_mut().text_clipboard = true;
    }
    /// Opt in before mounting widgets to file handles explicitly selected or
    /// dropped by the user. No host paths or filesystem authority are exposed.
    pub fn enable_files(&self) -> Result<(), Error> {
        let mut state = self.0.borrow_mut();
        if state.files.is_none() {
            state.files = Some(FileDocumentId::new()?);
        }
        Ok(())
    }
    pub fn session(&self) -> String {
        self.0.borrow().transport.session.clone()
    }
    /// Bind the interpreter to this consumer, including two consumers of one RuntimeId.
    pub fn initialization_script(&self) -> String {
        format!(
            "({INTERPRETER})({});",
            json!({"v":protocol::VERSION,"session":self.session(),"maxIpcBytes":protocol::MAX_IPC_BYTES,"keyBindings":keys::profiles(),"maxDeferredBytes":protocol::MAX_PENDING_BYTES,"textClipboard":self.0.borrow().text_clipboard,"files":self.0.borrow().files.is_some()})
        )
    }
    /// Take at most one ordered batch; the WebView must acknowledge it before
    /// another is sent. Acknowledgement means DOM execution, never presentation.
    pub fn take_script(&self) -> Result<Option<String>, Error> {
        let mut state = self.0.borrow_mut();
        state.transport.check()?;
        if !state.transport.ready || state.transport.flight.is_some() {
            return Ok(None);
        }
        // After App has consumed the proposal and committed bindings, acknowledge
        // the current program barrier. Never execute callbacks from transport IO.
        let receipts: Vec<_> = state
            .edit_receipts
            .iter()
            .filter_map(|(id, accepted)| accepted.map(|accepted| (*id, accepted)))
            .collect();
        for (id, accepted) in receipts {
            state.edit_receipts.remove(&id);
            let editor = state.mapper.tree().node(id).ok().and_then(|node| {
                match node.descriptor.view().props {
                    Props::Input(p) => Some((p.revision, (!accepted).then(|| json!(p.value)))),
                    Props::RichEditor(p) => {
                        Some((p.revision, (!accepted).then(|| rich::document(&p.document))))
                    }
                    _ => None,
                }
            });
            let mut command = json!({"op":"input_done","id":protocol::id(id)});
            if let Some((revision, reset)) = editor {
                command["revision"] = json!(revision.to_string());
                command["reset"] = json!(reset.is_some());
                if let Some(value) = reset {
                    command["value"] = value;
                }
            } else {
                command["retired"] = json!(true);
            }
            state.transport.push(command)?;
        }
        let transport = &mut state.transport;
        if transport.commands.is_empty() {
            return Ok(None);
        }
        transport.sequence = transport
            .sequence
            .checked_add(1)
            .ok_or(Error::InvalidProps("Wry batch sequence exhausted"))?;
        transport.flight = Some(transport.sequence);
        let commands = std::mem::take(&mut transport.commands).join(",");
        transport.bytes = 0;
        Ok(Some(format!(
            "window.__gloryWryReceive({{\"v\":{},\"session\":{},\"sequence\":{},\"commands\":[{}]}});",
            protocol::VERSION,
            json!(transport.session),
            json!(transport.sequence.to_string()),
            commands
        )))
    }
    pub fn in_flight(&self) -> bool {
        self.0.borrow().transport.flight.is_some()
    }
    pub fn close(&self) {
        let mut state = self.0.borrow_mut();
        state.transport.closed = true;
        state.transport.commands.clear();
        state.edit_receipts.clear();
        state.transport.bytes = 0;
        state.transport.flight = None;
        let pending = std::mem::take(&mut state.pending);
        for (token, pending) in pending {
            if let Some(live) = state.nodes.get_mut(&pending.query.node()) {
                live.queries.remove(&token);
            }
            state.inputs.push(Input::Response {
                token,
                applied: pending.applied,
                result: Err(Error::Backend("Wry document closed".into())),
            });
        }
    }
    /// Only enqueue observations. Business handlers execute later in App::pump.
    /// Old documents/retired identities/duplicate replies cannot target new UI.
    pub fn receive(&self, message: &str) -> Result<bool, Error> {
        if message.len() > protocol::MAX_IPC_BYTES {
            return Err(Error::InvalidEvent("Wry IPC byte budget"));
        }
        let incoming: protocol::Incoming = serde_json::from_str(message)
            .map_err(|e| Error::Backend(format!("invalid Wry IPC: {e}")))?;
        let mut state = self.0.borrow_mut();
        if state.transport.closed || incoming.session != state.transport.session {
            return Ok(false);
        }
        state.transport.check()?;
        if incoming.v != protocol::VERSION {
            return Err(Error::InvalidEvent("Wry protocol version"));
        }
        if matches!(
            incoming.payload,
            Payload::Files { .. } | Payload::Event { .. } | Payload::Response { .. }
        ) && state.input_bytes.saturating_add(message.len()) > protocol::MAX_PENDING_BYTES
        {
            return Err(Error::InvalidEvent("Wry queued input byte budget"));
        }
        match incoming.payload {
            Payload::Files { observation } => {
                return observation.receive(&mut state, message.len());
            }
            Payload::Ready => {
                if state.transport.ready {
                    return Err(Error::InvalidEvent("Wry document restarted"));
                }
                state.transport.ready = true;
            }
            Payload::Ack { sequence } => {
                let sequence = protocol::integer(&sequence)?;
                if state.transport.flight != Some(sequence) {
                    return Err(Error::InvalidResponse("Wry batch acknowledgement"));
                }
                state.transport.flight = None;
            }
            Payload::LayerBounds {
                node,
                request,
                anchor,
                panel,
                viewport,
            } => {
                let Some(id) = state.nodes.id_for_token(&node) else {
                    return Ok(false);
                };
                let tree = state.mapper.tree();
                let Some(layer) = tree
                    .node(id)?
                    .descriptor
                    .view()
                    .semantics
                    .as_ref()
                    .filter(|s| !s.hidden && tree.active(id) && !tree.inert(id))
                    .and_then(|s| s.layer.as_ref())
                else {
                    return Ok(false);
                };
                if let Some(command) = layers::position(
                    id,
                    protocol::integer(&request)?,
                    layer,
                    anchor,
                    panel,
                    viewport,
                )? {
                    state.transport.push(command)?;
                }
            }
            Payload::Fault { message } => {
                let error = Error::Backend(format!("Wry interpreter: {message}"));
                state.transport.fault = Some(error.clone());
                return Err(error);
            }
            Payload::Response {
                token,
                value,
                error,
            } => {
                let token = QueryToken(protocol::integer(&token)?);
                let Some(mut pending) = state.pending.remove(&token) else {
                    return Ok(false);
                };
                let result = if error.is_some() {
                    Err(Error::Unsupported("Wry physical query unavailable"))
                } else {
                    let value = value.unwrap_or(Value::Null);
                    if matches!(pending.query, Query::FileRead(_) | Query::FilePreview(_)) {
                        match files::response(pending, value, &state) {
                            Ok(files::Completion::Done(value)) => Ok(value),
                            Ok(files::Completion::Preview(image)) => {
                                let Query::FilePreview(file) = pending.query else {
                                    unreachable!()
                                };
                                let cmd = json!({"op":"query","kind":"file_preview_url","id":protocol::id(file.node),"serial":file.serial.to_string(),"revision":file.revision.to_string(),"token":token.0.to_string()});
                                state.transport.push(cmd)?;
                                pending.preview = Some(image);
                                state.pending.insert(token, pending);
                                return Ok(true);
                            }
                            Err(error) => Err(error),
                        }
                    } else {
                        response(pending.query, value)
                    }
                };
                if let Some(live) = state.nodes.get_mut(&pending.query.node()) {
                    live.queries.remove(&token);
                }
                state.inputs.push(Input::Response {
                    token,
                    applied: pending.applied,
                    result,
                });
                state.input_bytes += message.len();
            }
            Payload::Event { node, event } => {
                let Some(node) = state.nodes.id_for_token(&node) else {
                    return Ok(false);
                };
                let from = if let WireEvent::FormSubmit {
                    submitter: Some(source),
                }
                | WireEvent::KeyBinding {
                    from: Some(source), ..
                }
                | WireEvent::TreeSearch { from: source, .. } = &event
                {
                    let Some(source) = state.nodes.id_for_token(source) else {
                        return Ok(false);
                    };
                    source
                } else if let WireEvent::Drag {
                    from: Some(source),
                    phase,
                    ..
                } = &event
                {
                    let Some(source) = state.nodes.id_for_token(source) else {
                        return Ok(false);
                    };
                    if phase != "drop"
                        || !state
                            .mapper
                            .tree()
                            .node(source)?
                            .descriptor
                            .view()
                            .semantics
                            .as_ref()
                            .and_then(|s| s.drag_capture)
                            .is_some_and(glory_render::DragCapture::is_threshold)
                    {
                        return Err(Error::InvalidEvent(
                            "Wry hit-tested drop requires a threshold drag source",
                        ));
                    }
                    source
                } else {
                    node
                };
                let related = if let WireEvent::Pointer {
                    related: Some(key), ..
                } = &event
                {
                    let Some(id) = state.nodes.id_for_token(key) else {
                        return Ok(false);
                    };
                    Some(id)
                } else {
                    None
                };
                let sheet = if matches!(&event, WireEvent::Sheet { .. }) {
                    state
                        .mapper
                        .tree()
                        .node(node)?
                        .descriptor
                        .view()
                        .semantics
                        .as_ref()
                        .and_then(|s| s.layer.as_ref())
                        .and_then(|layer| layer.sheet.as_ref())
                } else {
                    None
                };
                let mut event = event.into_event(node, from, sheet)?;
                if let Event::Pointer { related: other, .. } = &mut event {
                    *other = related;
                }
                let editing = matches!(
                    event,
                    Event::ValueChanged { .. } | Event::InputEdit(_) | Event::RichEdit(_)
                );
                if editing {
                    if state.edit_receipts.len() >= protocol::MAX_INPUTS
                        && !state.edit_receipts.contains_key(&node)
                    {
                        return Err(Error::InvalidEvent("Wry edit receipt budget"));
                    }
                    state.edit_receipts.insert(node, Some(false));
                }
                if matches!(event, Event::RichEdit(_)) {
                    let descriptor = state.mapper.tree().node(node)?.descriptor.view();
                    if descriptor.readonly() {
                        // A valid edit may have been in flight when Rust made the editor readonly.
                        event.validate_payload(node, descriptor)?;
                        return Ok(false);
                    }
                }
                if !state.mapper.tree().validate_event(node, &event)? {
                    return Ok(false);
                }
                if let Event::Clipboard {
                    request, revision, ..
                } = &event
                    && !state.mapper.finish_clipboard(node, *request, *revision)?
                {
                    return Ok(false);
                }
                if !state.mapper.tree().active(node) && !event.is_terminal_loss() {
                    return Ok(false);
                }
                let descriptor = state.mapper.tree().node(node)?.descriptor.view();
                let edit_revision = match &event {
                    Event::ValueChanged { revision, .. } | Event::Commit { revision, .. } => {
                        Some(*revision)
                    }
                    Event::InputEdit(edit) => Some(edit.revision()),
                    _ => None,
                };
                if let Some(revision) = edit_revision {
                    let Props::Input(props) = descriptor.props else {
                        return Err(Error::InvalidEvent("Wry edit target"));
                    };
                    if revision < props.revision {
                        return Ok(false);
                    }
                    if revision > props.revision {
                        return Err(Error::InputRevision {
                            current: props.revision,
                            incoming: revision,
                        });
                    }
                }
                if state.inputs.len().saturating_add(state.pending.len()) >= protocol::MAX_INPUTS {
                    return Err(Error::InvalidEvent("Wry input queue budget"));
                }
                // Coalesce only adjacent geometry observations, preserving input order.
                if matches!(
                    event,
                    Event::Resize { .. }
                        | Event::Scroll { .. }
                        | Event::Drag {
                            phase: glory_render::DragPhase::Move,
                            ..
                        }
                ) && let Some(Input::Event {
                    node: previous,
                    event: old,
                }) = state.inputs.last_mut()
                    && *previous == node
                    && old.kind() == event.kind()
                    && !matches!(old,Event::Drag{phase,..} if *phase!=glory_render::DragPhase::Move)
                {
                    *old = event;
                    return Ok(true);
                }
                if editing {
                    state.edit_receipts.insert(node, None);
                }
                state.inputs.push(Input::Event { node, event });
                state.input_bytes += message.len();
            }
        }
        Ok(true)
    }
    /// Replace this document's trusted application stylesheet without remounting
    /// widgets or disturbing focus/selection. This is the same author CSS layer
    /// as `shell::Config::css`; it is not HTML and does not grant network access.
    /// The shell's content security policy and explicit local assets still apply.
    /// The update follows the normal ready/ack ordering and pending-byte budget.
    pub fn set_author_css(&self, css: &str) -> Result<(), Error> {
        if css.len() > 1024 * 1024 {
            return Err(Error::InvalidProps("Wry author stylesheet exceeds 1 MiB"));
        }
        self.0
            .borrow_mut()
            .transport
            .push(json!({"op":"css","value":css}))
    }
    #[cfg(feature = "css")]
    pub fn set_sheet(&self, sheet: &glory_html::Sheet) -> Result<(), Error> {
        let css = glory_html::sheet(sheet, style::Profile::HTML)
            .map_err(|error| Error::Backend(error.to_string()))?;
        self.set_author_css(&css)
    }
}

// Refuse host behavior that has not been implemented, before staged UI changes.
fn preflight(node: DescriptorView<'_>, patch: Option<&Patch>) -> Result<(), Error> {
    glory_html::preflight(node, patch)?;
    if node
        .options
        .get::<glory_html::display::Placement>()
        .is_some()
    {
        return Err(Error::Unsupported("Wry sparse display templates"));
    }
    Ok(())
}
fn supported_event(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::Activate
            | EventKind::Submit
            | EventKind::Reset
            | EventKind::ValueChanged
            | EventKind::RichEdit
            | EventKind::RichSelection
            | EventKind::RichRejected
            | EventKind::RichCommand
            | EventKind::Files
            | EventKind::InputEdit
            | EventKind::Commit
            | EventKind::SelectChanged
            | EventKind::CheckedChanged
            | EventKind::Composition
            | EventKind::FocusChanged
            | EventKind::FocusWithin
            // Anchor is a passive SSR handover notification. A fresh Wry
            // document starts at its declared scroll position.
            | EventKind::Anchor
            | EventKind::Resize
            | EventKind::Scroll
            | EventKind::Keyboard
            | EventKind::Navigate
            | EventKind::GridNavigate
            | EventKind::TreeNavigate
            | EventKind::ScrollKeyboard
            | EventKind::Spin
            | EventKind::Choice
            | EventKind::Tags
            | EventKind::Palette
            | EventKind::NavigationPanel
            | EventKind::ColorSchemeChanged
            | EventKind::Clipboard
            | EventKind::ImageStatus
            | EventKind::Drag
            | EventKind::Range
            | EventKind::CollapseComplete
            | EventKind::PresenceComplete
            | EventKind::Sheet
            | EventKind::Swipe
            | EventKind::ContextMenu
            | EventKind::Dismiss
            | EventKind::Menu
            | EventKind::Typeahead
            | EventKind::Pointer
            | EventKind::PlotPointer
            | EventKind::PointerWithin
    )
}
impl Render for Renderer {
    fn preflight(&self, node: DescriptorView<'_>, patch: Option<&Patch>) -> Result<(), Error> {
        preflight(node, patch)
    }
    fn preflight_place(
        &self,
        tree: &Memory,
        child: Subtree<'_>,
        parent: Option<NodeId>,
    ) -> Result<(), Error> {
        glory_html::preflight_place(tree, child, parent)
    }
    fn supports_plot_pointer(&self) -> bool {
        true
    }
    fn style_profile(&self) -> style::Profile {
        style::Profile::HTML
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            measure: true,
            scroll: true,
            focus: true,
            input: true,
            collapse_completion: true,
            presence_completion: true,
            presence_transforms: true,
            clipboard_write: self.0.borrow().text_clipboard,
            files: self.0.borrow().files.map(|document| FileCapabilities {
                document,
                picker: true,
                drop: true,
                read: true,
                preview: true,
            }),
        }
    }
    fn listen(&mut self, node: NodeId, kind: EventKind, enabled: bool) -> Result<(), Error> {
        let mut state = self.0.borrow_mut();
        let descriptor = state.mapper.tree().node(node)?.descriptor.view();
        if enabled {
            kind.validate_target(descriptor)?;
            if !supported_event(kind) {
                return Err(Error::Unsupported("Wry event"));
            }
        }
        state.transport.push(json!({"op":"listen","id":protocol::id(node),"kind":format!("{kind:?}"),"enabled":enabled}))?;
        let live = state.nodes.get_mut(&node).ok_or(Error::NodeGone(node))?;
        if enabled {
            live.interests.insert(kind);
        } else {
            live.interests.remove(&kind);
        }
        Ok(())
    }
    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, Error> {
        let mut state = self.0.borrow_mut();
        state.transport.check()?;
        for mutation in mutations {
            match mutation {
                Mutation::Create { node, .. } => preflight(node.view(), None)?,
                Mutation::CreatePlan { plan, .. } => {
                    for node in plan.nodes() {
                        preflight(node.descriptor.view(), None)?;
                    }
                }
                Mutation::Patch { id, patch } => preflight(
                    state.mapper.tree().node(*id)?.descriptor.view(),
                    Some(patch),
                )?,
                _ => {}
            }
            let State {
                mapper,
                transport,
                nodes,
                pending,
                inputs,
                ..
            } = &mut *state;
            mapper.apply(
                std::slice::from_ref(mutation),
                &mut Output {
                    transport,
                    nodes,
                    pending,
                    inputs,
                },
            )?;
            let changed: &[NodeId] = match mutation {
                Mutation::Create { id, .. } | Mutation::Patch { id, .. } => {
                    std::slice::from_ref(id)
                }
                Mutation::CreatePlan { ids, .. } => ids,
                _ => &[],
            };
            for &id in changed {
                let revision = state
                    .mapper
                    .tree()
                    .node(id)?
                    .descriptor
                    .view()
                    .semantics
                    .as_ref()
                    .and_then(|s| s.clipboard.as_ref())
                    .map(|s| s.revision);
                if let Some(revision) = revision {
                    state.transport.push(json!({"op":"clipboard_source","id":protocol::id(id),"revision":revision.to_string()}))?;
                }
            }
        }
        Ok(state.mapper.receipt())
    }
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error> {
        let mut state = self.0.borrow_mut();
        state.transport.check()?;
        if state.pending.contains_key(&token) {
            return Err(Error::InvalidResponse("duplicate Wry query token"));
        }
        if state.pending.len() >= protocol::MAX_QUERIES
            || state.inputs.len().saturating_add(state.pending.len()) >= protocol::MAX_INPUTS
        {
            return Err(Error::Unsupported("Wry query budget"));
        }
        let result = state
            .mapper
            .tree()
            .node(query.node())
            .and_then(|node| query.validate_target(node.descriptor.view()))
            .and_then(|()| match query {
                Query::FileRead(_) | Query::FilePreview(_) => files::query(query, &state).map(Some),
                _ => Ok(None),
            });
        let applied = state.mapper.receipt().applied;
        let mut command = match result {
            Ok(Some(command)) => command,
            Ok(None) => json!({"kind":match query {
                Query::RichSelection(_) => "rich_selection",
                Query::Bounds(_) => "bounds",
                Query::Focus(_) => "focus",
                Query::Scroll(_) => "scroll",
                Query::ScrollMetrics(_) => "scroll_metrics",
                Query::Selection(_) => "selection",
                Query::FileRead(_)|Query::FilePreview(_)=>unreachable!(),
            }}),
            Err(error) => {
                state.inputs.push(Input::Response {
                    token,
                    applied,
                    result: Err(error),
                });
                return Ok(());
            }
        };
        command["id"] = json!(protocol::id(query.node()));
        command["op"] = json!("query");
        command["token"] = json!(token.0.to_string());
        state.transport.push(command)?;
        state.pending.insert(
            token,
            Pending {
                query,
                applied,
                preview: None,
            },
        );
        state
            .nodes
            .get_mut(&query.node())
            .expect("live mapper node")
            .queries
            .insert(token);
        Ok(())
    }
    fn action(&mut self, action: Action) -> Result<(), Error> {
        let mut state = self.0.borrow_mut();
        let descriptor = state.mapper.tree().node(action.node())?.descriptor.view();
        action.validate_target(descriptor)?;
        let tree = state.mapper.tree();
        match action {
            Action::RevealOption { node } if !tree.active(node) || !tree.enabled(node) => {
                return Ok(());
            }
            // A controlled menu commonly closes in the same commit that
            // requests Tab exit. Its retained declaration still owns the
            // return-focus relation, even when the panel is already hidden.
            Action::ExitMenu { node, .. } if tree.inert(node) => {
                return Err(Error::Unsupported("menu exit in an inert region"));
            }
            Action::ExitMenu { .. } => {}
            _ if !tree.active(action.node()) => {
                return Err(Error::Unsupported("Wry inactive action target"));
            }
            _ => {}
        }
        let value = match action {
            Action::PickFiles { .. } | Action::ClearFiles { .. } | Action::RemoveFile(_) => {
                files::action(action, &mut state)?
            }
            Action::CopyText {
                revision, request, ..
            } => {
                if !state.text_clipboard {
                    return Err(Error::Unsupported(
                        "Wry text clipboard requires explicit host opt-in",
                    ));
                }
                let text = descriptor
                    .semantics
                    .as_ref()
                    .and_then(|s| s.clipboard.as_ref())
                    .ok_or(Error::Unsupported("No clipboard source"))?
                    .text
                    .clone();
                state.mapper.admit_clipboard(action)?;
                json!({"kind":"copy_text","revision":revision.to_string(),"request":request.to_string(),"text":text})
            }
            Action::RaiseLayer(_) => json!({"kind":"raise_layer"}),
            Action::SelectRich {
                revision,
                selection,
                ..
            } => {
                json!({"kind":"rich_selection","revision":revision.to_string(),"selection":rich::selection(selection)})
            }
            Action::Focus(_) => json!({"kind":"focus"}),
            Action::ScrollTo { x, y, .. } => json!({"kind":"scroll","x":x,"y":y}),
            Action::ExitMenu { backwards, .. } => json!({"kind":"exit_menu","backwards":backwards}),
            Action::RevealOption { .. } => json!({"kind":"reveal_option"}),
            Action::ScrollIntoView(_) => json!({"kind":"reveal"}),
            Action::SelectText {
                revision,
                selection,
                ..
            } => {
                let Props::Input(props) = descriptor.props else {
                    unreachable!()
                };
                if props.revision != revision {
                    return Err(Error::InputRevision {
                        current: props.revision,
                        incoming: revision,
                    });
                }
                selection.byte_range(&props.value)?;
                json!({"kind":"selection","revision":revision.to_string(),"start":selection.start,"end":selection.end,"direction":selection.direction.as_str()})
            }
        };
        state
            .transport
            .push(json!({"op":"action","id":protocol::id(action.node()),"action":value}))
    }
    fn drain(&mut self, inputs: &mut Vec<Input>) {
        let mut state = self.0.borrow_mut();
        let edits: Vec<_> = state
            .inputs
            .iter()
            .filter_map(|input| match input {
                Input::Event {
                    node,
                    event: Event::ValueChanged { .. } | Event::InputEdit(_) | Event::RichEdit(_),
                } => Some(*node),
                _ => None,
            })
            .collect();
        for node in edits {
            state.edit_receipts.insert(node, Some(true));
        }
        inputs.append(&mut state.inputs);
        state.input_bytes = 0;
    }
}

fn drag_phase(phase: &str) -> Result<glory_render::DragPhase, Error> {
    Ok(match phase {
        "start" => glory_render::DragPhase::Start,
        "move" => glory_render::DragPhase::Move,
        "drop" => glory_render::DragPhase::Drop,
        "cancel" => glory_render::DragPhase::Cancel,
        _ => return Err(Error::InvalidEvent("Wry drag phase")),
    })
}
impl WireEvent {
    fn into_event(
        self,
        node: NodeId,
        drag_source: NodeId,
        sheet: Option<&glory_render::SheetConfig>,
    ) -> Result<Event, Error> {
        Ok(match self {
            Self::Clipboard {
                request,
                revision,
                outcome,
            } => Event::Clipboard {
                request: protocol::integer(&request)?,
                revision: protocol::integer(&revision)?,
                outcome: match outcome.as_str() {
                    "written" => ClipboardOutcome::Written,
                    "denied" => ClipboardOutcome::Denied,
                    "unavailable" => ClipboardOutcome::Unavailable,
                    "failed" => ClipboardOutcome::Failed,
                    _ => return Err(Error::InvalidEvent("Wry clipboard outcome")),
                },
            },
            Self::RichEdit { snapshot } => Event::RichEdit(snapshot.decode()?),
            Self::RichSelection { snapshot } => Event::RichSelection(snapshot.decode()?),
            Self::RichRejected { revision } => Event::RichRejected {
                revision: protocol::integer(&revision)?,
            },
            Self::RichCommand { revision, command } => Event::RichCommand {
                revision: protocol::integer(&revision)?,
                command: match command.as_str() {
                    "undo" => RichCommand::Undo,
                    "redo" => RichCommand::Redo,
                    "bold" => RichCommand::Bold,
                    "italic" => RichCommand::Italic,
                    "code" => RichCommand::Code,
                    _ => return Err(Error::InvalidEvent("Wry rich command")),
                },
            },
            Self::Activate => Event::Activate,
            Self::FormSubmit { submitter } => Event::Submit {
                submitter: submitter.map(|_| drag_source),
            },
            Self::FormReset => Event::Reset,
            Self::KeyBinding { binding, from } => {
                keys::event(binding, drag_source, from.is_some())?
            }
            Self::TreeSearch {
                text, at_millis, ..
            } => Event::TreeNavigate {
                from: drag_source,
                command: glory_render::TreeCommand::Search { text, at_millis },
            },
            Self::Palette => Event::Palette { from: node },
            Self::Tags { revision, data } => Event::Tags {
                from: node,
                revision: protocol::integer(&revision)?,
                command: data.into(),
            },
            Self::Value {
                value,
                composing,
                revision,
            } => Event::ValueChanged {
                value,
                composing,
                revision: protocol::integer(&revision)?,
            },
            Self::InputEdit { revision, edit } => {
                Event::InputEdit(edit.decode(protocol::integer(&revision)?))
            }
            Self::Commit { value, revision } => Event::Commit {
                value,
                revision: protocol::integer(&revision)?,
            },
            Self::Select { keys, revision } => {
                if keys.len() > MAX_SELECT_OPTIONS || keys.iter().any(|key| key.len() > 64) {
                    return Err(Error::InvalidEvent("Wry select key budget"));
                }
                Event::SelectChanged {
                    keys: keys
                        .into_iter()
                        .map(SemanticId::new)
                        .collect::<Result<_, _>>()?,
                    revision: protocol::integer(&revision)?,
                }
            }
            Self::Checked { value } => Event::CheckedChanged(value),
            Self::Composition { active } => Event::Composition { active },
            Self::Focus { focused } => Event::FocusChanged(focused),
            Self::Resize {
                width,
                height,
                viewport_width,
                viewport_height,
            } => Event::Resize {
                size: Rect {
                    x: 0.,
                    y: 0.,
                    width,
                    height,
                },
                viewport: Rect {
                    x: 0.,
                    y: 0.,
                    width: viewport_width,
                    height: viewport_height,
                },
            },
            Self::Scroll {
                x,
                y,
                viewport_width,
                viewport_height,
                ack,
            } => Event::Scroll {
                x,
                y,
                viewport: Rect {
                    x: 0.,
                    y: 0.,
                    width: viewport_width,
                    height: viewport_height,
                },
                ack: ack.map(|a| ScrollAck {
                    requested: ScrollRequest {
                        x: a.requested_x,
                        y: a.requested_y,
                    },
                    x: a.x,
                    y: a.y,
                }),
            },
            Self::ColorScheme { dark } => Event::ColorSchemeChanged { dark },
            Self::Image { revision, loaded } => Event::ImageStatus {
                revision: protocol::integer(&revision)?,
                status: if loaded {
                    ImageStatus::Loaded
                } else {
                    ImageStatus::Failed
                },
            },
            Self::Sheet {
                revision,
                viewport,
                extent,
            } => Event::Sheet {
                from: node,
                revision: protocol::integer(&revision)?,
                command: sheet
                    .ok_or(Error::InvalidEvent("Wry sheet release requires a sheet"))?
                    .nearest(viewport, extent)?,
            },
            Self::CollapseComplete { revision } => Event::CollapseComplete {
                revision: protocol::integer(&revision)?,
            },
            Self::PresenceComplete { revision } => Event::PresenceComplete {
                revision: protocol::integer(&revision)?,
            },
            Self::Swipe {
                phase,
                revision,
                x,
                y,
            } => Event::Swipe {
                from: node,
                phase: drag_phase(&phase)?,
                revision: protocol::integer(&revision)?,
                x,
                y,
            },
            Self::Drag { phase, x, y, .. } => Event::Drag {
                from: drag_source,
                x,
                y,
                phase: drag_phase(&phase)?,
            },
            Self::Keyboard { command } => Event::Keyboard {
                from: node,
                command: match command.as_str() {
                    "submit" => KeyboardCommand::Submit,
                    "palette" => KeyboardCommand::Palette,
                    "search" => KeyboardCommand::Search,
                    "help" => KeyboardCommand::Help,
                    "dismiss" => KeyboardCommand::Dismiss,
                    _ => return Err(Error::InvalidEvent("Wry keyboard command")),
                },
            },
            Self::ContextMenu { x, y, keyboard } => Event::ContextMenu {
                from: node,
                point: glory_render::ContextPoint { x, y },
                source: if keyboard {
                    glory_render::ContextSource::Keyboard
                } else {
                    glory_render::ContextSource::Pointer
                },
            },
            Self::Dismiss { reason } => Event::Dismiss(match reason.as_str() {
                "escape" => glory_render::DismissReason::Escape,
                "outside" => glory_render::DismissReason::Outside,
                "anchor_lost" => glory_render::DismissReason::AnchorLost,
                _ => return Err(Error::InvalidEvent("Wry dismissal reason")),
            }),
            Self::Typeahead { text, at_millis } => Event::Typeahead {
                from: node,
                text,
                at_millis,
            },
            Self::PlotPointer {
                pointer_id,
                device,
                phase,
                x,
                y,
                width,
                height,
            } => {
                let pointer = PlotPointer {
                    pointer_id,
                    x,
                    y,
                    viewport: Rect {
                        width,
                        height,
                        ..Rect::default()
                    },
                    device: match device.as_str() {
                        "mouse" => PlotPointerDevice::Mouse,
                        "pen" => PlotPointerDevice::Pen,
                        "touch" => PlotPointerDevice::Touch,
                        _ => return Err(Error::InvalidEvent("Wry plot pointer device")),
                    },
                    phase: match phase.as_str() {
                        "down" => PlotPointerPhase::Down,
                        "up" => PlotPointerPhase::Up,
                        "move" => PlotPointerPhase::Move,
                        "leave" => PlotPointerPhase::Leave,
                        "cancel" => PlotPointerPhase::Cancel,
                        _ => return Err(Error::InvalidEvent("Wry plot pointer phase")),
                    },
                };
                pointer.validate()?;
                Event::PlotPointer {
                    from: node,
                    pointer,
                }
            }
            Self::Pointer { phase, .. } => Event::Pointer {
                from: node,
                related: None,
                phase: match phase.as_str() {
                    "enter" => glory_render::PointerPhase::Enter,
                    "leave" => glory_render::PointerPhase::Leave,
                    "move" => glory_render::PointerPhase::Move,
                    _ => return Err(Error::InvalidEvent("Wry pointer phase")),
                },
            },
            Self::Navigate { command } => Event::Navigate {
                from: node,
                command: match command.as_str() {
                    "previous" => Navigation::Previous,
                    "next" => Navigation::Next,
                    "first" => Navigation::First,
                    "last" => Navigation::Last,
                    "page_previous" => Navigation::PagePrevious,
                    "page_next" => Navigation::PageNext,
                    _ => return Err(Error::InvalidEvent("Wry navigation command")),
                },
            },
        })
    }
}
fn response(query: Query, value: Value) -> Result<QueryValue, Error> {
    let number = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_f64)
            .ok_or(Error::InvalidResponse("Wry query number"))
    };
    let result = match query {
        Query::RichSelection(_) => QueryValue::RichSelection(
            serde_json::from_value::<rich::Snapshot>(value.clone())
                .map_err(|_| Error::InvalidResponse("invalid Wry rich selection"))?
                .decode()?,
        ),
        Query::Bounds(_) => QueryValue::Bounds(Rect {
            x: number("x")?,
            y: number("y")?,
            width: number("width")?,
            height: number("height")?,
        }),
        Query::Focus(_) => QueryValue::Focus(
            value
                .as_bool()
                .ok_or(Error::InvalidResponse("Wry focus result"))?,
        ),
        Query::Scroll(_) => QueryValue::Scroll {
            x: number("x")?,
            y: number("y")?,
        },
        Query::ScrollMetrics(_) => QueryValue::ScrollMetrics(ScrollMetrics {
            x: number("x")?,
            y: number("y")?,
            viewport_width: number("viewport_width")?,
            viewport_height: number("viewport_height")?,
            content_width: number("content_width")?,
            content_height: number("content_height")?,
        }),
        Query::Selection(_) => {
            let integer = |key: &str| {
                value
                    .get(key)
                    .and_then(Value::as_u64)
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or(Error::InvalidResponse("Wry selection offset"))
            };
            QueryValue::Selection {
                start: integer("start")?,
                end: integer("end")?,
            }
        }
        _ => return Err(Error::Unsupported("Wry query")),
    };
    query.validate_response(&result)?;
    Ok(result)
}
