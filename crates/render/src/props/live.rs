//! Independent instance payloads over immutable declarations. Field patches
//! detach metadata separately; ordinary nodes retain a single owned descriptor.
use super::*;
use crate::Plan;

#[derive(Clone, Debug)]
enum Storage {
    // No shared declaration exists for a one-off Create.
    Owned(Box<Descriptor>),
    Planned(Instance),
}

/// Instance state contains no Style, class strings, options or semantics until
/// that particular field changes. Every Memory owns its own mutable payload.
#[derive(Clone, Debug)]
struct Instance {
    props: Props,
    plan: Plan,
    index: usize,
    fields: Option<Box<Fields>>,
}

#[derive(Clone, Debug, Default)]
struct Fields {
    classes: Option<Vec<String>>,
    style: Option<Box<Style>>,
    role: Option<Option<Role>>,
    name: Option<Option<String>>,
    focus: Option<FocusMode>,
    item: Option<Option<ItemPosition>>,
    semantics: Option<Option<Box<crate::Semantics>>>,
}
impl Instance {
    fn declaration(&self) -> &Descriptor {
        &self.plan.nodes()[self.index].descriptor
    }
    #[inline(always)]
    fn view(&self) -> DescriptorView<'_> {
        let base = self.declaration();
        let f = self.fields.as_deref();
        DescriptorView {
            props: &self.props,
            classes: f.and_then(|f| f.classes.as_ref()).unwrap_or(&base.classes),
            style: f.and_then(|f| f.style.as_deref()).unwrap_or(&base.style),
            role: f.and_then(|f| f.role).unwrap_or(base.role),
            name: f.and_then(|f| f.name.as_ref()).unwrap_or(&base.name),
            focus: f.and_then(|f| f.focus).unwrap_or(base.focus),
            item: f.and_then(|f| f.item).unwrap_or(base.item),
            semantics: f
                .and_then(|f| f.semantics.as_ref())
                .unwrap_or(&base.semantics),
            options: &base.options,
        }
    }
}
impl Storage {
    pub(crate) fn owned(d: Descriptor) -> Self {
        Self::Owned(Box::new(d))
    }
    pub(crate) fn plan(plan: &Plan, index: usize) -> Self {
        Self::Planned(Instance {
            props: plan.nodes()[index].descriptor.props.clone(),
            plan: plan.clone(),
            index,
            fields: None,
        })
    }
    #[inline]
    pub fn view(&self) -> DescriptorView<'_> {
        match self {
            Self::Owned(d) => d.view(),
            Self::Planned(i) => i.view(),
        }
    }
    fn props_mut(&mut self) -> &mut Props {
        match self {
            Self::Owned(d) => &mut d.props,
            Self::Planned(i) => &mut i.props,
        }
    }
    pub(crate) fn field_mask(&self) -> u8 {
        match self {
            Self::Owned(_) => 0,
            Self::Planned(i) => i.fields.as_ref().map_or(0, |f| {
                u8::from(f.classes.is_some())
                    | u8::from(f.style.is_some()) << 1
                    | u8::from(f.role.is_some()) << 2
                    | u8::from(f.name.is_some()) << 3
                    | u8::from(f.focus.is_some()) << 4
                    | u8::from(f.item.is_some()) << 5
                    | u8::from(f.semantics.is_some()) << 6
            }),
        }
    }
    pub(crate) fn restore_fields(&mut self, mask: u8) {
        if let Self::Planned(i) = self {
            if let Some(f) = i.fields.as_mut() {
                if mask & 1 == 0 {
                    f.classes = None;
                }
                if mask & 2 == 0 {
                    f.style = None;
                }
                if mask & 4 == 0 {
                    f.role = None;
                }
                if mask & 8 == 0 {
                    f.name = None;
                }
                if mask & 16 == 0 {
                    f.focus = None;
                }
                if mask & 32 == 0 {
                    f.item = None;
                }
                if mask & 64 == 0 {
                    f.semantics = None;
                }
            }
            if mask == 0 {
                i.fields = None;
            }
        }
    }
    fn classes_mut(&mut self) -> &mut Vec<String> {
        match self {
            Self::Owned(d) => &mut d.classes,
            Self::Planned(i) => i
                .fields
                .get_or_insert_with(Default::default)
                .classes
                .get_or_insert_with(|| i.plan.nodes()[i.index].descriptor.classes.clone()),
        }
    }
    fn style_mut(&mut self) -> &mut Style {
        match self {
            Self::Owned(d) => &mut d.style,
            Self::Planned(i) => i
                .fields
                .get_or_insert_with(Default::default)
                .style
                .get_or_insert_with(|| Box::new(i.plan.nodes()[i.index].descriptor.style.clone()))
                .as_mut(),
        }
    }
    fn role_mut(&mut self) -> &mut Option<Role> {
        match self {
            Self::Owned(d) => &mut d.role,
            Self::Planned(i) => i
                .fields
                .get_or_insert_with(Default::default)
                .role
                .get_or_insert_with(|| i.plan.nodes()[i.index].descriptor.role),
        }
    }
    fn name_mut(&mut self) -> &mut Option<String> {
        match self {
            Self::Owned(d) => &mut d.name,
            Self::Planned(i) => i
                .fields
                .get_or_insert_with(Default::default)
                .name
                .get_or_insert_with(|| i.plan.nodes()[i.index].descriptor.name.clone()),
        }
    }
    fn focus_mut(&mut self) -> &mut FocusMode {
        match self {
            Self::Owned(d) => &mut d.focus,
            Self::Planned(i) => i
                .fields
                .get_or_insert_with(Default::default)
                .focus
                .get_or_insert_with(|| i.plan.nodes()[i.index].descriptor.focus),
        }
    }
    fn item_mut(&mut self) -> &mut Option<ItemPosition> {
        match self {
            Self::Owned(d) => &mut d.item,
            Self::Planned(i) => i
                .fields
                .get_or_insert_with(Default::default)
                .item
                .get_or_insert_with(|| i.plan.nodes()[i.index].descriptor.item),
        }
    }
    fn semantics_mut(&mut self) -> &mut Option<Box<crate::Semantics>> {
        match self {
            Self::Owned(d) => &mut d.semantics,
            Self::Planned(i) => i
                .fields
                .get_or_insert_with(Default::default)
                .semantics
                .get_or_insert_with(|| i.plan.nodes()[i.index].descriptor.semantics.clone()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LiveDescriptor(Storage);
impl LiveDescriptor {
    pub(crate) fn owned(d: Descriptor) -> Self {
        Self(Storage::owned(d))
    }
    pub(crate) fn plan(plan: &Plan, index: usize) -> Self {
        Self(Storage::plan(plan, index))
    }
    #[inline]
    pub fn view(&self) -> DescriptorView<'_> {
        self.0.view()
    }
    pub(crate) fn pending_external_options(&self) -> bool {
        matches!(&self.0, Storage::Planned(instance) if instance.index == 0 && crate::choice::plan_external_options(&instance.plan))
    }
    pub(crate) fn field_mask(&self) -> u8 {
        self.0.field_mask()
    }
    pub(crate) fn restore_fields(&mut self, mask: u8) {
        self.0.restore_fields(mask);
    }
    pub(crate) fn swap_patch(&mut self, patch: &mut Patch) -> Result<(), Error> {
        self.0.swap_patch(patch)
    }
    pub(crate) fn patch_live(&mut self, patch: &mut Patch) -> Result<(), Error> {
        self.0.patch_live(patch)
    }
}
impl<'a> From<&'a LiveDescriptor> for DescriptorView<'a> {
    fn from(d: &'a LiveDescriptor) -> Self {
        d.view()
    }
}
fn normalize_input(props: &mut Props) -> Result<(), Error> {
    if let Props::Input(input) = props {
        if !input.multiline && input.value.contains(['\r', '\n']) {
            return Err(Error::InvalidProps(
                "single-line input cannot contain line breaks",
            ));
        }
        if let std::borrow::Cow::Owned(value) = normalize_text(&input.value, input.multiline) {
            input.value = value;
        }
    }
    if let Props::Image(image) = props
        && let std::borrow::Cow::Owned(source) = normalize_text(&image.source, true)
    {
        image.source = source;
    }
    Ok(())
}

fn exchange<T>(change: &mut Change<T>, target: &mut T, default: T) {
    match change {
        Change::Keep => {}
        Change::Set(value) => std::mem::swap(value, target),
        Change::Clear => *change = Change::Set(std::mem::replace(target, default)),
    }
}

impl Storage {
    /// Swap just the touched field into the inverse patch. Does not validate:
    /// the same operation restores a previously valid descriptor on rollback.
    pub(crate) fn swap_patch(&mut self, patch: &mut Patch) -> Result<(), Error> {
        if matches!(
            patch,
            Patch::Classes(Change::Keep)
                | Patch::Style(Change::Keep)
                | Patch::Role(Change::Keep)
                | Patch::Name(Change::Keep)
                | Patch::Focus(Change::Keep)
                | Patch::Item(Change::Keep)
                | Patch::Semantics(Change::Keep)
        ) {
            return Ok(());
        }
        if matches!(
            patch,
            Patch::Classes(_)
                | Patch::Style(_)
                | Patch::Role(_)
                | Patch::Name(_)
                | Patch::Focus(_)
                | Patch::Item(_)
                | Patch::Semantics(_)
        ) {
            match patch {
                Patch::Classes(change) => exchange(change, self.classes_mut(), Vec::new()),
                Patch::Style(change) => match change {
                    Change::Keep => {}
                    Change::Set(style) => std::mem::swap(style.as_mut(), self.style_mut()),
                    Change::Clear => {
                        *change = Change::Set(Box::new(std::mem::take(self.style_mut())))
                    }
                },
                Patch::Role(change) => exchange(change, self.role_mut(), None),
                Patch::Name(change) => exchange(change, self.name_mut(), None),
                Patch::Focus(change) => exchange(change, self.focus_mut(), FocusMode::default()),
                Patch::Item(change) => exchange(change, self.item_mut(), None),
                Patch::Semantics(change) => exchange(change, self.semantics_mut(), None),
                _ => unreachable!(),
            }
            return Ok(());
        }
        match (patch, self.props_mut()) {
            (Patch::Text(change), Props::Text(p)) => {
                exchange(change, &mut p.content, String::new())
            }
            (Patch::TextKind(change), Props::Text(p)) => {
                exchange(change, &mut p.kind, TextKind::default())
            }
            (Patch::Selectable(change), Props::Text(p)) => {
                exchange(change, &mut p.selectable, true)
            }
            (Patch::Label(change), Props::File(p)) => {
                exchange(change, &mut p.label, "Choose file".into())
            }
            (Patch::Enabled(change), Props::RichEditor(p)) => {
                exchange(change, &mut p.enabled, true)
            }
            (Patch::RichDocument { document, revision }, Props::RichEditor(p)) => {
                if !matches!(document, Change::Keep) {
                    exchange(
                        document,
                        &mut p.document,
                        std::rc::Rc::new(crate::RichDocument::default()),
                    );
                    std::mem::swap(revision, &mut p.revision);
                }
            }
            (Patch::Enabled(change), Props::File(p)) => exchange(change, &mut p.enabled, true),
            (
                Patch::File {
                    configuration,
                    revision,
                },
                Props::File(p),
            ) => {
                if !matches!(configuration, Change::Keep) {
                    match configuration {
                        Change::Set(value) => std::mem::swap(value.as_mut(), &mut p.configuration),
                        Change::Clear => {
                            *configuration =
                                Change::Set(Box::new(std::mem::take(&mut p.configuration)))
                        }
                        Change::Keep => {}
                    }
                    std::mem::swap(revision, &mut p.revision);
                }
            }
            (Patch::Label(change), Props::Button(p)) => {
                exchange(change, &mut p.label, String::new())
            }
            (Patch::Label(change), Props::Check(p)) => {
                exchange(change, &mut p.label, String::new())
            }
            (Patch::Label(change), Props::Link(p)) => exchange(change, &mut p.label, String::new()),
            (Patch::Enabled(change), Props::Button(p)) => exchange(change, &mut p.enabled, true),
            (Patch::Enabled(change), Props::Check(p)) => exchange(change, &mut p.enabled, true),
            (Patch::Enabled(change), Props::Input(p)) => exchange(change, &mut p.enabled, true),
            (Patch::Value { value, revision }, Props::Input(p)) => {
                if !matches!(value, Change::Keep) {
                    exchange(value, &mut p.value, String::new());
                    std::mem::swap(revision, &mut p.revision);
                }
            }
            (Patch::Enabled(change), Props::Select(p)) => exchange(change, &mut p.enabled, true),
            (Patch::SelectSelection { keys, revision }, Props::Select(p)) => {
                if !matches!(keys, Change::Keep) {
                    exchange(keys, &mut p.selection.keys, Vec::new());
                    std::mem::swap(revision, &mut p.selection.revision);
                }
            }
            (Patch::Purpose(change), Props::Input(p)) => {
                exchange(change, &mut p.purpose, Purpose::default())
            }
            (Patch::Multiline(change), Props::Input(p)) => {
                exchange(change, &mut p.multiline, false)
            }
            (Patch::Checked(change), Props::Check(p)) => exchange(change, &mut p.checked, false),
            (Patch::CheckMode(change), Props::Check(p)) => {
                exchange(change, &mut p.mode, CheckMode::default())
            }
            (Patch::Source { source, revision }, Props::Image(p)) => {
                if !matches!(source, Change::Keep) {
                    exchange(source, &mut p.source, String::new());
                    std::mem::swap(revision, &mut p.revision);
                }
            }
            (Patch::Alternative(change), Props::Image(p)) => {
                exchange(change, &mut p.alternative, None)
            }
            (Patch::Graphic(change), Props::Graphic(p)) => match change {
                Change::Keep => {}
                Change::Set(value) => std::mem::swap(value.as_mut(), p),
                Change::Clear => *change = Change::Set(Box::new(std::mem::take(p))),
            },
            (Patch::Target(change), Props::Link(p)) => {
                exchange(change, &mut p.target, String::new())
            }
            _ => {
                return Err(Error::InvalidProps(
                    "property does not belong to this widget",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn patch_live(&mut self, inverse: &mut Patch) -> Result<(), Error> {
        let fields_before = self.field_mask();
        let value = matches!(inverse, Patch::Value { value, .. } if !matches!(value, Change::Keep));
        let source =
            matches!(inverse, Patch::Source { source, .. } if !matches!(source, Change::Keep));
        let graphic = matches!(inverse, Patch::Graphic(change) if !matches!(change, Change::Keep));
        let style = matches!(inverse, Patch::Style(change) if !matches!(change, Change::Keep));
        let classes = matches!(inverse, Patch::Classes(change) if !matches!(change, Change::Keep));
        self.swap_patch(inverse)?;
        let result = (|| {
            if let Patch::Semantics(Change::Set(previous)) = &inverse {
                if let Some(next) = self
                    .view()
                    .semantics
                    .as_ref()
                    .and_then(|s| s.clipboard.as_ref())
                {
                    if let Some(old) = previous.as_ref().and_then(|s| s.clipboard.as_ref()) {
                        next.validate_update(old)?
                    } else {
                        next.validate()?
                    }
                }
                crate::swipe::revision(
                    previous.as_ref().and_then(|s| s.swipe_capture),
                    self.view().semantics.as_ref().and_then(|s| s.swipe_capture),
                )?;
                crate::collapse::revision(
                    previous.as_ref().and_then(|s| s.collapse.as_ref()),
                    self.view()
                        .semantics
                        .as_ref()
                        .and_then(|s| s.collapse.as_ref()),
                )?;
                crate::sheet::revision(
                    previous.as_ref().and_then(|s| s.layer.as_ref()),
                    self.view()
                        .semantics
                        .as_ref()
                        .and_then(|s| s.layer.as_ref()),
                )?;
            }
            if let (
                Props::Select(select),
                Patch::SelectSelection {
                    keys: Change::Set(old),
                    revision,
                },
            ) = (self.view().props, &inverse)
            {
                select.validate_selection(&select.selection.keys)?;
                if select.selection.revision < *revision
                    || select.selection.revision == *revision && select.selection.keys != *old
                {
                    return Err(Error::SelectRevision {
                        current: *revision,
                        incoming: select.selection.revision,
                    });
                }
            }
            if let (Props::Graphic(new), Patch::Graphic(Change::Set(old))) =
                (self.view().props, &inverse)
            {
                new.transition_from(old)?;
            }
            if let (
                Props::File(file),
                Patch::File {
                    configuration: Change::Set(old),
                    revision,
                },
            ) = (self.view().props, &inverse)
            {
                file.configuration.validate()?;
                if file.revision < *revision
                    || file.revision == *revision && file.configuration != **old
                {
                    return Err(Error::InvalidProps(
                        "file configuration changes must advance their revision",
                    ));
                }
            }
            if let (Props::File(file), Patch::Label(_)) = (self.view().props, &inverse) {
                file.validate_label()?;
            }
            if graphic && let Props::Graphic(graphic) = self.view().props {
                graphic.validate()?;
            }
            if matches!(inverse, Patch::Target(change) if !matches!(change, Change::Keep))
                && let Props::Link(link) = self.view().props
            {
                link.validate()?;
            }
            let multiline_changed = matches!((self.view().props, &inverse), (Props::Input(input), Patch::Multiline(Change::Set(old))) if input.multiline != *old);
            if value || source || multiline_changed {
                normalize_input(self.props_mut())?;
            }
            if value
                && let (
                    Props::Input(input),
                    Patch::Value {
                        value: Change::Set(old),
                        revision,
                    },
                ) = (self.view().props, &inverse)
                && (input.revision < *revision
                    || input.revision == *revision && input.value != *old)
            {
                return Err(Error::InputRevision {
                    current: *revision,
                    incoming: input.revision,
                });
            }
            if source
                && let (
                    Props::Image(image),
                    Patch::Source {
                        source: Change::Set(old),
                        revision,
                    },
                ) = (self.view().props, &inverse)
                && (image.revision < *revision
                    || image.revision == *revision && image.source != *old)
            {
                return Err(Error::ImageRevision {
                    current: *revision,
                    incoming: image.revision,
                });
            }
            if let (
                Patch::RichDocument {
                    document: Change::Set(previous),
                    revision,
                },
                Props::RichEditor(next),
            ) = (&*inverse, self.view().props)
            {
                next.document.validate()?;
                if next.revision < *revision
                    || next.revision == *revision && next.document != *previous
                {
                    return Err(Error::InputRevision {
                        current: *revision,
                        incoming: next.revision,
                    });
                }
            }
            self.view().validate_fields(false, style, classes)
        })();
        if result.is_err() {
            // The inverse owns only built-in field values, never application
            // callbacks/options. Restoring it cannot run validation or I/O.
            self.swap_patch(inverse)
                .expect("same field restores its valid descriptor");
            self.restore_fields(fields_before);
        }
        result
    }
}

impl PartialEq for LiveDescriptor {
    fn eq(&self, other: &Self) -> bool {
        self.view() == other.view()
    }
}
impl PartialEq<Descriptor> for LiveDescriptor {
    fn eq(&self, other: &Descriptor) -> bool {
        self.view() == other.view()
    }
}
impl PartialEq<LiveDescriptor> for Descriptor {
    fn eq(&self, other: &LiveDescriptor) -> bool {
        self.view() == other.view()
    }
}
