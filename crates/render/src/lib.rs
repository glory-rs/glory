//! A finite semantic UI contract. No HTML tags, engine types or reactive handles.
//! Rendering adapters translate this contract into their own objects and ranges.

#[cfg(feature = "conformance")]
pub mod conformance;
mod id;
pub mod memory;
pub mod profile;
mod props;
mod text;

pub use glory_style as style;
pub use id::{NodeId, NodeIds, RuntimeId};
pub use props::*;
pub use text::normalize_text;
mod keyboard;
pub use keyboard::{KeyboardCommand, KeyboardInput};
mod drag;
pub use drag::DragPhase;

/// Optional windowed-view metadata. Engines can show a flow preview before
/// geometry is available without changing logical ownership or identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualPart {
    Root,
    Surface,
    Row,
}

/// A patch distinguishes absence, replacement and reset to the default value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Change<T> {
    #[default]
    Keep,
    Set(T),
    Clear,
}

impl<T: Default> Change<T> {
    pub fn apply(self, target: &mut T) {
        self.apply_or(target, T::default());
    }
}

impl<T> Change<T> {
    pub fn apply_or(self, target: &mut T, default: T) {
        match self {
            Self::Keep => {}
            Self::Set(value) => *target = value,
            Self::Clear => *target = default,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mutation {
    Create {
        id: NodeId,
        node: Box<Descriptor>,
    },
    Patch {
        id: NodeId,
        patch: Patch,
    },
    /// `None` parent places a root. Existing nodes move, preserving identity.
    Place {
        id: NodeId,
        parent: Option<NodeId>,
        before: Option<NodeId>,
    },
    /// Disconnect a range, preserving nodes and identities until Resume/Dispose.
    Suspend {
        id: NodeId,
    },
    Resume {
        id: NodeId,
        parent: Option<NodeId>,
        before: Option<NodeId>,
    },
    /// Permanently release this subtree, including suspended descendants.
    Dispose {
        id: NodeId,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub measure: bool,
    pub scroll: bool,
    pub focus: bool,
    pub input: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueryToken(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Query {
    Bounds(NodeId),
    Scroll(NodeId),
    Selection(NodeId),
    /// Whether this node's actual control owns document focus at the request
    /// barrier. This is neither FocusWithin nor OS/window activation.
    Focus(NodeId),
}

impl Query {
    pub fn node(self) -> NodeId {
        match self {
            Self::Bounds(id) | Self::Scroll(id) | Self::Selection(id) | Self::Focus(id) => id,
        }
    }

    /// Semantic target checks precede host-specific availability checks. A
    /// boxless Group has no physical bounds/scroll/selection of its own.
    pub fn validate_target(self, node: &Descriptor) -> Result<(), Error> {
        if matches!(node.props, Props::Group) {
            return Err(Error::Unsupported("group has no physical query target"));
        }
        if matches!(self, Self::Selection(_)) && !matches!(node.props, Props::Input(_)) {
            return Err(Error::Unsupported("selection requires a text input"));
        }
        Ok(())
    }

    /// Check adapter output before it reaches application callbacks. Coordinates
    /// use logical pixels; selection offsets use UTF-16 code units on all hosts.
    pub fn validate_response(self, value: &QueryValue) -> Result<(), Error> {
        match (self, value) {
            (Self::Bounds(_), QueryValue::Bounds(rect)) => {
                if ![rect.x, rect.y, rect.width, rect.height]
                    .into_iter()
                    .all(f64::is_finite)
                    || rect.width < 0.0
                    || rect.height < 0.0
                {
                    return Err(Error::InvalidResponse("invalid bounds"));
                }
            }
            (Self::Scroll(_), QueryValue::Scroll { x, y }) => {
                // Native RTL and elastic scrolling can expose negative offsets.
                if !x.is_finite() || !y.is_finite() {
                    return Err(Error::InvalidResponse("invalid scroll offsets"));
                }
            }
            (Self::Selection(_), QueryValue::Selection { start, end }) => {
                if start > end {
                    return Err(Error::InvalidResponse("reversed selection"));
                }
            }
            (Self::Focus(_), QueryValue::Focus(_)) => {}
            _ => return Err(Error::InvalidResponse("query result kind mismatch")),
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Request focus without scrolling. Ok means the host processed the intent;
    /// it may decline because of current layout/platform state. Query::Focus
    /// reads the actual result; FocusChanged reports actual transitions only.
    Focus(NodeId),
    ScrollTo {
        node: NodeId,
        x: f64,
        y: f64,
    },
}
impl Action {
    pub fn node(self) -> NodeId {
        match self {
            Self::Focus(node) | Self::ScrollTo { node, .. } => node,
        }
    }
    /// Validate after checking identity, before capability checks or queuing.
    pub fn validate_target(self, node: &Descriptor) -> Result<(), Error> {
        if let Self::ScrollTo { x, y, .. } = self
            && (!x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0)
        {
            return Err(Error::InvalidProps("invalid scroll position"));
        }
        if matches!(node.props, Props::Group) {
            return Err(Error::Unsupported("group has no physical action target"));
        }
        if matches!(self, Self::Focus(_)) && !node.props.enabled() {
            return Err(Error::Unsupported("focus on disabled control"));
        }
        if matches!(self, Self::Focus(_))
            && matches!(&node.props, Props::Image(image) if image.alternative.is_none())
        {
            return Err(Error::Unsupported("focus on decorative image"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Logical collection navigation, independent of a host's key codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Navigation {
    Previous,
    Next,
    First,
    Last,
    PagePrevious,
    PageNext,
    /// Reveal the originating mounted row without moving keyboard focus.
    /// A virtual-list root or an origin outside its rows has no destination.
    Reveal,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryValue {
    Focus(bool),
    Bounds(Rect),
    Scroll {
        x: f64,
        y: f64,
    },
    /// UTF-16 code-unit offsets, matching Web text controls. Native adapters must
    /// convert their engine's byte/scalar/grapheme offsets at this boundary.
    Selection {
        start: usize,
        end: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Activate,
    /// Host preference, independent of an application's explicit appearance.
    ColorSchemeChanged {
        dark: bool,
    },
    /// Physical origin remains explicit while App routes to the nearest observer.
    Keyboard {
        from: NodeId,
        command: KeyboardCommand,
    },
    /// Start/Move/Cancel target the source. Drop targets a live accepting node.
    Drag {
        from: NodeId,
        phase: DragPhase,
        x: f64,
        y: f64,
    },
    ValueChanged {
        value: String,
        composing: bool,
        revision: u64,
    },
    Commit {
        value: String,
        revision: u64,
    },
    CheckedChanged(bool),
    /// Direct focus origin. Hosts report this even if only an ancestor has a
    /// FocusWithin interest; App derives the logical ancestor observations.
    FocusChanged(bool),
    /// Focus transition originating at `node`, delivered to observing logical
    /// ancestors (including boxless Groups). FocusChanged itself stays direct.
    FocusWithin {
        node: NodeId,
        focused: bool,
    },
    /// Origin stays explicit when routed to the nearest logical observer.
    Navigate {
        from: NodeId,
        command: Navigation,
    },
    Composition {
        active: bool,
    },
    Scroll {
        x: f64,
        y: f64,
        viewport: Rect,
        /// Last accepted assignment, separate from current x/y (which may
        /// already include native movement). None before any host assignment.
        ack: Option<ScrollAck>,
    },
    /// Border-box size and client viewport are distinct (borders/scrollbars).
    /// Size uses local x/y=0; adapters preserve fractional layout dimensions.
    Resize {
        size: Rect,
        viewport: Rect,
    },
    /// Restore an existing visible row and its pixel inset at host handover.
    Anchor {
        row: NodeId,
        within: f64,
        viewport: Rect,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollRequest {
    pub x: f64,
    pub y: f64,
}
/// Last accepted programmatic assignment, retained on later observations so
/// native movement before the first acknowledgement is still distinguishable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAck {
    pub requested: ScrollRequest,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventKind {
    Activate,
    ColorSchemeChanged,
    Keyboard,
    Drag,
    ValueChanged,
    Commit,
    CheckedChanged,
    FocusChanged,
    FocusWithin,
    Navigate,
    Composition,
    Scroll,
    Resize,
    Anchor,
}

impl EventKind {
    /// Logical bubbling/ancestor observations can target Groups; direct physical
    /// observations cannot. Recording a valid interest does not promise the host
    /// can produce it (SSR records handlers for later browser adoption).
    pub fn validate_target(self, node: &Descriptor) -> Result<(), Error> {
        if matches!(self, Self::Drag | Self::ColorSchemeChanged)
            && !matches!(node.props, Props::View)
        {
            return Err(Error::Unsupported(
                "host observation requires a semantic View",
            ));
        }
        if matches!(self, Self::ValueChanged | Self::Commit | Self::Composition)
            && !matches!(node.props, Props::Input(_))
            || matches!(self, Self::CheckedChanged) && !matches!(node.props, Props::Check(_))
        {
            return Err(Error::InvalidProps("event does not belong to this control"));
        }
        if matches!(node.props, Props::Group)
            && matches!(
                self,
                Self::Resize | Self::Scroll | Self::Anchor | Self::FocusChanged | Self::Drag
            )
        {
            return Err(Error::Unsupported("group has no physical event target"));
        }
        Ok(())
    }
}

impl Event {
    /// Cleanup observations still reach live scopes after suspension. Retired
    /// identities never become valid again because an event is terminal.
    pub fn is_terminal_loss(&self) -> bool {
        matches!(
            self,
            Self::FocusChanged(false)
                | Self::Composition { active: false }
                | Self::FocusWithin { focused: false, .. }
                | Self::Drag {
                    phase: DragPhase::Cancel,
                    ..
                }
        )
    }

    pub(crate) fn origin(&self) -> Option<NodeId> {
        match self {
            Self::FocusWithin { node, .. } => Some(*node),
            Self::Navigate { from, .. } => Some(*from),
            Self::Keyboard { from, .. } => Some(*from),
            Self::Drag { from, .. } => Some(*from),
            Self::Anchor { row, .. } => Some(*row),
            _ => None,
        }
    }

    /// Check typed host input before business handlers. Embedded identities'
    /// liveness/ancestry require a tree and are checked by Memory::validate_event.
    /// Scroll offsets may be negative; local geometry must have x/y=0.
    pub fn validate_payload(&self, target: NodeId, node: &Descriptor) -> Result<(), Error> {
        if self
            .origin()
            .is_some_and(|id| id.runtime() != target.runtime())
        {
            return Err(Error::ForeignRuntime);
        }
        self.kind().validate_target(node)?;
        let local = |rect: &Rect| {
            rect.x == 0.0
                && rect.y == 0.0
                && rect.width.is_finite()
                && rect.height.is_finite()
                && rect.width >= 0.0
                && rect.height >= 0.0
        };
        match self {
            Self::Drag { from, phase, x, y } => {
                if !x.is_finite() || !y.is_finite() {
                    return Err(Error::InvalidEvent("drag coordinates must be finite"));
                }
                if *phase != DragPhase::Drop && *from != target {
                    return Err(Error::InvalidEvent("drag phase requires source target"));
                }
            }
            Self::Keyboard { from, command } => {
                if *from != target || matches!(node.props, Props::Group) {
                    return Err(Error::InvalidEvent("keyboard requires its physical origin"));
                }
                if *command == KeyboardCommand::Submit && !matches!(node.props, Props::Input(_)) {
                    return Err(Error::InvalidEvent("submit requires a text control"));
                }
            }
            Self::Navigate { from, .. } if *from != target => {
                return Err(Error::InvalidEvent(
                    "navigation origin must match its event target",
                ));
            }
            Self::Scroll {
                x,
                y,
                viewport,
                ack,
            } => {
                if !x.is_finite() || !y.is_finite() || !local(viewport) {
                    return Err(Error::InvalidEvent("invalid scroll geometry"));
                }
                if ack.is_some_and(|ack| {
                    !ack.requested.x.is_finite()
                        || !ack.requested.y.is_finite()
                        || ack.requested.x < 0.0
                        || ack.requested.y < 0.0
                        || !ack.x.is_finite()
                        || !ack.y.is_finite()
                }) {
                    return Err(Error::InvalidEvent("invalid scroll acknowledgement"));
                }
            }
            Self::Resize { size, viewport } if !local(size) || !local(viewport) => {
                return Err(Error::InvalidEvent("invalid resize geometry"));
            }
            Self::Anchor {
                within, viewport, ..
            } if !within.is_finite() || *within < 0.0 || !local(viewport) => {
                return Err(Error::InvalidEvent("invalid anchor geometry"));
            }
            Self::ValueChanged {
                value, revision, ..
            }
            | Self::Commit { value, revision } => {
                if matches!(&node.props, Props::Input(input) if !input.multiline && value.contains(['\r', '\n']))
                {
                    return Err(Error::InvalidEvent(
                        "single-line input event cannot contain line breaks",
                    ));
                }
                if value.contains(['\0', '\r']) {
                    return Err(Error::InvalidEvent("input event text is not normalized"));
                }
                if let Props::Input(input) = &node.props
                    && *revision > input.revision
                {
                    return Err(Error::InputRevision {
                        current: input.revision,
                        incoming: *revision,
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn kind(&self) -> EventKind {
        match self {
            Self::Activate => EventKind::Activate,
            Self::ColorSchemeChanged { .. } => EventKind::ColorSchemeChanged,
            Self::Keyboard { .. } => EventKind::Keyboard,
            Self::Drag { .. } => EventKind::Drag,
            Self::ValueChanged { .. } => EventKind::ValueChanged,
            Self::Commit { .. } => EventKind::Commit,
            Self::CheckedChanged(_) => EventKind::CheckedChanged,
            Self::FocusChanged(_) => EventKind::FocusChanged,
            Self::FocusWithin { .. } => EventKind::FocusWithin,
            Self::Navigate { .. } => EventKind::Navigate,
            Self::Composition { .. } => EventKind::Composition,
            Self::Scroll { .. } => EventKind::Scroll,
            Self::Resize { .. } => EventKind::Resize,
            Self::Anchor { .. } => EventKind::Anchor,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Event {
        node: NodeId,
        event: Event,
    },
    Response {
        token: QueryToken,
        /// Applied mutation watermark observed by this request.
        applied: u64,
        result: Result<QueryValue, Error>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Backend(String),
    ForeignRuntime,
    NodeGone(NodeId),
    StaleNode(NodeId),
    DuplicateNode(NodeId),
    InvalidPlacement(&'static str),
    InvalidProps(&'static str),
    Unsupported(&'static str),
    UnsupportedStyle {
        profile: &'static str,
        version: u32,
        requirement: style::Unsupported,
    },
    InvalidReceipt {
        previous: u64,
        applied: u64,
    },
    QueryWatermark {
        expected: u64,
        applied: u64,
    },
    InvalidResponse(&'static str),
    InvalidEvent(&'static str),
    InputRevision {
        current: u64,
        incoming: u64,
    },
    InputRevisionExhausted,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Error {}

/// Applying nodes does not imply pixels have been displayed.
/// Successful nonempty apply calls strictly advance this renderer-local
/// watermark; empty calls preserve it. It is not a displayed-frame sequence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Receipt {
    pub applied: u64,
}

/// Local calls are typed and require no serialization. Each request observes
/// every preceding successful apply; adapters must not coalesce across requests.
/// A response carries the watermark at request acceptance, even if delivered
/// after later commits. Capture results at that barrier, not from a newer tree.
/// Errors may leave an applied prefix; adapters do not promise rollback.
pub trait Render {
    /// Reject descriptor-local limitations before the App changes its logical
    /// tree or queues output. None checks creation; Some checks a patch against
    /// the supplied current descriptor, including earlier uncommitted changes.
    /// Read only these arguments and immutable adapter configuration: no host
    /// calls, layout, application callbacks, or committed-tree lookups. Adapters
    /// must enforce the same restrictions in apply for callers without an App.
    /// This is not a transaction or a guarantee that later host work succeeds.
    fn preflight(&self, _node: &Descriptor, _patch: Option<&Patch>) -> Result<(), Error> {
        Ok(())
    }
    /// Validate adapter-specific ancestor/content constraints against the App's
    /// staged semantic tree, including uncommitted Create/Patch/Place changes.
    /// New children are checked before identity allocation; existing children
    /// before Place/Resume changes the model or output queue. Read only these
    /// arguments/configuration: no host tree, layout, I/O or application code.
    /// The App checks shared identity/placement semantics first. Default adds
    /// no restrictions. Direct apply must enforce the same content rules.
    fn preflight_place(
        &self,
        _tree: &memory::Memory,
        _child: memory::Subtree<'_>,
        _parent: Option<NodeId>,
    ) -> Result<(), Error> {
        Ok(())
    }
    /// Optional local diagnostics. Return an unused or retired handle so its
    /// captured clock is dropped outside the caller's renderer borrow.
    fn set_profile(&mut self, profile: Option<profile::Profile>) -> Option<profile::Profile> {
        profile
    }
    /// Another local observation can make progress without awaiting I/O, an
    /// animation timestamp or a future browser frame. Read-only and never runs
    /// application code. Hosts with asynchronous observation keep the default.
    fn needs_observe(&self) -> bool {
        false
    }
    /// Declared once per App. Default rejects nonempty inline styles; adapters
    /// opt into exactly the declarations/values they preserve or implement.
    fn style_profile(&self) -> style::Profile {
        style::Profile::NONE
    }
    /// Event interest is frame metadata. Hosts may use it to avoid observing
    /// thousands of nodes whose resize/scroll events nobody consumes.
    fn listen(&mut self, _node: NodeId, _kind: EventKind, _enabled: bool) -> Result<(), Error> {
        Ok(())
    }
    fn capabilities(&self) -> Capabilities;
    fn apply(&mut self, mutations: &[Mutation]) -> Result<Receipt, Error>;
    fn request(&mut self, query: Query, token: QueryToken) -> Result<(), Error>;
    /// Actions run after a frame's mutations and before subsequent queries.
    /// Unsupported focus refusal leaves the semantic tree valid: App treats it
    /// as an unfulfilled intent and continues later intents. Backend errors remain
    /// fatal. Never synthesize FocusChanged(true) merely for accepting Focus.
    fn action(&mut self, _action: Action) -> Result<(), Error> {
        Err(Error::Unsupported("platform action"))
    }
    /// Return queued inputs, never synchronously reenter business closures.
    fn drain(&mut self, inputs: &mut Vec<Input>);
}
