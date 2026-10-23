use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::rc::Rc;

use glory_style::Style;

use crate::{Change, Error, normalize_text};
mod live;

/// Process-local, typed mapping metadata. It is deliberately not a wire format.
#[derive(Clone, Default)]
pub struct Options(HashMap<TypeId, Rc<dyn Any>>);

impl Options {
    pub fn insert<T: 'static>(&mut self, value: T) {
        self.0.insert(TypeId::of::<T>(), Rc::new(value));
    }
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.0.get(&TypeId::of::<T>())?.downcast_ref()
    }
}

impl std::fmt::Debug for Options {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Options")
            .field("types", &self.0.len())
            .finish()
    }
}

impl PartialEq for Options {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self.0.iter().all(|(key, value)| {
                other
                    .0
                    .get(key)
                    .is_some_and(|other| Rc::ptr_eq(value, other))
            })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextKind {
    #[default]
    Inline,
    Paragraph,
    Heading(u8),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Purpose {
    #[default]
    Text,
    Password,
    Email,
    Search,
    Url,
    Tel,
    Number,
}

/// Structural roles belong to View. Other components keep their intrinsic
/// control/text/image semantics rather than impersonating unrelated widgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Group,
    Main,
    Section,
    Navigation,
    List,
    ListItem,
    Status,
    Alert,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextProps {
    pub content: String,
    pub kind: TextKind,
    pub selectable: bool,
}

impl Default for TextProps {
    fn default() -> Self {
        Self {
            content: String::new(),
            kind: TextKind::Inline,
            selectable: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ButtonProps {
    pub label: String,
    pub enabled: bool,
}

impl Default for ButtonProps {
    fn default() -> Self {
        Self {
            label: String::new(),
            enabled: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputProps {
    pub value: String,
    pub revision: u64,
    pub purpose: Purpose,
    pub multiline: bool,
    pub enabled: bool,
}

impl Default for InputProps {
    fn default() -> Self {
        Self {
            value: String::new(),
            revision: 0,
            purpose: Purpose::Text,
            multiline: false,
            enabled: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckProps {
    pub checked: bool,
    pub label: String,
    pub enabled: bool,
}

impl Default for CheckProps {
    fn default() -> Self {
        Self {
            checked: false,
            label: String::new(),
            enabled: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageProps {
    pub source: String,
    /// None is explicitly decorative; Some("") is an empty description.
    pub alternative: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkProps {
    pub target: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Props {
    #[default]
    View,
    Group,
    Text(TextProps),
    Button(ButtonProps),
    Input(InputProps),
    Check(CheckProps),
    Image(ImageProps),
    Link(LinkProps),
}

impl Props {
    pub fn interactive(&self) -> bool {
        matches!(
            self,
            Self::Button(_) | Self::Input(_) | Self::Check(_) | Self::Link(_)
        )
    }

    pub fn enabled(&self) -> bool {
        match self {
            Self::Button(p) => p.enabled,
            Self::Input(p) => p.enabled,
            Self::Check(p) => p.enabled,
            _ => true,
        }
    }

    pub fn accepts_children(&self) -> bool {
        matches!(
            self,
            Self::View | Self::Group | Self::Button(_) | Self::Link(_)
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Descriptor {
    pub props: Props,
    pub classes: Vec<String>,
    pub style: Style,
    pub role: Option<Role>,
    pub name: Option<String>,
    pub focus: FocusMode,
    pub item: Option<ItemPosition>,
    pub options: Options,
}

/// Host-native default, programmatic focus only, or a normal tab stop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FocusMode {
    #[default]
    Auto,
    Programmatic,
    TabStop,
}

/// Position in the logical collection, not in the mounted render window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemPosition {
    pub index: usize,
    /// None represents an unknown total. A known total must exceed index.
    pub count: Option<usize>,
}

impl Descriptor {
    pub fn new(props: Props) -> Self {
        Self {
            props,
            ..Self::default()
        }
    }

    /// Naming an otherwise generic View creates an accessible group; unnamed
    /// generic Views remain only layout boxes. Explicit structural roles win.
    pub fn effective_role(&self) -> Option<Role> {
        self.role.or_else(|| {
            (matches!(self.props, Props::View) && self.name.is_some()).then_some(Role::Group)
        })
    }

    /// Canonicalize stored editable text without changing a program's explicit
    /// revision. Single-line declarations retain their line-break diagnostic.
    pub fn normalize_input(&mut self) -> Result<(), Error> {
        if let Props::Input(input) = &mut self.props {
            if !input.multiline && input.value.contains(['\r', '\n']) {
                return Err(Error::InvalidProps(
                    "single-line input cannot contain line breaks",
                ));
            }
            if let std::borrow::Cow::Owned(value) = normalize_text(&input.value, input.multiline) {
                input.value = value;
            }
        }
        Ok(())
    }

    fn validate_input_revision(&self, next: &Self) -> Result<(), Error> {
        if let (Props::Input(old), Props::Input(next)) = (&self.props, &next.props)
            && (next.revision < old.revision
                || next.revision == old.revision
                    && normalize_text(&old.value, old.multiline).as_ref() != next.value)
        {
            return Err(Error::InputRevision {
                current: old.revision,
                incoming: next.revision,
            });
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), Error> {
        self.validate_fields(true, true, true)
    }

    // Live descriptors are already canonical/validated. A field patch only
    // needs to rescan payloads it changes; cross-field constraints always run.
    fn validate_fields(&self, input: bool, style: bool, classes: bool) -> Result<(), Error> {
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
        if self.props == Props::Group && (self.focus != FocusMode::Auto || self.item.is_some()) {
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
        if self.props == Props::Group
            && (self.style != Style::default()
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

    /// Validate before replacing the live descriptor. An invalid patch does not
    /// partially corrupt a node even though a mutation batch is not atomic.
    pub fn patch(&mut self, patch: Patch) -> Result<(), Error> {
        *self = self.patched(patch)?;
        Ok(())
    }

    pub fn patched(&self, patch: Patch) -> Result<Self, Error> {
        let mut next = self.clone();
        next.apply_patch(patch)?;
        next.normalize_input()?;
        self.validate_input_revision(&next)?;
        next.validate()?;
        Ok(next)
    }

    /// Builder configuration is validated as a whole, so intermediate settings
    /// (e.g. purpose then multiline) do not reject an otherwise valid final node.
    pub fn patches(&mut self, patches: impl IntoIterator<Item = Patch>) -> Result<(), Error> {
        let mut next = self.clone();
        for patch in patches {
            next.apply_patch(patch)?;
        }
        next.normalize_input()?;
        self.validate_input_revision(&next)?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    fn apply_patch(&mut self, patch: Patch) -> Result<(), Error> {
        match (patch, &mut self.props) {
            (Patch::Text(change), Props::Text(p)) => change.apply(&mut p.content),
            (Patch::TextKind(change), Props::Text(p)) => change.apply(&mut p.kind),
            (Patch::Selectable(change), Props::Text(p)) => change.apply_or(&mut p.selectable, true),
            (Patch::Label(change), Props::Button(p)) => change.apply(&mut p.label),
            (Patch::Label(change), Props::Check(p)) => change.apply(&mut p.label),
            (Patch::Label(change), Props::Link(p)) => change.apply(&mut p.label),
            (Patch::Enabled(change), Props::Button(p)) => change.apply_or(&mut p.enabled, true),
            (Patch::Enabled(change), Props::Check(p)) => change.apply_or(&mut p.enabled, true),
            (Patch::Enabled(change), Props::Input(p)) => change.apply_or(&mut p.enabled, true),
            (Patch::Value { value, revision }, Props::Input(p)) => {
                if !matches!(value, Change::Keep) {
                    value.apply(&mut p.value);
                    p.revision = revision;
                }
            }
            (Patch::Purpose(change), Props::Input(p)) => change.apply(&mut p.purpose),
            (Patch::Multiline(change), Props::Input(p)) => change.apply(&mut p.multiline),
            (Patch::Checked(change), Props::Check(p)) => change.apply(&mut p.checked),
            (Patch::Source(change), Props::Image(p)) => change.apply(&mut p.source),
            (Patch::Alternative(change), Props::Image(p)) => change.apply(&mut p.alternative),
            (Patch::Target(change), Props::Link(p)) => change.apply(&mut p.target),
            (Patch::Classes(change), _) => change.apply(&mut self.classes),
            (Patch::Style(change), _) => match change {
                Change::Keep => {}
                Change::Clear => self.style = Style::default(),
                Change::Set(style) => self.style = *style,
            },
            (Patch::Role(change), _) => change.apply(&mut self.role),
            (Patch::Name(change), _) => change.apply(&mut self.name),
            (Patch::Focus(change), _) => change.apply(&mut self.focus),
            (Patch::Item(change), _) => change.apply(&mut self.item),
            _ => {
                return Err(Error::InvalidProps(
                    "property does not belong to this component",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Patch {
    Text(Change<String>),
    TextKind(Change<TextKind>),
    Selectable(Change<bool>),
    Label(Change<String>),
    Enabled(Change<bool>),
    Value {
        value: Change<String>,
        revision: u64,
    },
    Purpose(Change<Purpose>),
    Multiline(Change<bool>),
    Checked(Change<bool>),
    Source(Change<String>),
    Alternative(Change<Option<String>>),
    Target(Change<String>),
    Classes(Change<Vec<String>>),
    Style(Change<Box<Style>>),
    Role(Change<Option<Role>>),
    Name(Change<Option<String>>),
    Focus(Change<FocusMode>),
    Item(Change<Option<ItemPosition>>),
}
