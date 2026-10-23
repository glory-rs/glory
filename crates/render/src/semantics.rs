//! Finite composite-widget metadata. Stable names are explicit document keys,
//! independent of NodeId lifetimes and snapshot rekeying.

use crate::{Descriptor, Error, Navigation, Props, Role};

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
    pub id: Option<SemanticId>,
    pub controls: Option<SemanticId>,
    pub labelled_by: Option<SemanticId>,
    pub described_by: Option<SemanticId>,
    pub has_popup: bool,
    pub layer: Option<Layer>,
    pub expanded: Option<bool>,
    pub selected: Option<bool>,
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
}
impl Semantics {
    pub(crate) fn validate(&self, d: &Descriptor) -> Result<(), Error> {
        if self.has_popup && (!matches!(d.props, Props::Button(_)) || self.tab) {
            return Err(Error::InvalidProps(
                "popup triggers require a non-tab button",
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
        if (self.tab || self.expanded.is_some() || self.disabled || self.navigation_item)
            && !matches!(d.props, Props::Button(_))
        {
            return Err(Error::InvalidProps(
                "composite trigger metadata requires a button",
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
        if self.navigation.is_some() && !matches!(d.role, None | Some(Role::Group | Role::TabList))
        {
            return Err(Error::InvalidProps(
                "navigation requires a generic group or tab list",
            ));
        }
        Ok(())
    }
}
