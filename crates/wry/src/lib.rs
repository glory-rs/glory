//! Native Rust UI with an incremental, document-local WebView interpreter.
//! No WASM, second reactive runtime, HTML snapshot replacement or VDOM diff.
#![cfg(not(target_arch = "wasm32"))]

mod protocol;
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
pub const INTERPRETER: &str = include_str!("interpreter.js");

#[derive(Default)]
struct Live {
    queries: BTreeSet<QueryToken>,
    interests: BTreeSet<EventKind>,
}
#[derive(Clone, Copy)]
struct Pending {
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
    mapper: Mapper,
    transport: Transport,
    nodes: NodeMap<Live>,
    pending: BTreeMap<QueryToken, Pending>,
    inputs: Vec<Input>,
    input_bytes: usize,
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
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .expect("Wry document identities exhausted");
        Self(Rc::new(RefCell::new(State {
            mapper: Mapper::new(runtime),
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
        })))
    }
    pub fn session(&self) -> String {
        self.0.borrow().transport.session.clone()
    }
    /// Bind the interpreter to this consumer, including two consumers of one RuntimeId.
    pub fn initialization_script(&self) -> String {
        format!(
            "({INTERPRETER})({});",
            json!({"v":protocol::VERSION,"session":self.session()})
        )
    }
    /// Take at most one ordered batch; the WebView must acknowledge it before
    /// another is sent. Acknowledgement means DOM execution, never presentation.
    pub fn take_script(&self) -> Result<Option<String>, Error> {
        let mut state = self.0.borrow_mut();
        let transport = &mut state.transport;
        transport.check()?;
        if !transport.ready || transport.flight.is_some() || transport.commands.is_empty() {
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
            Payload::Event { .. } | Payload::Response { .. }
        ) && state.input_bytes.saturating_add(message.len()) > protocol::MAX_PENDING_BYTES
        {
            return Err(Error::InvalidEvent("Wry queued input byte budget"));
        }
        match incoming.payload {
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
                let Some(pending) = state.pending.remove(&token) else {
                    return Ok(false);
                };
                if let Some(live) = state.nodes.get_mut(&pending.query.node()) {
                    live.queries.remove(&token);
                }
                let result = if error.is_some() {
                    Err(Error::Unsupported("Wry physical query unavailable"))
                } else {
                    response(pending.query, value.unwrap_or(Value::Null))
                };
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
                let event = event.into_event(node)?;
                if !state.mapper.tree().validate_event(node, &event)? {
                    return Ok(false);
                }
                if !state.mapper.tree().active(node) && !event.is_terminal_loss() {
                    return Ok(false);
                }
                let descriptor = state.mapper.tree().node(node)?.descriptor.view();
                if let Event::ValueChanged { revision, .. } | Event::Commit { revision, .. } =
                    &event
                {
                    let Props::Input(props) = descriptor.props else {
                        return Err(Error::InvalidEvent("Wry edit target"));
                    };
                    if *revision < props.revision {
                        return Ok(false);
                    }
                    if *revision > props.revision {
                        return Err(Error::InputRevision {
                            current: props.revision,
                            incoming: *revision,
                        });
                    }
                }
                if state.inputs.len().saturating_add(state.pending.len()) >= protocol::MAX_INPUTS {
                    return Err(Error::InvalidEvent("Wry input queue budget"));
                }
                // Coalesce only adjacent geometry observations, preserving input order.
                if matches!(event, Event::Resize { .. } | Event::Scroll { .. })
                    && let Some(Input::Event {
                        node: previous,
                        event: old,
                    }) = state.inputs.last_mut()
                    && *previous == node
                    && old.kind() == event.kind()
                {
                    *old = event;
                    return Ok(true);
                }
                state.inputs.push(Input::Event { node, event });
                state.input_bytes += message.len();
            }
        }
        Ok(true)
    }
    #[cfg(feature = "css")]
    pub fn set_sheet(&self, sheet: &glory_html::Sheet) -> Result<(), Error> {
        let css = glory_html::sheet(sheet, style::Profile::HTML)
            .map_err(|error| Error::Backend(error.to_string()))?;
        self.0
            .borrow_mut()
            .transport
            .push(json!({"op":"css","value":css}))
    }
}

// Refuse host behavior that has not been implemented, before staged UI changes.
fn preflight(node: DescriptorView<'_>, patch: Option<&Patch>) -> Result<(), Error> {
    glory_html::preflight(node, patch)?;
    if node.options.get::<VirtualPart>().is_some() {
        return Err(Error::Unsupported("Wry virtual collection handover"));
    }
    if matches!(
        node.props,
        Props::Select(_) | Props::File(_) | Props::RichEditor(_)
    ) {
        return Err(Error::Unsupported("Wry structured control"));
    }
    let semantics = match patch {
        None => node.semantics.as_deref(),
        Some(Patch::Semantics(Change::Set(value))) => value.as_deref(),
        _ => None,
    };
    if let Some(s) = semantics
        && (s.focus_scope.is_some()
            || s.clipboard.is_some()
            || s.input_edit.is_some()
            || s.translation.is_some()
            || s.collapse.is_some()
            || s.drag_capture.is_some()
            || s.swipe_capture.is_some()
            || s.context_menu.is_some()
            || s.tags_input.is_some()
            || s.splitter.is_some()
            || s.slider.is_some()
            || s.spinbutton.is_some()
            || s.scroll_keyboard.is_some()
            || s.combobox.is_some()
            || s.option.is_some()
            || s.menu_item.is_some()
            || s.menubar_item
            || s.layer.is_some()
            || s.sheet_grip
            || s.navigation.is_some()
            || s.grid_navigation.is_some()
            || s.tree_navigation.is_some()
            || s.navigation_panel.is_some()
            || s.typeahead
            || s.form_action.is_some()
            || s.form_control.is_some()
            || s.field_part == Some(FieldPart::Form))
    {
        return Err(Error::Unsupported("Wry interaction metadata"));
    }
    if matches!(node.props, Props::Graphic(_)) {
        return Err(Error::Unsupported("Wry animated graphic"));
    }
    Ok(())
}
fn supported_event(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::Activate
            | EventKind::ValueChanged
            | EventKind::Commit
            | EventKind::CheckedChanged
            | EventKind::Composition
            | EventKind::FocusChanged
            | EventKind::FocusWithin
            | EventKind::Resize
            | EventKind::Scroll
            | EventKind::Keyboard
            | EventKind::Navigate
            | EventKind::ColorSchemeChanged
            | EventKind::ImageStatus
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
    fn style_profile(&self) -> style::Profile {
        style::Profile::HTML
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            measure: true,
            scroll: true,
            focus: true,
            input: true,
            ..Capabilities::default()
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
            .and({
                if matches!(
                    query,
                    Query::FileRead(_) | Query::FilePreview(_) | Query::RichSelection(_)
                ) {
                    Err(Error::Unsupported("Wry structured query"))
                } else {
                    Ok(())
                }
            });
        let applied = state.mapper.receipt().applied;
        if let Err(error) = result {
            state.inputs.push(Input::Response {
                token,
                applied,
                result: Err(error),
            });
            return Ok(());
        }
        let kind = match query {
            Query::Bounds(_) => "bounds",
            Query::Focus(_) => "focus",
            Query::Scroll(_) => "scroll",
            Query::ScrollMetrics(_) => "scroll_metrics",
            Query::Selection(_) => "selection",
            _ => unreachable!(),
        };
        state.transport.push(json!({"op":"query","id":protocol::id(query.node()),"kind":kind,"token":token.0.to_string()}))?;
        state.pending.insert(token, Pending { query, applied });
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
        if !state.mapper.tree().active(action.node()) {
            return Err(Error::Unsupported("Wry inactive action target"));
        }
        let value = match action {
            Action::Focus(_) => json!({"kind":"focus"}),
            Action::ScrollTo { x, y, .. } => json!({"kind":"scroll","x":x,"y":y}),
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
            _ => return Err(Error::Unsupported("Wry platform action")),
        };
        state
            .transport
            .push(json!({"op":"action","id":protocol::id(action.node()),"action":value}))
    }
    fn drain(&mut self, inputs: &mut Vec<Input>) {
        let mut state = self.0.borrow_mut();
        inputs.append(&mut state.inputs);
        state.input_bytes = 0;
    }
}

impl WireEvent {
    fn into_event(self, node: NodeId) -> Result<Event, Error> {
        Ok(match self {
            Self::Activate => Event::Activate,
            Self::Value {
                value,
                composing,
                revision,
            } => Event::ValueChanged {
                value,
                composing,
                revision: protocol::integer(&revision)?,
            },
            Self::Commit { value, revision } => Event::Commit {
                value,
                revision: protocol::integer(&revision)?,
            },
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
