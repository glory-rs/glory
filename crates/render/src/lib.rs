//! A finite semantic UI contract. No HTML tags, engine types or reactive handles.
//! Rendering adapters translate this contract into their own objects and ranges.

mod anchor;
mod viewport_layer;
pub use viewport_layer::ViewportPlacement;
#[cfg(feature = "conformance")]
pub mod conformance;
mod id;
pub use anchor::{AnchorAlign, AnchorPlacement, AnchorPosition, AnchorSide};
mod node_map;
pub use node_map::NodeMap;
mod collapse;
mod document;
pub use collapse::{CollapseConfig, MAX_COLLAPSE_MILLIS};
mod graphic;
mod graphic_transition;
pub use graphic::{
    GraphicPoint, GraphicProps, GraphicStroke, MAX_GRAPHIC_COMMANDS, MAX_GRAPHIC_COORDINATE,
    PathCommand, StrokeCap,
};
pub use graphic_transition::GraphicTransition;
mod choice;
pub use choice::{ChoiceCommand, ComboboxKind, OptionSemantics};
mod context_menu;
mod tags;
pub use context_menu::{ContextEditPolicy, ContextPoint, ContextSource};
pub use tags::{
    MAX_TAG_INPUT_BYTES, MAX_TAG_PASTE_BYTES, TagHostRejection, TagKey, TagSeparators, TagsCommand,
};
mod select;
pub use select::{
    MAX_SELECT_BYTES, MAX_SELECT_GROUPS, MAX_SELECT_OPTIONS, SelectEntry, SelectGroup,
    SelectOption, SelectProps, SelectSelection,
};
mod range;
pub use range::RangeValue;
mod progress;
pub use progress::ProgressValue;
mod motion;
pub use motion::{GraphicAnimation, MotionPreference};
mod translation;
pub use translation::{
    MAX_TRANSLATION_COORDINATE, TranslationKind, TranslationMotion, TranslationOffset,
    TranslationPhase,
};
pub mod memory;
mod plan;
pub mod profile;
mod props;
mod semantics;
pub use document::DocumentPart;
mod text;

pub use glory_style as style;
pub use id::{NodeId, NodeIds, RuntimeId};
pub use plan::{MAX_PLAN_NODES, Plan, PlanNode, WeakPlan};
pub use props::*;
pub use semantics::{
    Current, Direction, DismissReason, FieldPart, FormAction, FormControl, Layer, LayerFocus,
    LayerPlacement, LinkOpen, MenuItemKind, MenuItemSemantics, Orientation, PopupKind, SemanticId,
    Semantics,
};
pub use text::normalize_text;
mod keyboard;
pub use keyboard::{KeyboardCommand, KeyboardInput};
mod navigation_panel;
pub use navigation_panel::{NavigationPanel, NavigationPanelCommand};
mod grid;
pub use grid::GridCommand;
mod scroll;
pub use scroll::{ScrollAxes, ScrollCommand, ScrollMetrics, ScrollMovement};
mod sheet;
pub use sheet::{MAX_SHEET_SNAPS, SheetCommand, SheetConfig};
mod slider;
pub use slider::SliderValue;
mod spinbutton;
pub use spinbutton::SpinValue;
mod input_assist;
pub use input_assist::{InputAssist, InputAutofill, InputMode};
mod text_selection;
pub use text_selection::{SelectionDirection, TextSelection};
mod input_edit;
pub use input_edit::{
    InputEdit, InputEditCapture, InputEditIntent, InputEditRejection, MAX_INPUT_EDIT_BYTES,
};
mod splitter;
pub use splitter::RangeCommand;
mod typeahead;
pub use typeahead::{MAX_TYPEAHEAD_MILLIS, validate_typeahead};
mod drag;
pub use drag::{DragCapture, DragPhase};
mod menu;
pub use menu::{MenuCommand, PointerPhase};

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
    /// One validated, detached construction unit. Internal placement belongs
    /// to this mutation; attach/move its root with a subsequent Place.
    CreatePlan {
        plan: Plan,
        ids: Vec<NodeId>,
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
    /// Emits a terminal observation for each finite collapse configuration.
    pub collapse_completion: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueryToken(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Query {
    Bounds(NodeId),
    Scroll(NodeId),
    ScrollMetrics(NodeId),
    Selection(NodeId),
    /// Whether this node's actual control owns document focus at the request
    /// barrier. This is neither FocusWithin nor OS/window activation.
    Focus(NodeId),
}

impl Query {
    pub fn node(self) -> NodeId {
        match self {
            Self::Bounds(id)
            | Self::Scroll(id)
            | Self::ScrollMetrics(id)
            | Self::Selection(id)
            | Self::Focus(id) => id,
        }
    }

    /// Semantic target checks precede host-specific availability checks. A
    /// boxless Group has no physical bounds/scroll/selection of its own.
    pub fn validate_target<'a>(self, node: impl Into<DescriptorView<'a>>) -> Result<(), Error> {
        let node = node.into();
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
            (Self::ScrollMetrics(_), QueryValue::ScrollMetrics(metrics)) => metrics.validate()?,
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
    /// Bring a current nonmodal Dialog layer forward without moving its DOM,
    /// editor identity or actual focus. The host's current modal still applies.
    RaiseLayer(NodeId),
    /// Select an exact range in the current ASCII Input value. A stale revision
    /// is rejected; Unicode/surrogate/grapheme addressing is explicitly deferred.
    SelectText {
        node: NodeId,
        revision: u64,
        selection: TextSelection,
    },
    /// Reveal a mounted option inside its nearest owning listbox, preserving
    /// actual combobox focus and the outer document scroll position.
    RevealOption {
        node: NodeId,
    },
    /// Leave this named nonmodal Menu's tree in native Tab order, relative to
    /// its return-focus trigger. Does not imply the controlled menu accepted a close.
    ExitMenu {
        node: NodeId,
        backwards: bool,
    },
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
            Self::Focus(node)
            | Self::RaiseLayer(node)
            | Self::ScrollTo { node, .. }
            | Self::ExitMenu { node, .. }
            | Self::RevealOption { node } => node,
            Self::SelectText { node, .. } => node,
        }
    }
    /// Validate after checking identity, before capability checks or queuing.
    pub fn validate_target<'a>(self, node: impl Into<DescriptorView<'a>>) -> Result<(), Error> {
        let node = node.into();
        if matches!(self, Self::RaiseLayer(_))
            && (node.role != Some(Role::Dialog)
                || node.hidden()
                || !node
                    .semantics
                    .as_ref()
                    .and_then(|s| s.layer.as_ref())
                    .is_some_and(|layer| !layer.modal))
        {
            return Err(Error::Unsupported(
                "raise requires an open nonmodal dialog layer",
            ));
        }
        if let Self::SelectText {
            revision,
            selection,
            ..
        } = self
        {
            let Props::Input(input) = node.props else {
                return Err(Error::Unsupported("text selection requires an Input"));
            };
            if revision != input.revision {
                return Err(Error::InputRevision {
                    current: input.revision,
                    incoming: revision,
                });
            }
            if !input.value.is_ascii() {
                return Err(Error::Unsupported(
                    "exact selection currently requires ASCII input text",
                ));
            }
            selection.validate(input.value.len())?;
        }
        if matches!(self, Self::Focus(_))
            && node.semantics.as_ref().is_some_and(|s| s.menubar_item)
            && node.focus != FocusMode::TabStop
        {
            return Err(Error::Unsupported(
                "menubar focus intent no longer names the current root",
            ));
        }
        if matches!(self, Self::RevealOption { .. })
            && !node.semantics.as_ref().is_some_and(|s| s.option.is_some())
        {
            return Err(Error::Unsupported(
                "option reveal requires an intrinsic listbox option",
            ));
        }
        if matches!(self, Self::Focus(_))
            && (node.role == Some(Role::Listbox)
                || node.semantics.as_ref().is_some_and(|s| s.option.is_some()))
        {
            return Err(Error::Unsupported(
                "listbox options preserve combobox focus",
            ));
        }
        if matches!(self, Self::ExitMenu { .. })
            && (node.role != Some(Role::Menu)
                || !node
                    .semantics
                    .as_ref()
                    .and_then(|s| s.layer.as_ref())
                    .is_some_and(|layer| {
                        !layer.modal
                            && (layer.return_focus.is_some()
                                || matches!(layer.placement, LayerPlacement::ContextPoint(_))
                                    && layer.anchor.is_some())
                    }))
        {
            return Err(Error::Unsupported(
                "menu exit requires a named nonmodal menu with a return target",
            ));
        }
        if let Self::ScrollTo { x, y, .. } = self
            && (!x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0)
        {
            return Err(Error::InvalidProps("invalid scroll position"));
        }
        if matches!(self, Self::Focus(_))
            && node.role == Some(Role::Separator)
            && !node
                .semantics
                .as_ref()
                .is_some_and(|s| s.splitter.is_some())
        {
            return Err(Error::Unsupported("focus on passive separator"));
        }
        if matches!(self, Self::Focus(_)) && node.role == Some(Role::Tooltip) {
            return Err(Error::Unsupported("focus on passive tooltip"));
        }
        if matches!(self, Self::Focus(_)) && node.role == Some(Role::ProgressBar) {
            return Err(Error::Unsupported("focus on passive progress"));
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
        if matches!(self, Self::Focus(_))
            && matches!(node.props, Props::Graphic(graphic) if graphic.alternative.is_none())
        {
            return Err(Error::Unsupported("focus on decorative graphic"));
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
    ScrollMetrics(ScrollMetrics),
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
    GridNavigate {
        from: NodeId,
        command: GridCommand,
    },
    InputEdit(InputEdit),
    /// Terminal presentation observation; valid while the closing subtree is
    /// logically hidden/inert, without admitting business input there.
    CollapseComplete {
        revision: u64,
    },
    ContextMenu {
        from: NodeId,
        point: ContextPoint,
        source: ContextSource,
    },
    Sheet {
        from: NodeId,
        revision: u64,
        command: SheetCommand,
    },
    NavigationPanel {
        from: NodeId,
        command: NavigationPanelCommand,
    },
    Spin {
        from: NodeId,
        command: RangeCommand,
    },
    Range {
        from: NodeId,
        command: RangeCommand,
    },
    ScrollKeyboard {
        from: NodeId,
        command: ScrollCommand,
    },
    /// Terminal observation for one program-selected image request.
    ImageStatus {
        revision: u64,
        status: ImageStatus,
    },
    Menu {
        from: NodeId,
        command: MenuCommand,
    },
    Pointer {
        from: NodeId,
        related: Option<NodeId>,
        phase: PointerPhase,
    },
    /// Logical boundary observation derived from a physical pointer origin.
    PointerWithin {
        from: NodeId,
        related: Option<NodeId>,
        phase: PointerPhase,
    },
    /// Native form default behavior is cancelled before this queued request.
    /// None represents a programmatic request or implicit keyboard submission.
    Submit {
        submitter: Option<NodeId>,
    },
    Reset,
    Activate,
    /// A request only: the application may keep controlled state open.
    Dismiss(DismissReason),
    /// Host preference, independent of an application's explicit appearance.
    ColorSchemeChanged {
        dark: bool,
    },
    /// Physical origin remains explicit while App routes to the nearest observer.
    Keyboard {
        from: NodeId,
        command: KeyboardCommand,
    },
    /// Only the explicit palette chord; other application commands stay native.
    Palette {
        from: NodeId,
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
    Tags {
        from: NodeId,
        revision: u64,
        command: TagsCommand,
    },
    Choice {
        from: NodeId,
        command: ChoiceCommand,
    },
    SelectChanged {
        keys: Vec<SemanticId>,
        revision: u64,
    },
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
    Typeahead {
        from: NodeId,
        text: String,
        at_millis: u64,
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
    GridNavigate,
    InputEdit,
    CollapseComplete,
    ContextMenu,
    Tags,
    Sheet,
    NavigationPanel,
    Spin,
    Range,
    ScrollKeyboard,
    ImageStatus,
    Menu,
    Pointer,
    PointerWithin,
    Submit,
    Reset,
    Activate,
    Dismiss,
    ColorSchemeChanged,
    Keyboard,
    Palette,
    Drag,
    ValueChanged,
    Commit,
    CheckedChanged,
    SelectChanged,
    Choice,
    FocusChanged,
    FocusWithin,
    Navigate,
    Typeahead,
    Composition,
    Scroll,
    Resize,
    Anchor,
}

impl EventKind {
    /// Logical bubbling/ancestor observations can target Groups; direct physical
    /// observations cannot. Recording a valid interest does not promise the host
    /// can produce it (SSR records handlers for later browser adoption).
    pub fn validate_target<'a>(self, node: impl Into<DescriptorView<'a>>) -> Result<(), Error> {
        let node = node.into();
        if self == Self::GridNavigate
            && (node.role != Some(Role::Grid)
                || !matches!(node.props, Props::View)
                || !node
                    .semantics
                    .as_ref()
                    .is_some_and(|s| s.grid_navigation.is_some()))
        {
            return Err(Error::InvalidProps(
                "grid navigation requires an opted-in Grid View",
            ));
        }
        if self == Self::InputEdit
            && !matches!(node.props,Props::Input(p) if !p.multiline&&node.semantics.as_ref().is_some_and(|s|s.input_edit.is_some()))
        {
            return Err(Error::Unsupported(
                "InputEdit requires an opted-in single-line Input",
            ));
        }
        if self == Self::CollapseComplete
            && (!matches!(node.props, Props::View)
                || !node
                    .semantics
                    .as_ref()
                    .is_some_and(|s| s.collapse.is_some()))
        {
            return Err(Error::InvalidProps(
                "collapse completion requires a collapse View",
            ));
        }
        if self == Self::ContextMenu
            && (!matches!(node.props, Props::View)
                || !node
                    .semantics
                    .as_ref()
                    .is_some_and(|s| s.context_menu.is_some()))
        {
            return Err(Error::InvalidProps(
                "context requests require an opted-in region view",
            ));
        }
        if self == Self::Tags
            && !matches!(node.props,Props::Input(input) if !input.multiline && node.semantics.as_ref().is_some_and(|s|s.tags_input.is_some()))
        {
            return Err(Error::InvalidProps(
                "tags commands require an opted-in single-line input",
            ));
        }
        if self == Self::Sheet && !matches!(node.props, Props::View) {
            return Err(Error::InvalidProps("sheet commands require a view"));
        }
        if self == Self::NavigationPanel
            && !matches!(node.props, Props::Button(_) | Props::Link(_) | Props::View)
        {
            return Err(Error::InvalidProps(
                "navigation panel commands require a physical navigation control or content view",
            ));
        }
        if self == Self::Spin
            && !matches!(node.props, Props::Input(p) if !p.multiline && node.semantics.as_ref().is_some_and(|s| s.spinbutton.is_some()))
        {
            return Err(Error::InvalidProps(
                "spin commands require an opted-in single-line input",
            ));
        }
        if self == Self::Range
            && (!matches!(node.props, Props::View)
                || !matches!(node.role, Some(Role::Separator | Role::Slider)))
        {
            return Err(Error::InvalidProps(
                "range commands require a slider or splitter separator",
            ));
        }
        if self == Self::Choice
            && !node
                .semantics
                .as_ref()
                .is_some_and(|s| s.combobox.is_some())
        {
            return Err(Error::InvalidProps("choice commands require a combobox"));
        }
        if self == Self::SelectChanged && !matches!(node.props, Props::Select(_)) {
            return Err(Error::InvalidProps("selection event requires a select"));
        }
        if self == Self::ScrollKeyboard && !matches!(node.props, Props::View) {
            return Err(Error::InvalidProps(
                "scroll keyboard requires a viewport view",
            ));
        }
        if self == Self::ImageStatus && !matches!(node.props, Props::Image(_)) {
            return Err(Error::InvalidProps("image status requires an image"));
        }
        if matches!(self, Self::Submit | Self::Reset) && node.field_part() != Some(FieldPart::Form)
        {
            return Err(Error::InvalidProps("form request requires a form"));
        }
        if matches!(
            node.role,
            Some(Role::Separator | Role::Tooltip | Role::ProgressBar)
        ) && matches!(
            self,
            Self::Activate
                | Self::Keyboard
                | Self::Palette
                | Self::ScrollKeyboard
                | Self::Drag
                | Self::Navigate
                | Self::Typeahead
                | Self::Menu
                | Self::NavigationPanel
                | Self::Range
        ) && !(node.role == Some(Role::Separator)
            && node
                .semantics
                .as_ref()
                .is_some_and(|s| s.splitter.is_some())
            && matches!(self, Self::Range | Self::Drag))
        {
            return Err(Error::Unsupported("passive separator has no interaction"));
        }
        // Interests may be registered before layer metadata bindings run.
        if self == Self::Dismiss && !matches!(node.props, Props::View) {
            return Err(Error::InvalidProps("dismiss requires a view"));
        }
        if matches!(self, Self::Drag | Self::ColorSchemeChanged)
            && !matches!(node.props, Props::View)
        {
            return Err(Error::Unsupported(
                "host observation requires a semantic View",
            ));
        }
        if self == Self::Palette
            && matches!(
                node.props,
                Props::Group | Props::Input(_) | Props::Select(_)
            )
        {
            return Err(Error::Unsupported(
                "palette shortcut requires a non-editor physical scope or origin",
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
                Self::Resize
                    | Self::Scroll
                    | Self::Anchor
                    | Self::FocusChanged
                    | Self::Drag
                    | Self::Pointer
            )
        {
            return Err(Error::Unsupported("group has no physical event target"));
        }
        Ok(())
    }
}

impl Event {
    /// User input is suppressed by inherited inert regions. Resource/layout
    /// observations and terminal cleanup remain available to live Owners.
    pub fn is_user_interaction(&self) -> bool {
        !self.is_terminal_loss()
            && !matches!(
                self,
                Self::ImageStatus { .. }
                    | Self::ColorSchemeChanged { .. }
                    | Self::Scroll { .. }
                    | Self::Resize { .. }
                    | Self::Anchor { .. }
            )
    }
    /// Cleanup observations still reach live scopes after suspension. Retired
    /// identities never become valid again because an event is terminal.
    pub fn is_terminal_loss(&self) -> bool {
        matches!(
            self,
            Self::CollapseComplete { .. }
                | Self::FocusChanged(false)
                | Self::Composition { active: false }
                | Self::FocusWithin { focused: false, .. }
                | Self::Drag {
                    phase: DragPhase::Cancel,
                    ..
                }
                | Self::Pointer {
                    phase: PointerPhase::Leave,
                    ..
                }
                | Self::PointerWithin {
                    phase: PointerPhase::Leave,
                    ..
                }
        )
    }

    pub(crate) fn origin(&self) -> Option<NodeId> {
        match self {
            Self::GridNavigate { from, .. } => Some(*from),
            Self::ContextMenu { from, .. } => Some(*from),
            Self::Tags { from, .. } => Some(*from),
            Self::Submit { submitter } => *submitter,
            Self::FocusWithin { node, .. } => Some(*node),
            Self::Navigate { from, .. } => Some(*from),
            Self::Menu { from, .. }
            | Self::Pointer { from, .. }
            | Self::PointerWithin { from, .. } => Some(*from),
            Self::Typeahead { from, .. } => Some(*from),
            Self::Choice { from, .. } | Self::Keyboard { from, .. } | Self::Palette { from } => {
                Some(*from)
            }
            Self::ScrollKeyboard { from, .. } => Some(*from),
            Self::Spin { from, .. } | Self::Range { from, .. } => Some(*from),
            Self::Sheet { from, .. } => Some(*from),
            Self::NavigationPanel { from, .. } => Some(*from),
            Self::Drag { from, .. } => Some(*from),
            Self::Anchor { row, .. } => Some(*row),
            _ => None,
        }
    }

    /// Check typed host input before business handlers. Embedded identities'
    /// liveness/ancestry require a tree and are checked by Memory::validate_event.
    /// Scroll offsets may be negative; local geometry must have x/y=0.
    pub fn validate_payload<'a>(
        &self,
        target: NodeId,
        node: impl Into<DescriptorView<'a>>,
    ) -> Result<(), Error> {
        let node = node.into();
        if let Self::GridNavigate { from, .. } = self {
            EventKind::GridNavigate.validate_target(node)?;
            if from.runtime() != target.runtime() {
                return Err(Error::ForeignRuntime);
            }
            if *from == target {
                return Err(Error::InvalidEvent(
                    "grid navigation requires a real descendant Button",
                ));
            }
        }
        if let Self::InputEdit(edit) = self {
            EventKind::InputEdit.validate_target(node)?;
            let capture = node
                .semantics
                .as_ref()
                .and_then(|s| s.input_edit)
                .ok_or(Error::InvalidEvent("input edit capture is absent"))?;
            edit.validate(capture)?;
            if let Props::Input(input) = node.props
                && edit.revision() > input.revision
            {
                return Err(Error::InputRevision {
                    current: input.revision,
                    incoming: edit.revision(),
                });
            }
        }
        if let Self::CollapseComplete { revision } = self {
            let config = node
                .semantics
                .as_ref()
                .and_then(|s| s.collapse.as_ref())
                .ok_or(Error::InvalidEvent(
                    "collapse completion requires a collapse View",
                ))?;
            if *revision > config.revision {
                return Err(Error::CollapseRevision {
                    current: config.revision,
                    incoming: *revision,
                });
            }
        }
        if let Self::Sheet {
            from,
            revision,
            command,
        } = self
        {
            if from.runtime() != target.runtime() {
                return Err(Error::ForeignRuntime);
            }
            let config = node
                .semantics
                .as_ref()
                .and_then(|s| s.layer.as_ref())
                .and_then(|layer| layer.sheet.as_ref())
                .ok_or(Error::InvalidEvent("sheet command requires a sheet"))?;
            if *from != target {
                return Err(Error::InvalidEvent(
                    "sheet command origin must equal its target",
                ));
            }
            if *revision > config.revision {
                return Err(Error::SheetRevision {
                    current: config.revision,
                    incoming: *revision,
                });
            }
            if *revision == config.revision
                && match command {
                    SheetCommand::Snap(index) => *index >= config.fractions.len(),
                    SheetCommand::Dismiss => !config.dismissible,
                }
            {
                return Err(Error::InvalidEvent(
                    "sheet command exceeds its accepted configuration",
                ));
            }
        }
        if let Self::Dismiss(reason) = self {
            let layer = node
                .semantics
                .as_ref()
                .and_then(|s| s.layer.as_ref())
                .ok_or(Error::InvalidEvent("dismiss requires a layer"))?;
            if !match reason {
                DismissReason::Gesture => {
                    layer.sheet.as_ref().is_some_and(|sheet| sheet.dismissible)
                }
                DismissReason::Escape => layer.escape,
                DismissReason::Outside => layer.outside,
                DismissReason::AnchorLost => layer.anchor.is_some(),
            } {
                return Err(Error::InvalidEvent("disabled layer dismissal"));
            }
        }
        if self
            .origin()
            .is_some_and(|id| id.runtime() != target.runtime())
        {
            return Err(Error::ForeignRuntime);
        }
        self.kind().validate_target(node)?;
        if matches!(
            self,
            Self::Choice {
                command: ChoiceCommand::RemoveLast,
                ..
            }
        ) && node.semantics.as_ref().and_then(|value| value.combobox)
            != Some(ComboboxKind::Multiple)
        {
            return Err(Error::InvalidEvent(
                "remove-last requires a multiple combobox",
            ));
        }
        let local = |rect: &Rect| {
            rect.x == 0.0
                && rect.y == 0.0
                && rect.width.is_finite()
                && rect.height.is_finite()
                && rect.width >= 0.0
                && rect.height >= 0.0
        };
        match self {
            Self::ContextMenu { from, point, .. } => {
                if from.runtime() != target.runtime() {
                    return Err(Error::ForeignRuntime);
                }
                point.validate()?;
                EventKind::ContextMenu.validate_target(node)?;
            }
            Self::Spin { from, command } => {
                EventKind::Spin.validate_target(node)?;
                if *from != target {
                    return Err(Error::InvalidEvent(
                        "spin command requires its physical origin",
                    ));
                }
                let spin = node
                    .semantics
                    .as_ref()
                    .filter(|s| !s.disabled && !s.readonly && node.props.enabled())
                    .and_then(|s| s.spinbutton.as_ref())
                    .ok_or(Error::InvalidEvent(
                        "spin command requires an enabled writable input",
                    ))?;
                spin.validate_command(*command)?;
            }
            Self::Range { from, command } => {
                let range = node
                    .semantics
                    .as_ref()
                    .filter(|s| !s.disabled)
                    .and_then(|s| {
                        s.splitter
                            .or_else(|| s.slider.as_ref().map(|slider| slider.range))
                    })
                    .ok_or(Error::InvalidEvent(
                        "range command requires an enabled slider or splitter",
                    ))?;
                if *from != target {
                    return Err(Error::InvalidEvent(
                        "range command requires its physical origin",
                    ));
                }
                command.validate(range)?;
            }
            Self::Choice { from, .. } => {
                if *from != target {
                    return Err(Error::InvalidEvent(
                        "choice command requires the combobox physical origin",
                    ));
                }
            }
            Self::ScrollKeyboard { from, command } => {
                if *from != target
                    || !node
                        .semantics
                        .as_ref()
                        .and_then(|s| s.scroll_keyboard)
                        .is_some_and(|axes| axes.allows(command.axis))
                {
                    return Err(Error::InvalidEvent(
                        "scroll keyboard requires its opted-in physical viewport and axis",
                    ));
                }
            }
            Self::PointerWithin { related, .. } => {
                if related.is_some_and(|related| related.runtime() != target.runtime()) {
                    return Err(Error::ForeignRuntime);
                }
            }
            Self::NavigationPanel { from, command } => {
                if *from != target || matches!(node.props, Props::Group) {
                    return Err(Error::InvalidEvent(
                        "navigation panel requires its exact physical origin",
                    ));
                }
                let marker = node.semantics.as_ref().and_then(|s| s.navigation_panel);
                let member = node.semantics.as_ref().is_some_and(|s| s.navigation_item);
                if *command == NavigationPanelCommand::Close {
                    if !member && marker != Some(NavigationPanel::Content) {
                        return Err(Error::InvalidEvent(
                            "navigation close requires a content member",
                        ));
                    }
                } else if marker != Some(NavigationPanel::Trigger) {
                    return Err(Error::InvalidEvent(
                        "navigation entry requires a disclosure trigger",
                    ));
                }
            }
            Self::Menu { from, .. } | Self::Pointer { from, .. } => {
                if *from != target || matches!(node.props, Props::Group) {
                    return Err(Error::InvalidEvent(
                        "menu/pointer requires its physical origin",
                    ));
                }
                if matches!(self, Self::Menu { .. })
                    && node.role != Some(Role::Menu)
                    && !node
                        .semantics
                        .as_ref()
                        .is_some_and(|s| s.menu_item.is_some() || s.popup == Some(PopupKind::Menu))
                {
                    return Err(Error::InvalidEvent(
                        "menu command requires a menu or menu trigger/item",
                    ));
                }
                if let Self::Pointer {
                    related: Some(related),
                    ..
                } = self
                    && related.runtime() != target.runtime()
                {
                    return Err(Error::ForeignRuntime);
                }
            }
            Self::Typeahead {
                from,
                text,
                at_millis,
            } => {
                if *from != target || matches!(node.props, Props::Group) {
                    return Err(Error::InvalidEvent(
                        "typeahead requires its physical origin",
                    ));
                }
                validate_typeahead(text, *at_millis)?;
            }
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
            Self::Palette { from } => {
                if from.runtime() != target.runtime() {
                    return Err(Error::ForeignRuntime);
                }
                if *from != target {
                    return Err(Error::InvalidEvent(
                        "palette shortcut requires its physical origin",
                    ));
                }
                EventKind::Palette.validate_target(node)?;
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
            Self::SelectChanged { keys, revision } => {
                if let Props::Select(select) = node.props {
                    select.validate_user_selection(keys)?;
                    if *revision > select.selection.revision {
                        return Err(Error::SelectRevision {
                            current: select.selection.revision,
                            incoming: *revision,
                        });
                    }
                }
            }
            Self::ImageStatus { revision, .. } => {
                if let Props::Image(image) = node.props
                    && *revision > image.revision
                {
                    return Err(Error::ImageRevision {
                        current: image.revision,
                        incoming: *revision,
                    });
                }
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
            Self::Tags {
                from,
                revision,
                command,
            } => {
                if *from != target {
                    return Err(Error::InvalidEvent(
                        "tags command requires its exact input origin",
                    ));
                }
                command.validate()?;
                if let Props::Input(input) = node.props
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
            Self::GridNavigate { .. } => EventKind::GridNavigate,
            Self::InputEdit(_) => EventKind::InputEdit,
            Self::CollapseComplete { .. } => EventKind::CollapseComplete,
            Self::ContextMenu { .. } => EventKind::ContextMenu,
            Self::Tags { .. } => EventKind::Tags,
            Self::Spin { .. } => EventKind::Spin,
            Self::Range { .. } => EventKind::Range,
            Self::Sheet { .. } => EventKind::Sheet,
            Self::NavigationPanel { .. } => EventKind::NavigationPanel,
            Self::ScrollKeyboard { .. } => EventKind::ScrollKeyboard,
            Self::ImageStatus { .. } => EventKind::ImageStatus,
            Self::Menu { .. } => EventKind::Menu,
            Self::Pointer { .. } => EventKind::Pointer,
            Self::PointerWithin { .. } => EventKind::PointerWithin,
            Self::Submit { .. } => EventKind::Submit,
            Self::Reset => EventKind::Reset,
            Self::Activate => EventKind::Activate,
            Self::Dismiss(_) => EventKind::Dismiss,
            Self::ColorSchemeChanged { .. } => EventKind::ColorSchemeChanged,
            Self::Keyboard { .. } => EventKind::Keyboard,
            Self::Palette { .. } => EventKind::Palette,
            Self::Drag { .. } => EventKind::Drag,
            Self::ValueChanged { .. } => EventKind::ValueChanged,
            Self::Commit { .. } => EventKind::Commit,
            Self::CheckedChanged(_) => EventKind::CheckedChanged,
            Self::SelectChanged { .. } => EventKind::SelectChanged,
            Self::Choice { .. } => EventKind::Choice,
            Self::FocusChanged(_) => EventKind::FocusChanged,
            Self::FocusWithin { .. } => EventKind::FocusWithin,
            Self::Navigate { .. } => EventKind::Navigate,
            Self::Typeahead { .. } => EventKind::Typeahead,
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
    SelectRevision {
        current: u64,
        incoming: u64,
    },
    SelectRevisionExhausted,
    ImageRevision {
        current: u64,
        incoming: u64,
    },
    ImageRevisionExhausted,
    SheetRevision {
        current: u64,
        incoming: u64,
    },
    SheetRevisionExhausted,
    CollapseRevision {
        current: u64,
        incoming: u64,
    },
    CollapseRevisionExhausted,
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
    fn preflight(&self, _node: DescriptorView<'_>, _patch: Option<&Patch>) -> Result<(), Error> {
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

mod table;
pub use table::{MAX_CELL_HEADERS, MAX_TABLE_SPAN, SortDirection, TableMetadata};
