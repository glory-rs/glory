use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::rc::Rc;

use glory_style::Style;

use crate::{Change, Error, normalize_text};
mod live;
pub use live::LiveDescriptor;
mod view;
pub use view::DescriptorView;

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
    Code,
    Kbd,
    Mark,
}
impl TextKind {
    pub const fn is_inline(self) -> bool {
        matches!(self, Self::Inline | Self::Code | Self::Kbd | Self::Mark)
    }
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
    /// Interactive numeric range control; finite configuration is required.
    Slider,
    /// Intrinsic editable numeric input; use spinbutton metadata on Input.
    SpinButton,
    /// Read-only numeric or indeterminate progress, never a range input.
    ProgressBar,
    /// Passive document boundary; resizing/focus belongs to a future splitter.
    Separator,
    Group,
    Main,
    Banner,
    ContentInfo,
    Section,
    Navigation,
    List,
    ListItem,
    Status,
    Alert,
    /// Read-only tabular data; rows contain column headers or cells.
    Table,
    Grid,
    GridCell,
    Tree,
    TreeItem,
    Row,
    ColumnHeader,
    Cell,
    /// A named dialog. Modality and focus management belong to the host.
    Dialog,
    /// A modal decision with an explicit safe initial action.
    AlertDialog,
    /// Passive supplemental description; its descendants cannot be interactive.
    Tooltip,
    TabList,
    TabPanel,
    RadioGroup,
    Toolbar,
    Menu,
    Menubar,
    Listbox,
    Heading(u8),
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

/// Finite native check semantics. Mixed preserves indeterminacy independently
/// of the stored checked bit; Switch and Radio never expose a mixed state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckMode {
    #[default]
    Checkbox,
    Mixed,
    Switch,
    Radio,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckProps {
    pub mode: CheckMode,
    pub checked: bool,
    pub label: String,
    pub enabled: bool,
}

impl Default for CheckProps {
    fn default() -> Self {
        Self {
            mode: CheckMode::Checkbox,
            checked: false,
            label: String::new(),
            enabled: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImageProps {
    pub source: String,
    /// Monotonic program request identity, independent of URL equality.
    pub revision: u64,
    /// None is explicitly decorative; Some("") is an empty description.
    pub alternative: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageStatus {
    Loaded,
    Failed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkProps {
    pub target: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Props {
    #[default]
    View,
    Group,
    Text(TextProps),
    Button(ButtonProps),
    Input(InputProps),
    Select(crate::SelectProps),
    File(crate::FileProps),
    RichEditor(crate::RichEditorProps),
    Check(CheckProps),
    Image(ImageProps),
    Graphic(crate::GraphicProps),
    Link(LinkProps),
}

impl Props {
    pub fn interactive(&self) -> bool {
        matches!(
            self,
            Self::Button(_)
                | Self::Input(_)
                | Self::Select(_)
                | Self::File(_)
                | Self::RichEditor(_)
                | Self::Check(_)
                | Self::Link(_)
        )
    }

    pub fn enabled(&self) -> bool {
        match self {
            Self::Button(p) => p.enabled,
            Self::Input(p) => p.enabled,
            Self::Select(p) => p.enabled,
            Self::File(p) => p.enabled,
            Self::RichEditor(p) => p.enabled,
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
    pub semantics: Option<Box<crate::Semantics>>,
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
    #[inline]
    pub fn view(&self) -> DescriptorView<'_> {
        self.into()
    }
    pub fn semantic_id(&self) -> Option<&crate::SemanticId> {
        self.view().semantic_id()
    }
    pub fn hidden(&self) -> bool {
        self.view().hidden()
    }
    pub fn activation_enabled(&self) -> bool {
        self.view().activation_enabled()
    }
    pub fn readonly(&self) -> bool {
        self.view().readonly()
    }
    pub fn field_part(&self) -> Option<crate::FieldPart> {
        self.view().field_part()
    }
    pub fn document_part(&self) -> Option<crate::DocumentPart> {
        self.view().document_part()
    }
    pub fn effective_role(&self) -> Option<Role> {
        self.view().effective_role()
    }
    pub fn validate(&self) -> Result<(), Error> {
        self.view().validate()
    }

    pub fn new(props: Props) -> Self {
        Self {
            props,
            ..Self::default()
        }
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
        if let Props::Image(image) = &mut self.props
            && let std::borrow::Cow::Owned(source) = normalize_text(&image.source, true)
        {
            image.source = source;
        }
        Ok(())
    }

    fn validate_input_revision(&self, next: &Self) -> Result<(), Error> {
        if let (Some(old), Some(next)) = (
            self.semantics.as_ref().and_then(|s| s.clipboard.as_ref()),
            next.semantics.as_ref().and_then(|s| s.clipboard.as_ref()),
        ) {
            next.validate_update(old)?;
        }
        crate::swipe::revision(
            self.semantics.as_ref().and_then(|s| s.swipe_capture),
            next.semantics.as_ref().and_then(|s| s.swipe_capture),
        )?;
        crate::collapse::revision(
            self.semantics.as_ref().and_then(|s| s.collapse.as_ref()),
            next.semantics.as_ref().and_then(|s| s.collapse.as_ref()),
        )?;
        if let (Props::RichEditor(old), Props::RichEditor(next)) = (&self.props, &next.props) {
            next.validate_update(old)?;
        }
        if let (Props::File(old), Props::File(new)) = (&self.props, &next.props) {
            new.validate_update(old)?;
        }
        if let (Props::Graphic(old), Props::Graphic(new)) = (&self.props, &next.props) {
            new.transition_from(old)?;
        }
        crate::sheet::revision(
            self.semantics.as_ref().and_then(|s| s.layer.as_ref()),
            next.semantics.as_ref().and_then(|s| s.layer.as_ref()),
        )?;
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
        if let (Props::Select(old), Props::Select(next)) = (&self.props, &next.props)
            && (next.selection.revision < old.selection.revision
                || next.selection.revision == old.selection.revision
                    && next.selection.keys != old.selection.keys)
        {
            return Err(Error::SelectRevision {
                current: old.selection.revision,
                incoming: next.selection.revision,
            });
        }
        if let (Props::Image(old), Props::Image(next)) = (&self.props, &next.props)
            && (next.revision < old.revision
                || next.revision == old.revision
                    && normalize_text(&old.source, true).as_ref() != next.source)
        {
            return Err(Error::ImageRevision {
                current: old.revision,
                incoming: next.revision,
            });
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
            (Patch::Label(change), Props::File(p)) => {
                change.apply_or(&mut p.label, "Choose file".into())
            }
            (Patch::Enabled(change), Props::RichEditor(p)) => change.apply_or(&mut p.enabled, true),
            (Patch::RichDocument { document, revision }, Props::RichEditor(p)) => {
                if !matches!(document, Change::Keep) {
                    document.apply_or(
                        &mut p.document,
                        std::rc::Rc::new(crate::RichDocument::default()),
                    );
                    p.revision = revision;
                }
            }
            (Patch::Enabled(change), Props::File(p)) => change.apply_or(&mut p.enabled, true),
            (
                Patch::File {
                    configuration,
                    revision,
                },
                Props::File(p),
            ) => {
                if !matches!(configuration, Change::Keep) {
                    match configuration {
                        Change::Set(value) => p.configuration = *value,
                        Change::Clear => p.configuration = crate::FileConfig::default(),
                        Change::Keep => {}
                    }
                    p.revision = revision;
                }
            }
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
            (Patch::Enabled(change), Props::Select(p)) => change.apply_or(&mut p.enabled, true),
            (Patch::SelectSelection { keys, revision }, Props::Select(p)) => {
                if !matches!(keys, Change::Keep) {
                    keys.apply(&mut p.selection.keys);
                    p.selection.revision = revision;
                }
            }
            (Patch::Purpose(change), Props::Input(p)) => change.apply(&mut p.purpose),
            (Patch::Multiline(change), Props::Input(p)) => change.apply(&mut p.multiline),
            (Patch::Checked(change), Props::Check(p)) => change.apply(&mut p.checked),
            (Patch::CheckMode(change), Props::Check(p)) => change.apply(&mut p.mode),
            (Patch::Source { source, revision }, Props::Image(p)) => {
                if !matches!(source, Change::Keep) {
                    source.apply(&mut p.source);
                    p.revision = revision;
                }
            }
            (Patch::Alternative(change), Props::Image(p)) => change.apply(&mut p.alternative),
            (Patch::Graphic(change), Props::Graphic(p)) => match change {
                Change::Keep => {}
                Change::Clear => *p = crate::GraphicProps::default(),
                Change::Set(value) => *p = *value,
            },
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
            (Patch::Semantics(change), _) => change.apply(&mut self.semantics),
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
    RichDocument {
        document: Change<std::rc::Rc<crate::RichDocument>>,
        revision: u64,
    },
    SelectSelection {
        keys: Change<Vec<crate::SemanticId>>,
        revision: u64,
    },
    Purpose(Change<Purpose>),
    Multiline(Change<bool>),
    /// An explicit Set/Clear also synchronizes the host control property when
    /// the semantic value is equal (for example a rejected native toggle).
    Checked(Change<bool>),
    CheckMode(Change<CheckMode>),
    Source {
        source: Change<String>,
        revision: u64,
    },
    Alternative(Change<Option<String>>),
    Graphic(Change<Box<crate::GraphicProps>>),
    File {
        configuration: Change<Box<crate::FileConfig>>,
        revision: u64,
    },
    Target(Change<String>),
    Classes(Change<Vec<String>>),
    Style(Change<Box<Style>>),
    Role(Change<Option<Role>>),
    Name(Change<Option<String>>),
    Focus(Change<FocusMode>),
    Item(Change<Option<ItemPosition>>),
    Semantics(Change<Option<Box<crate::Semantics>>>),
}
