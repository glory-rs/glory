//! Read-only effective fields, borrowed without materializing a Descriptor.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DescriptorView<'a> {
    pub props: &'a Props,
    pub classes: &'a Vec<String>,
    pub style: &'a Style,
    pub role: Option<Role>,
    pub name: &'a Option<String>,
    pub focus: FocusMode,
    pub item: Option<ItemPosition>,
    pub options: &'a Options,
    pub semantics: &'a Option<Box<crate::Semantics>>,
}
impl<'a> DescriptorView<'a> {
    pub fn view(&self) -> Self {
        *self
    }
    /// Explicit full snapshot for an owned declaration, never a patch read.
    pub fn to_owned(self) -> Descriptor {
        Descriptor {
            props: self.props.clone(),
            classes: self.classes.clone(),
            style: self.style.clone(),
            role: self.role,
            name: self.name.clone(),
            focus: self.focus,
            item: self.item,
            options: self.options.clone(),
            semantics: self.semantics.clone(),
        }
    }
    pub fn semantic_id(&self) -> Option<&'a crate::SemanticId> {
        self.semantics.as_ref().and_then(|s| s.id.as_ref())
    }
    pub fn hidden(&self) -> bool {
        self.semantics.as_ref().is_some_and(|s| s.hidden)
    }
    pub fn activation_enabled(&self) -> bool {
        self.props.enabled() && !self.semantics.as_ref().is_some_and(|s| s.disabled)
    }
    pub fn readonly(&self) -> bool {
        self.semantics.as_ref().is_some_and(|s| s.readonly)
    }
    pub fn field_part(&self) -> Option<crate::FieldPart> {
        self.semantics.as_ref().and_then(|s| s.field_part)
    }
    pub fn document_part(&self) -> Option<crate::DocumentPart> {
        self.semantics.as_ref().and_then(|s| s.document_part)
    }
    /// Naming an otherwise generic View creates an accessible group; unnamed
    /// generic Views remain only layout boxes. Explicit structural roles win.
    pub fn effective_role(&self) -> Option<Role> {
        if self.document_part().is_some() {
            return None;
        }
        if self.field_part() == Some(crate::FieldPart::Form) {
            return None;
        }
        if matches!(self.props, Props::Input(_))
            && self
                .semantics
                .as_ref()
                .is_some_and(|s| s.spinbutton.is_some())
        {
            return Some(Role::SpinButton);
        }
        self.role.or_else(|| {
            (matches!(self.props, Props::View)
                && (self.name.is_some()
                    || self
                        .semantics
                        .as_ref()
                        .is_some_and(|s| s.labelled_by.is_some())))
            .then_some(Role::Group)
        })
    }

    pub fn validate(&self) -> Result<(), Error> {
        self.validate_fields(true, true, true)
    }

    // Live descriptors are already canonical/validated. A field patch only
    // needs to rescan payloads it changes; cross-field constraints always run.
    pub(crate) fn validate_fields(
        &self,
        input: bool,
        style: bool,
        classes: bool,
    ) -> Result<(), Error> {
        if self.role == Some(Role::SpinButton) {
            return Err(Error::InvalidProps(
                "spinbutton is intrinsic to numeric Input metadata",
            ));
        }
        if self.role == Some(Role::Menubar) && self.semantics.is_none() {
            return Err(Error::InvalidProps(
                "menubar role requires finite navigation metadata",
            ));
        }
        if self.role == Some(Role::Slider)
            && self.semantics.as_ref().is_none_or(|s| s.slider.is_none())
        {
            return Err(Error::InvalidProps(
                "slider role requires finite slider metadata",
            ));
        }
        if self.role == Some(Role::ProgressBar)
            && (self.focus != FocusMode::Auto
                || self.semantics.as_ref().is_none_or(|s| s.progress.is_none()))
        {
            return Err(Error::InvalidProps(
                "progress bars require read-only progress metadata and no focus",
            ));
        }
        if self.role == Some(Role::Separator)
            && self.focus != FocusMode::Auto
            && !self
                .semantics
                .as_ref()
                .is_some_and(|s| s.splitter.is_some())
        {
            return Err(Error::InvalidProps(
                "passive separators cannot be focusable",
            ));
        }
        if self.role == Some(Role::Tooltip) && self.focus != FocusMode::Auto {
            return Err(Error::InvalidProps("tooltips cannot receive focus"));
        }
        if let Some(semantics) = &self.semantics {
            semantics.validate(*self)?;
        }
        if matches!(self.role, Some(Role::Heading(level)) if !(1..=6).contains(&level)) {
            return Err(Error::InvalidProps("heading level must be 1..=6"));
        }
        if style {
            self.style.validate().map_err(Error::InvalidProps)?;
        }
        if let Some(item) = self.item
            && (self.role != Some(Role::ListItem)
                || item.index == usize::MAX
                || item.count.is_some_and(|count| item.index >= count))
        {
            return Err(Error::InvalidProps(
                "item position requires listitem role and a valid index/total",
            ));
        }
        if *self.props == Props::Group && (self.focus != FocusMode::Auto || self.item.is_some()) {
            return Err(Error::InvalidProps(
                "group has no focus box or item position",
            ));
        }
        if matches!(&self.props, Props::Text(TextProps { kind: TextKind::Heading(level), .. }) if !(1..=6).contains(level))
        {
            return Err(Error::InvalidProps("heading level must be 1..=6"));
        }
        if matches!(&self.props, Props::Input(p) if p.multiline && p.purpose != Purpose::Text) {
            return Err(Error::InvalidProps("multiline input requires text purpose"));
        }
        if input
            && matches!(&self.props, Props::Input(p) if !p.multiline && p.value.contains(['\r', '\n']))
        {
            return Err(Error::InvalidProps(
                "single-line input cannot contain line breaks",
            ));
        }
        if *self.props == Props::Group
            && (*self.style != Style::default()
                || !self.classes.is_empty()
                || self.role.is_some()
                || self.name.is_some())
        {
            return Err(Error::InvalidProps(
                "group has no style box or accessibility role",
            ));
        }
        if self.role.is_some() && !matches!(self.props, Props::View) {
            return Err(Error::InvalidProps("structural roles require a view"));
        }
        if input && let Props::Select(select) = self.props {
            select.validate()?;
        }
        if input && let Props::Graphic(graphic) = self.props {
            graphic.validate()?;
        }
        if matches!(self.props, Props::Graphic(graphic) if graphic.alternative.is_none())
            && (self.name.is_some() || self.focus != FocusMode::Auto)
        {
            return Err(Error::InvalidProps(
                "decorative graphics cannot be named or focusable",
            ));
        }
        if self.name.is_some()
            && matches!(&self.props, Props::Text(text) if !matches!(text.kind, TextKind::Heading(_)))
        {
            return Err(Error::InvalidProps(
                "only heading text accepts an accessible name",
            ));
        }
        if matches!(&self.props, Props::Image(image) if image.alternative.is_none())
            && (self.name.is_some() || self.focus != FocusMode::Auto)
        {
            return Err(Error::InvalidProps(
                "decorative image has no name or focus target",
            ));
        }
        if classes
            && self
                .classes
                .iter()
                .any(|class| class.is_empty() || class.chars().any(char::is_whitespace))
        {
            return Err(Error::InvalidProps(
                "classes must be individual nonempty tokens",
            ));
        }
        Ok(())
    }
}
impl<'a> From<&'a Descriptor> for DescriptorView<'a> {
    fn from(d: &'a Descriptor) -> Self {
        Self {
            props: &d.props,
            classes: &d.classes,
            style: &d.style,
            role: d.role,
            name: &d.name,
            focus: d.focus,
            item: d.item,
            options: &d.options,
            semantics: &d.semantics,
        }
    }
}
impl<'a> From<&DescriptorView<'a>> for DescriptorView<'a> {
    fn from(d: &DescriptorView<'a>) -> Self {
        *d
    }
}
impl PartialEq<Descriptor> for DescriptorView<'_> {
    fn eq(&self, d: &Descriptor) -> bool {
        *self == d.view()
    }
}
impl PartialEq<DescriptorView<'_>> for Descriptor {
    fn eq(&self, d: &DescriptorView<'_>) -> bool {
        self.view() == *d
    }
}

impl<'a> From<&'a Box<Descriptor>> for DescriptorView<'a> {
    fn from(d: &'a Box<Descriptor>) -> Self {
        d.as_ref().view()
    }
}
