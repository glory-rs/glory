//! Finite composite-widget metadata. Stable names are explicit document keys,
//! independent of NodeId lifetimes and snapshot rekeying.

use crate::{Descriptor, Error, Navigation, Props, Role};

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

/// Host-managed top layer. Presence declares a focus scope; `hidden` controls
/// whether it participates in the stack. Relations resolve only live names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub modal: bool,
    pub escape: bool,
    pub outside: bool,
    pub placement: LayerPlacement,
    pub anchor: Option<SemanticId>,
    pub initial_focus: Option<SemanticId>,
    pub return_focus: Option<SemanticId>,
}
impl Default for Layer {
    fn default() -> Self {
        Self {
            modal: true,
            escape: true,
            outside: true,
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
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DismissReason {
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
impl Orientation {
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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Semantics {
    pub link_open: Option<LinkOpen>,
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
    pub has_popup: bool,
    pub layer: Option<Layer>,
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
    /// Explicit collection boundary, independent of virtual-list navigation.
    pub navigation: Option<Orientation>,
    pub navigation_item: bool,
    /// Native radio association; not an accessible name or a submission value.
    /// HTML uses the same attribute for native grouping and successful names.
    pub check_group: Option<SemanticId>,
}
impl Semantics {
    pub(crate) fn validate(&self, d: &Descriptor) -> Result<(), Error> {
        if self.link_open.is_some() && !matches!(d.props, Props::Link(_)) {
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
                || self.has_popup
                || self.expanded.is_some()
                || self.pressed.is_some())
        {
            return Err(Error::InvalidProps("form actions require ordinary buttons"));
        }
        if let Some(control) = &self.form_control
            && (!matches!(
                d.props,
                Props::Input(_) | Props::Check(_) | Props::Button(_)
            ) || control.name.len() > 256
                || control.name.contains(['\0', '\r', '\n'])
                || control
                    .value
                    .as_ref()
                    .is_some_and(|value| value.contains(['\0', '\r']))
                || control.value.is_some() && matches!(d.props, Props::Input(_)))
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
        let input_field = matches!(d.props, Props::Input(_) | Props::Check(_))
            || d.role == Some(Role::RadioGroup);
        if ((self.required || self.readonly) && !input_field)
            || ((self.invalid || self.error_message.is_some())
                && !input_field
                && self.field_part != Some(FieldPart::Fieldset))
        {
            return Err(Error::InvalidProps(
                "field states require an input or check",
            ));
        }
        if self.has_popup && (!matches!(d.props, Props::Button(_)) || self.tab) {
            return Err(Error::InvalidProps(
                "popup triggers require a non-tab button",
            ));
        }
        if self.pressed.is_some()
            && (!matches!(d.props, Props::Button(_)) || self.tab || self.has_popup)
        {
            return Err(Error::InvalidProps(
                "pressed requires a non-tab toggle button",
            ));
        }
        if let Some(layer) = &self.layer {
            if !matches!(d.props, Props::View)
                || d.role != Some(Role::Dialog)
                || d.focus == crate::FocusMode::Auto
                || self.id.is_none()
                || self.navigation.is_some()
            {
                return Err(Error::InvalidProps(
                    "layers require a named focusable dialog view",
                ));
            }
            if matches!(
                layer.placement,
                LayerPlacement::AnchorStart | LayerPlacement::AnchorEnd
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
                && !matches!(self.field_part, Some(FieldPart::Fieldset | FieldPart::Form))))
            && !matches!(d.props, Props::Button(_))
        {
            return Err(Error::InvalidProps(
                "composite trigger metadata requires a button",
            ));
        }
        if self.navigation_item
            && !matches!(&d.props, Props::Button(_))
            && !matches!(&d.props, Props::Check(check) if check.mode == crate::CheckMode::Radio)
        {
            return Err(Error::InvalidProps(
                "navigation items require a button or radio",
            ));
        }
        if self.selected.is_some() && !self.tab
            || self.tab && (self.expanded.is_some() || self.selected.is_none())
        {
            return Err(Error::InvalidProps(
                "selected belongs to tabs; tabs cannot be disclosures",
            ));
        }
        if (self.hidden || self.navigation.is_some()) && !matches!(d.props, Props::View) {
            return Err(Error::InvalidProps(
                "hidden panels and navigation boundaries require a view",
            ));
        }
        if self.labelled_by.is_some()
            && (matches!(&d.props, Props::Text(p) if !matches!(p.kind, crate::TextKind::Heading(_)))
                || matches!(&d.props, Props::Image(p) if p.alternative.is_none()))
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
                None | Some(Role::Group | Role::TabList | Role::RadioGroup)
            )
        {
            return Err(Error::InvalidProps(
                "navigation requires a generic group or tab list",
            ));
        }
        Ok(())
    }
}
