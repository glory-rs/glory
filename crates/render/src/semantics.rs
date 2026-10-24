//! Finite composite-widget metadata. Stable names are explicit document keys,
//! independent of NodeId lifetimes and snapshot rekeying.

use crate::{Error, Navigation, Props, Role};

/// Structural form content, fixed for a node's lifetime on HTML adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldPart {
    Form,
    Label,
    Fieldset,
    Legend,
}

/// The host captures requests; application policy owns validation and transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormAction {
    Submit,
    Reset,
}

/// Submission metadata is separate from the accessible name. Text input values
/// come from InputProps; Check/Button may supply an explicit successful value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormControl {
    pub name: String,
    pub value: Option<String>,
}

/// Native links may use their current context or request a separate one.
/// Opening a system browser remains an outer native host service.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinkOpen {
    #[default]
    Current,
    NewContext,
}

/// The current location in a related set, independent of selection or visits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Current {
    Page,
    Step,
}
/// The popup role is one typed value, never an independent boolean flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupKind {
    Dialog,
    Menu,
    Listbox,
}
impl PopupKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dialog => "dialog",
            Self::Menu => "menu",
            Self::Listbox => "listbox",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItemKind {
    Action,
    Checkbox,
    Radio,
}
/// Finite intrinsic-control refinement. Owner is a named Menu View, also used
/// to keep menu check controls outside any enclosing successful form controls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItemSemantics {
    pub kind: MenuItemKind,
    pub owner: SemanticId,
}

/// Host-managed top layer. Presence declares a focus scope; `hidden` controls
/// whether it participates in the stack. Relations resolve only live names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayerFocus {
    #[default]
    Manage,
    Preserve,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub sheet: Option<crate::SheetConfig>,
    pub modal: bool,
    pub escape: bool,
    pub outside: bool,
    pub focus: LayerFocus,
    pub placement: LayerPlacement,
    pub anchor: Option<SemanticId>,
    pub initial_focus: Option<SemanticId>,
    pub return_focus: Option<SemanticId>,
}
impl Default for Layer {
    fn default() -> Self {
        Self {
            sheet: None,
            modal: true,
            escape: true,
            outside: true,
            focus: LayerFocus::Manage,
            placement: LayerPlacement::Center,
            anchor: None,
            initial_focus: None,
            return_focus: None,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayerPlacement {
    #[default]
    Center,
    Left,
    Right,
    Top,
    Bottom,
    AnchorStart,
    AnchorEnd,
    AnchorRight,
    AnchorLeft,
}
impl LayerPlacement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Center => "center",
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::AnchorStart => "anchor-start",
            Self::AnchorEnd => "anchor-end",
            Self::AnchorRight => "anchor-right",
            Self::AnchorLeft => "anchor-left",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DismissReason {
    Gesture,
    Escape,
    Outside,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SemanticId(String);
impl SemanticId {
    /// Names are bounded ASCII tokens. Applications must choose distinct widget
    /// namespaces when multiple Apps share one host document.
    pub fn new(value: impl Into<String>) -> Result<Self, Error> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 256
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(Error::InvalidProps(
                "semantic id must be a bounded ASCII token",
            ));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}
impl Orientation {
    pub fn key_with_direction(
        self,
        direction: Direction,
        input: crate::KeyboardInput<'_>,
    ) -> Option<Navigation> {
        let command = self.key(input)?;
        Some(match (self, direction, input.key, command) {
            (Self::Horizontal, Direction::Rtl, "ArrowLeft" | "ArrowRight", Navigation::Next) => {
                Navigation::Previous
            }
            (
                Self::Horizontal,
                Direction::Rtl,
                "ArrowLeft" | "ArrowRight",
                Navigation::Previous,
            ) => Navigation::Next,
            _ => command,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
    /// Shared direction mapping. Tab, editing keys, modified/composing keys and
    /// the perpendicular arrows remain host-native. Repeats may navigate.
    pub fn key(self, input: crate::KeyboardInput<'_>) -> Option<Navigation> {
        if input.composing
            || input.editable
            || input.control
            || input.meta
            || input.alt
            || input.shift
        {
            return None;
        }
        match (self, input.key) {
            (Self::Horizontal, "ArrowLeft") | (Self::Vertical, "ArrowUp") => {
                Some(Navigation::Previous)
            }
            (Self::Horizontal, "ArrowRight") | (Self::Vertical, "ArrowDown") => {
                Some(Navigation::Next)
            }
            (_, "Home") => Some(Navigation::First),
            (_, "End") => Some(Navigation::Last),
            _ => None,
        }
    }
}

/// Optional and finite: ordinary nodes allocate none of this metadata. Named
/// relations may refer forward or to an unmounted target; adapters resolve only
/// live names in their document. Removing a target retains no owning reference.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Semantics {
    /// Opted-in free tag entry policy; retains the Input's intrinsic textbox.
    pub tags_input: Option<crate::TagSeparators>,
    pub progress: Option<crate::ProgressValue>,
    pub splitter: Option<crate::RangeValue>,
    pub slider: Option<crate::SliderValue>,
    /// Opt-in keyboard commands only while this viewport itself owns focus.
    pub scroll_keyboard: Option<crate::ScrollAxes>,
    /// An application-owned pending state on a boxed semantic node.
    pub busy: Option<bool>,
    pub current: Option<Current>,
    pub separator_axis: Option<Orientation>,
    pub link_open: Option<LinkOpen>,
    /// Capture activation as an application request before default navigation.
    pub link_request: bool,
    pub document_part: Option<crate::DocumentPart>,
    pub form_action: Option<FormAction>,
    pub form_control: Option<FormControl>,
    pub field_part: Option<FieldPart>,
    pub label_for: Option<SemanticId>,
    pub error_message: Option<SemanticId>,
    pub required: bool,
    pub readonly: bool,
    pub invalid: bool,
    pub id: Option<SemanticId>,
    pub controls: Option<SemanticId>,
    pub labelled_by: Option<SemanticId>,
    pub described_by: Option<SemanticId>,
    pub popup: Option<PopupKind>,
    pub combobox: Option<crate::ComboboxKind>,
    pub option: Option<crate::OptionSemantics>,
    pub active_descendant: Option<SemanticId>,
    pub listbox_multiple: bool,
    pub menu_item: Option<MenuItemSemantics>,
    pub layer: Option<Layer>,
    /// Passive grip hit area; the nearest live sheet owns pointer gestures.
    pub sheet_grip: bool,
    pub expanded: Option<bool>,
    pub selected: Option<bool>,
    /// A toggle button's application-owned state; independent from disclosure.
    pub pressed: Option<bool>,
    /// Refine a Button's intrinsic interaction to a tab, retaining activation.
    pub tab: bool,
    /// Keep a control focusable, while suppressing activation (e.g. the sole
    /// expanded header in an accordion that cannot collapse completely).
    pub disabled: bool,
    /// Hide a View subtree without destroying its state or removing its names.
    pub hidden: bool,
    /// Retain a View's content and layout while suppressing subtree interaction
    /// and accessibility exposure. This is not a scroll lock or a modal scope.
    pub inert: bool,
    /// Explicit collection boundary, independent of virtual-list navigation.
    pub navigation: Option<Orientation>,
    pub navigation_item: bool,
    pub navigation_panel: Option<crate::NavigationPanel>,
    /// Native radio association; not an accessible name or a submission value.
    /// HTML uses the same attribute for native grouping and successful names.
    pub check_group: Option<SemanticId>,
    pub navigation_direction: Direction,
    /// Opt in to bounded character search on the collection's mounted members.
    pub typeahead: bool,
}
impl Semantics {
    pub(crate) fn validate(&self, d: crate::DescriptorView<'_>) -> Result<(), Error> {
        if self.tags_input.is_some()
            && (!matches!(d.props,crate::Props::Input(input) if !input.multiline)
                || self.combobox.is_some()
                || self.popup.is_some()
                || self.controls.is_some()
                || self.active_descendant.is_some()
                || self.form_control.is_some())
        {
            return Err(Error::InvalidProps(
                "tags policy requires an independent single-line search input",
            ));
        }
        if self.sheet_grip
            && (!matches!(d.props, Props::View)
                || d.role.is_some()
                || d.focus != crate::FocusMode::Auto
                || self.layer.is_some()
                || self.navigation.is_some())
        {
            return Err(Error::InvalidProps(
                "sheet grip requires a passive generic view",
            ));
        }
        if let Some(panel) = self.navigation_panel {
            let valid = match panel {
                crate::NavigationPanel::Trigger => {
                    matches!(d.props, Props::Button(_))
                        && self.navigation_item
                        && self.id.is_some()
                        && self.controls.is_some()
                        && self.controls != self.id
                        && self.expanded.is_some()
                        && self.popup.is_none()
                        && self.menu_item.is_none()
                        && !self.tab
                        && self.pressed.is_none()
                        && self.form_action.is_none()
                }
                crate::NavigationPanel::Content => {
                    matches!(d.props, Props::View)
                        && d.role == Some(Role::Group)
                        && d.focus == crate::FocusMode::Auto
                        && self.id.is_some()
                        && self.navigation == Some(Orientation::Vertical)
                        && self.menu_item.is_none()
                        && self.popup.is_none()
                        && self.field_part.is_none()
                        && self.document_part.is_none()
                        && self.scroll_keyboard.is_none()
                }
            };
            if !valid {
                return Err(Error::InvalidProps(
                    "navigation panels require an independent disclosure button or physical group content",
                ));
            }
        }
        if self.inert && !matches!(d.props, Props::View) {
            return Err(Error::InvalidProps("inert regions require a view"));
        }
        self.validate_choice(d)?;
        if let Some(progress) = &self.progress {
            progress.validate()?;
            if d.role != Some(Role::ProgressBar)
                || !matches!(d.props, Props::View)
                || self.navigation.is_some()
                || self.layer.is_some()
                || self.field_part.is_some()
                || self.document_part.is_some()
                || self.splitter.is_some()
                || self.menu_item.is_some()
                || self.popup.is_some()
                || self.pressed.is_some()
                || self.expanded.is_some()
                || self.scroll_keyboard.is_some()
            {
                return Err(Error::InvalidProps(
                    "progress metadata requires an independent progress bar view",
                ));
            }
        }
        if let Some(slider) = &self.slider {
            slider.validate()?;
            if !matches!(d.props, Props::View)
                || d.role != Some(Role::Slider)
                || d.focus == crate::FocusMode::Auto
                || self.splitter.is_some()
                || self.progress.is_some()
                || self.separator_axis.is_some()
                || self.navigation.is_some()
                || self.navigation_item
                || self.layer.is_some()
                || self.field_part.is_some()
                || self.document_part.is_some()
                || self.tab
                || self.pressed.is_some()
                || self.expanded.is_some()
                || self.popup.is_some()
                || self.scroll_keyboard.is_some()
                || self.menu_item.is_some()
                || self.combobox.is_some()
                || self.option.is_some()
            {
                return Err(Error::InvalidProps(
                    "slider requires an independent focused range view",
                ));
            }
        }
        if let Some(range) = self.splitter {
            range.validate()?;
            if !matches!(d.props, Props::View)
                || d.role != Some(Role::Separator)
                || d.focus == crate::FocusMode::Auto
                || self.separator_axis.is_none()
                || self.controls.is_none()
                || self.controls == self.id
                || self.navigation.is_some()
            {
                return Err(Error::InvalidProps(
                    "splitter requires a focused separator, orientation and distinct controlled pane",
                ));
            }
        }
        if self.scroll_keyboard.is_some()
            && (!matches!(d.props, Props::View)
                || self.navigation.is_some()
                || self.navigation_item
                || self.layer.is_some()
                || self.field_part.is_some()
                || self.document_part.is_some()
                || d.role == Some(Role::Separator))
        {
            return Err(Error::InvalidProps(
                "scroll keyboard requires an independent viewport view",
            ));
        }
        if let Some(item) = &self.menu_item {
            let valid = match (&d.props, item.kind) {
                (Props::Button(_), MenuItemKind::Action) => {
                    !self.tab && self.pressed.is_none() && self.form_action.is_none()
                }
                (Props::Check(check), MenuItemKind::Checkbox) => matches!(
                    check.mode,
                    crate::CheckMode::Checkbox | crate::CheckMode::Mixed
                ),
                (Props::Check(check), MenuItemKind::Radio) => {
                    check.mode == crate::CheckMode::Radio && self.check_group.is_some()
                }
                _ => false,
            };
            if !valid
                || self.form_control.is_some()
                || self.id.as_ref() == Some(&item.owner)
                || self.popup.is_some_and(|popup| popup != PopupKind::Menu)
            {
                return Err(Error::InvalidProps(
                    "menu item requires a matching intrinsic control and distinct menu owner",
                ));
            }
        }
        if self.current.is_some()
            && !matches!(d.props, Props::Link(_) | Props::Button(_) | Props::Text(_))
        {
            return Err(Error::InvalidProps("current requires link, button or text"));
        }
        if (self.typeahead || self.navigation_direction != Direction::Ltr)
            && self.navigation.is_none()
            && self.combobox != Some(crate::ComboboxKind::Select)
        {
            return Err(Error::InvalidProps(
                "direction and typeahead require a navigation boundary",
            ));
        }
        if self.separator_axis.is_some() && d.role != Some(Role::Separator) {
            return Err(Error::InvalidProps(
                "separator orientation requires a separator role",
            ));
        }
        if (self.link_open.is_some() || self.link_request) && !matches!(d.props, Props::Link(_)) {
            return Err(Error::InvalidProps("link context requires a link"));
        }
        if self.document_part.is_some()
            && (!matches!(d.props, Props::View)
                || d.role.is_some()
                || d.focus != crate::FocusMode::Auto
                || self.field_part.is_some()
                || self.layer.is_some()
                || self.navigation.is_some())
        {
            return Err(Error::InvalidProps(
                "document parts require an unfocused structural view",
            ));
        }
        if let Some(group) = &self.check_group
            && (!matches!(&d.props, Props::Check(check) if check.mode == crate::CheckMode::Radio)
                || self
                    .form_control
                    .as_ref()
                    .is_some_and(|control| control.name != group.as_str()))
        {
            return Err(Error::InvalidProps(
                "radio association requires a radio and matching successful-control name",
            ));
        }
        if self.form_action.is_some()
            && (!matches!(d.props, Props::Button(_))
                || self.tab
                || self.popup.is_some()
                || self.expanded.is_some()
                || self.pressed.is_some())
        {
            return Err(Error::InvalidProps("form actions require ordinary buttons"));
        }
        if let Some(control) = &self.form_control
            && (!matches!(
                d.props,
                Props::Input(_) | Props::Select(_) | Props::Check(_) | Props::Button(_)
            ) && d.role != Some(Role::Slider)
                || control.name.len() > 256
                || control.name.contains(['\0', '\r', '\n'])
                || control
                    .value
                    .as_ref()
                    .is_some_and(|value| value.contains(['\0', '\r']))
                || control.value.is_some() && matches!(d.props, Props::Input(_) | Props::Select(_)))
        {
            return Err(Error::InvalidProps("invalid successful-control metadata"));
        }
        if self.field_part.is_some()
            && (!matches!(d.props, Props::View)
                || d.role.is_some()
                || d.focus != crate::FocusMode::Auto
                || self.layer.is_some()
                || self.navigation.is_some())
        {
            return Err(Error::InvalidProps(
                "form parts require an unfocused structural view",
            ));
        }
        if self.label_for.is_some() && self.field_part != Some(FieldPart::Label) {
            return Err(Error::InvalidProps(
                "label association requires a label part",
            ));
        }
        if self.label_for.is_some() && self.label_for == self.id {
            return Err(Error::InvalidProps("a label cannot target itself"));
        }
        if self.readonly && matches!(d.props, Props::Select(_)) {
            return Err(Error::Unsupported("native select has no readonly state"));
        }
        let input_field = self.combobox.is_some()
            || matches!(
                d.props,
                Props::Input(_) | Props::Select(_) | Props::Check(_)
            )
            || matches!(d.role, Some(Role::RadioGroup | Role::Slider));
        if ((self.required || self.readonly) && !input_field)
            || ((self.invalid || self.error_message.is_some())
                && !input_field
                && self.field_part != Some(FieldPart::Fieldset))
        {
            return Err(Error::InvalidProps(
                "field states require an input or check",
            ));
        }
        if self.popup.is_some()
            && ((!matches!(d.props, Props::Button(_)) && self.combobox.is_none()) || self.tab)
        {
            return Err(Error::InvalidProps(
                "popup triggers require a non-tab button",
            ));
        }
        if self.pressed.is_some()
            && (!matches!(d.props, Props::Button(_)) || self.tab || self.popup.is_some())
        {
            return Err(Error::InvalidProps(
                "pressed requires a non-tab toggle button",
            ));
        }
        if let Some(layer) = &self.layer {
            if let Some(sheet) = &layer.sheet {
                sheet.validate()?;
                if d.style
                    .position
                    .is_some_and(|p| p != crate::style::Position::Fixed)
                {
                    return Err(Error::InvalidProps("sheet owns fixed viewport positioning"));
                }
                if d.role != Some(Role::Dialog)
                    || !crate::sheet::edge(layer.placement)
                    || layer.focus != LayerFocus::Manage
                    || layer.anchor.is_some()
                {
                    return Err(Error::InvalidProps(
                        "sheet requires an edge dialog with managed focus",
                    ));
                }
            }
            if !matches!(d.props, Props::View)
                || !matches!(
                    d.role,
                    Some(
                        Role::Dialog
                            | Role::AlertDialog
                            | Role::Menu
                            | Role::Tooltip
                            | Role::Listbox
                            | Role::Group
                    )
                )
                || self.id.is_none()
                || (!matches!(d.role, Some(Role::Tooltip | Role::Listbox))
                    && d.focus == crate::FocusMode::Auto
                    && d.role != Some(Role::Group))
                || (layer.focus == LayerFocus::Preserve
                    && (layer.modal
                        || layer.initial_focus.is_some()
                        || layer.return_focus.is_some()))
                || (d.role == Some(Role::Tooltip)
                    && (layer.focus != LayerFocus::Preserve
                        || d.focus != crate::FocusMode::Auto
                        || self.navigation.is_some()
                        || self.popup.is_some()
                        || self.expanded.is_some()))
                || (matches!(d.role, Some(Role::Dialog | Role::AlertDialog))
                    && self.navigation.is_some())
                || (d.role == Some(Role::AlertDialog)
                    && (!layer.modal
                        || layer.focus != LayerFocus::Manage
                        || layer.initial_focus.is_none()))
                || (d.role == Some(Role::Listbox)
                    && (layer.modal
                        || layer.focus != LayerFocus::Preserve
                        || d.focus != crate::FocusMode::Auto
                        || self.navigation != Some(Orientation::Vertical)))
                || (d.role == Some(Role::Menu)
                    && (layer.modal
                        || layer.focus != LayerFocus::Manage
                        || self.navigation != Some(Orientation::Vertical)))
                || (d.role == Some(Role::Group)
                    && (self.navigation_panel != Some(crate::NavigationPanel::Content)
                        || layer.modal
                        || layer.focus != LayerFocus::Preserve
                        || d.focus != crate::FocusMode::Auto))
            {
                return Err(Error::InvalidProps(
                    "layers require a valid named dialog, alert decision, menu or passive tooltip focus policy",
                ));
            }
            if matches!(
                layer.placement,
                LayerPlacement::AnchorStart
                    | LayerPlacement::AnchorEnd
                    | LayerPlacement::AnchorRight
                    | LayerPlacement::AnchorLeft
            ) != layer.anchor.is_some()
                || layer
                    .anchor
                    .as_ref()
                    .is_some_and(|id| Some(id) == self.id.as_ref())
            {
                return Err(Error::InvalidProps(
                    "anchored layers require a distinct anchor",
                ));
            }
        }
        if matches!(d.props, Props::Group) {
            return Err(Error::InvalidProps(
                "group has no composite widget metadata",
            ));
        }
        if (self.tab
            || self.expanded.is_some()
            || (self.disabled
                && self.splitter.is_none()
                && self.slider.is_none()
                && !matches!(self.field_part, Some(FieldPart::Fieldset | FieldPart::Form))))
            && !matches!(d.props, Props::Button(_))
            && self.combobox.is_none()
        {
            return Err(Error::InvalidProps(
                "composite trigger metadata requires a button",
            ));
        }
        if self.navigation_item
            && !matches!(&d.props, Props::Button(_) | Props::Link(_))
            && !matches!(&d.props, Props::Check(check) if check.mode == crate::CheckMode::Radio)
            && self.menu_item.is_none()
        {
            return Err(Error::InvalidProps(
                "navigation items require a button, link or radio",
            ));
        }
        if self.selected.is_some() && !self.tab && self.option.is_none()
            || self.tab && (self.expanded.is_some() || self.selected.is_none())
        {
            return Err(Error::InvalidProps(
                "selected belongs to tabs; tabs cannot be disclosures",
            ));
        }
        if self.navigation.is_some() && !matches!(d.props, Props::View) {
            return Err(Error::InvalidProps(
                "hidden panels and navigation boundaries require a view",
            ));
        }
        if self.hidden && !matches!(d.props, Props::View) && self.option.is_none() {
            return Err(Error::InvalidProps(
                "hidden content requires a view or intrinsic option",
            ));
        }
        if self.labelled_by.is_some()
            && (matches!(&d.props, Props::Text(p) if !matches!(p.kind, crate::TextKind::Heading(_)))
                || matches!(&d.props, Props::Image(p) if p.alternative.is_none())
                || matches!(d.props, Props::Graphic(p) if p.alternative.is_none()))
        {
            return Err(Error::InvalidProps(
                "this node cannot have a naming relation",
            ));
        }
        if self.controls.is_some() && self.id == self.controls {
            return Err(Error::InvalidProps("a control cannot control itself"));
        }
        if self.labelled_by.is_some() && self.id == self.labelled_by {
            return Err(Error::InvalidProps("a node cannot label itself"));
        }
        if self.navigation.is_some()
            && !matches!(
                d.role,
                None | Some(
                    Role::Group
                        | Role::TabList
                        | Role::RadioGroup
                        | Role::Toolbar
                        | Role::Menu
                        | Role::Listbox
                )
            )
        {
            return Err(Error::InvalidProps(
                "navigation requires a generic group or tab list",
            ));
        }
        Ok(())
    }
}
