use std::collections::BTreeMap;
use std::rc::Rc;

use glory_core::{Cage, Lotus};
use glory_render::{
    ButtonProps, Change, CheckProps, Descriptor, Event, EventKind, FocusMode, ImageProps,
    InputProps, ItemPosition, LinkProps, NodeId, Patch, Props, Purpose, Role, TextKind, TextProps,
};
use glory_style::{Display, FlexDirection, Style};

use crate::app::{Result, Scope, Widget};

type Bind = Box<dyn FnOnce(&Scope, NodeId) -> Result>;
type Prepare = Box<dyn FnOnce() -> Result<(Patch, Bind)>>;
type Handler = Box<dyn FnMut(Event)>;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Slot {
    Classes,
    Style,
    Name,
    Role,
    Focus,
    Item,
    Semantics,
    Text,
    Kind,
    Selectable,
    Label,
    Enabled,
    Value,
    SelectSelection,
    Purpose,
    Multiline,
    Checked,
    CheckMode,
    Source,
    Alternative,
    Graphic,
    Target,
}

// A full initial Graphic declaration has no previous live path to interpolate.
// Seed its props before whole-descriptor construction validation; later bound
// writes still go through the canonical live topology guard.
fn seed_graphic(descriptor: &mut Descriptor, patches: &mut BTreeMap<Slot, Patch>) {
    if let (Props::Graphic(props), Some(Patch::Graphic(Change::Set(value)))) =
        (&mut descriptor.props, patches.remove(&Slot::Graphic))
    {
        *props = *value;
    }
}
struct Element {
    descriptor: Option<Descriptor>,
    initial: BTreeMap<Slot, Patch>,
    bindings: BTreeMap<Slot, Prepare>,
    handlers: BTreeMap<EventKind, Handler>,
    children: Vec<Box<dyn Widget>>,
    needs_alt: bool,
    node_ref: Option<Cage<Option<NodeId>>>,
    mark: Option<&'static str>,
}
impl Element {
    fn new(props: Props) -> Self {
        let needs_alt = matches!(props, Props::Image(_));
        Self {
            descriptor: Some(Descriptor::new(props)),
            initial: BTreeMap::new(),
            bindings: BTreeMap::new(),
            handlers: BTreeMap::new(),
            children: Vec::new(),
            needs_alt,
            node_ref: None,
            mark: None,
        }
    }
    fn set<T: Clone + PartialEq + 'static>(
        &mut self,
        slot: Slot,
        value: impl Into<Lotus<T>>,
        mut patch: impl FnMut(T) -> Patch + 'static,
    ) {
        self.initial.remove(&slot);
        self.bindings.remove(&slot);
        match value.into() {
            Lotus::Value(value) => {
                self.initial.insert(slot, patch(value));
            }
            value => {
                self.bindings.insert(
                    slot,
                    Box::new(move || {
                        // Read without subscribing the surrounding component or
                        // structural observer. The binding registers its own
                        // dependencies when it first runs after construction.
                        let initial = glory_core::untracked(|| value.try_get())?;
                        let initial_patch = patch(initial.clone());
                        let install: Bind = Box::new(move |scope, node| {
                            let session = Rc::downgrade(&scope.session()?);
                            let mut previous = Some(initial);
                            scope.binding(move || {
                                let Some(session) = session.upgrade() else {
                                    return;
                                };
                                let value = match value.try_get() {
                                    Ok(value) => value,
                                    Err(error) => {
                                        session.report(error.into());
                                        return;
                                    }
                                };
                                if previous.as_ref() == Some(&value) {
                                    return;
                                }
                                let update = patch(value.clone());
                                let result = match update {
                                    Patch::Value {
                                        value: Change::Set(value),
                                        ..
                                    } => session.input_value(node, value),
                                    Patch::SelectSelection {
                                        keys: Change::Set(keys),
                                        ..
                                    } => session.select_selection(node, keys),
                                    Patch::Source {
                                        source: Change::Set(value),
                                        ..
                                    } => session.image_source(node, value),
                                    update => session.patch(node, update),
                                };
                                match result {
                                    Ok(()) => previous = Some(value),
                                    Err(error) => session.report(error),
                                }
                            });
                            Ok(())
                        });
                        Ok((initial_patch, install))
                    }),
                );
            }
        }
    }
    fn build(&mut self, scope: &Scope) -> Result {
        if self.needs_alt {
            return Err(glory_render::Error::InvalidProps(
                "image requires alt text or explicit decorative()",
            )
            .into());
        }
        let mut descriptor = self.descriptor.take().ok_or(crate::app::Error::Gone)?;
        let mut initial = std::mem::take(&mut self.initial);
        let mut bindings = Vec::with_capacity(self.bindings.len());
        for (slot, prepare) in std::mem::take(&mut self.bindings) {
            let (patch, install) = prepare()?;
            initial.insert(slot, patch);
            bindings.push(install);
        }
        // Validate the complete initial descriptor and adapter profile before
        // allocating identity or changing the semantic model/host queue.
        seed_graphic(&mut descriptor, &mut initial);
        descriptor.patches(initial.into_values())?;
        let node = scope.create(descriptor)?;
        for bind in bindings {
            bind(scope, node)?;
        }
        for (kind, handler) in std::mem::take(&mut self.handlers) {
            scope.on(node, kind, handler)?;
        }
        for child in self.children.drain(..) {
            scope.fill(node, child)?;
        }
        if let Some(reference) = self.node_ref.take() {
            reference.try_revise(|value| *value = Some(node))?;
            scope.on_cleanup(move || {
                let _ = reference.try_revise(|value| {
                    if *value == Some(node) {
                        *value = None;
                    }
                });
            })?;
        }
        Ok(())
    }
    fn options<T: 'static>(&mut self, value: T) {
        self.descriptor
            .as_mut()
            .expect("unmounted builder")
            .options
            .insert(value);
    }
    fn static_plan(&mut self) -> Result<crate::Plan> {
        if self.needs_alt
            || !self.bindings.is_empty()
            || !self.handlers.is_empty()
            || self.node_ref.is_some()
        {
            return Err(glory_render::Error::InvalidProps(
                "plan requires static declarations; bind events/values on the instance",
            )
            .into());
        }
        let mut descriptor = self.descriptor.take().ok_or(crate::Error::Gone)?;
        let mut initial = std::mem::take(&mut self.initial);
        seed_graphic(&mut descriptor, &mut initial);
        descriptor.patches(initial.into_values())?;
        let mut nodes = vec![glory_render::PlanNode {
            parent: None,
            descriptor,
        }];
        let mut marks = self
            .mark
            .take()
            .map(|name| vec![(name, 0)])
            .unwrap_or_default();
        for mut child in self.children.drain(..) {
            child.static_plan()?.append(0, &mut nodes, &mut marks);
        }
        crate::Plan::from_nodes(nodes, marks)
    }
}

pub struct Children(Vec<Box<dyn Widget>>);
impl IntoChildren for Children {
    fn into_children(self) -> Children {
        self
    }
}
pub trait IntoChildren {
    fn into_children(self) -> Children;
}
impl<W: Widget> IntoChildren for W {
    fn into_children(self) -> Children {
        Children(vec![Box::new(self)])
    }
}
impl<W: Widget> IntoChildren for Vec<W> {
    fn into_children(self) -> Children {
        Children(
            self.into_iter()
                .map(|value| Box::new(value) as Box<dyn Widget>)
                .collect(),
        )
    }
}
impl IntoChildren for () {
    fn into_children(self) -> Children {
        Children(Vec::new())
    }
}
macro_rules! children_tuple {
    ($($name:ident),+) => {
        impl<$($name: Widget),+> IntoChildren for ($($name,)+) {
            #[allow(non_snake_case)]
            fn into_children(self) -> Children { let ($($name,)+) = self; Children(vec![$(Box::new($name)),+]) }
        }
    };
}
children_tuple!(A, B);
children_tuple!(A, B, C);
children_tuple!(A, B, C, D);
children_tuple!(A, B, C, D, E);
children_tuple!(A, B, C, D, E, F);
children_tuple!(A, B, C, D, E, F, G);
children_tuple!(A, B, C, D, E, F, G, H);
children_tuple!(A, B, C, D, E, F, G, H, I);
children_tuple!(A, B, C, D, E, F, G, H, I, J);
children_tuple!(A, B, C, D, E, F, G, H, I, J, K);
children_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

macro_rules! element {
    ($name:ident, $factory:ident, $props:expr) => {
        pub struct $name {
            inner: Element,
        }
        pub fn $factory() -> $name {
            $name {
                inner: Element::new($props),
            }
        }
        impl Widget for $name {
            fn static_plan(&mut self) -> Result<crate::Plan> {
                self.inner.static_plan()
            }
            fn build(&mut self, scope: &Scope) -> Result {
                self.inner.build(scope)
            }
        }
        impl $name {
            /// Name a part of an explicit static plan; not a DOM id or data key.
            pub fn mark(mut self, name: &'static str) -> Self {
                self.inner.mark = Some(name);
                self
            }
            pub fn backend_options<T: 'static>(mut self, value: T) -> Self {
                self.inner.options(value);
                self
            }
        }
    };
}
element!(View, view, Props::View);
element!(Group, group, Props::Group);
element!(Text, text, Props::Text(TextProps::default()));
element!(Button, button, Props::Button(ButtonProps::default()));
element!(Input, input, Props::Input(InputProps::default()));
element!(
    Select,
    select,
    Props::Select(glory_render::SelectProps::default())
);
element!(Check, check, Props::Check(CheckProps::default()));
element!(Image, image, Props::Image(ImageProps::default()));
element!(
    Graphic,
    graphic,
    Props::Graphic(glory_render::GraphicProps::default())
);
element!(Link, link, Props::Link(LinkProps::default()));

macro_rules! common {
    ($($name:ident),+) => {$ (
        impl $name {
            /// Set the complete whitespace-separated class list (last call wins).
            pub fn class(mut self, value: impl Into<Lotus<String>>) -> Self {
                self.inner.set(Slot::Classes, value, |value: String| Patch::Classes(Change::Set(value.split_whitespace().map(str::to_owned).collect()))); self
            }
            pub fn style(mut self, value: impl Into<Lotus<Style>>) -> Self {
                self.inner.set(Slot::Style, value, |value| Patch::Style(Change::Set(Box::new(value)))); self
            }
            /// Accessible name. Plain inline/paragraph text uses content;
            /// decorative images have no accessible name. A named generic View
            /// becomes an accessible group without changing its layout box.
            pub fn name(mut self, value: impl Into<Lotus<String>>) -> Self {
                self.inner.set(Slot::Name, value, |value| Patch::Name(Change::Set(Some(value)))); self
            }
            pub fn focus(mut self, mode: impl Into<Lotus<FocusMode>>) -> Self { self.inner.set(Slot::Focus, mode, |value| Patch::Focus(Change::Set(value))); self }
            /// Finite composite-widget state and document-scoped relations.
            pub fn semantics(mut self, value: impl Into<Lotus<Option<Box<glory_render::Semantics>>>>) -> Self {
                self.inner.set(Slot::Semantics, value, |value| Patch::Semantics(Change::Set(value))); self
            }
            pub fn on(mut self, kind: EventKind, handler: impl FnMut(Event) + 'static) -> Self { self.inner.handlers.insert(kind, Box::new(handler)); self }
            /// Publish the mounted semantic identity and clear it on retirement.
            pub fn node_ref(mut self, reference: Cage<Option<NodeId>>) -> Self { self.inner.node_ref = Some(reference); self }
        }
    )+};
}
common!(
    View, Text, Button, Input, Select, Check, Image, Graphic, Link
);
impl Graphic {
    pub fn drawing(mut self, value: impl Into<Lotus<glory_render::GraphicProps>>) -> Self {
        self.inner.set(Slot::Graphic, value, |value| {
            Patch::Graphic(Change::Set(Box::new(value)))
        });
        self
    }
}
impl View {
    /// Intrinsic document structure, installed before reactive metadata. Its
    /// kind cannot change on a live node; remount for a different document part.
    pub fn document_part(mut self, part: glory_render::DocumentPart) -> Self {
        self.inner
            .descriptor
            .as_mut()
            .expect("unmounted builder")
            .semantics
            .get_or_insert_with(Default::default)
            .document_part = Some(part);
        self
    }
    /// Structural form kind is installed before reactive metadata bindings.
    pub fn field_part(mut self, part: glory_render::FieldPart) -> Self {
        self.inner
            .descriptor
            .as_mut()
            .expect("unmounted builder")
            .semantics
            .get_or_insert_with(Default::default)
            .field_part = Some(part);
        self
    }
    /// Structural role; controls, text and images retain their own semantics.
    pub fn role(mut self, role: Role) -> Self {
        self.inner.set(Slot::Role, Some(role), |value| {
            Patch::Role(Change::Set(value))
        });
        self
    }
    pub fn item(mut self, value: impl Into<Lotus<Option<ItemPosition>>>) -> Self {
        self.inner
            .set(Slot::Item, value, |value| Patch::Item(Change::Set(value)));
        self
    }
}
macro_rules! fill {
    ($($name:ident),+) => {$ (
        impl $name { pub fn fill(mut self, children: impl IntoChildren) -> Self { self.inner.children.extend(children.into_children().0); self } }
    )+};
}
fill!(View, Group, Button, Link);
macro_rules! label {
    ($($name:ident),+) => {$ (
        impl $name { pub fn label(mut self, value: impl Into<Lotus<String>>) -> Self { self.inner.set(Slot::Label, value, |value| Patch::Label(Change::Set(value))); self } }
    )+};
}
label!(Button, Check, Link);
macro_rules! enabled {
    ($($name:ident),+) => {$ (
        impl $name { pub fn enabled(mut self, value: impl Into<Lotus<bool>>) -> Self { self.inner.set(Slot::Enabled, value, |value| Patch::Enabled(Change::Set(value))); self } }
    )+};
}
enabled!(Button, Input, Check);
impl Text {
    pub fn content(mut self, value: impl Into<Lotus<String>>) -> Self {
        self.inner
            .set(Slot::Text, value, |value| Patch::Text(Change::Set(value)));
        self
    }
    pub fn kind(mut self, value: TextKind) -> Self {
        self.inner.set(Slot::Kind, value, |value| {
            Patch::TextKind(Change::Set(value))
        });
        self
    }
    pub fn selectable(mut self, value: impl Into<Lotus<bool>>) -> Self {
        self.inner.set(Slot::Selectable, value, |value| {
            Patch::Selectable(Change::Set(value))
        });
        self
    }
}
impl Input {
    pub fn value(mut self, value: impl Into<Lotus<String>>) -> Self {
        // Static configuration starts at one; bound writes allocate from the
        // live node so explicit Scope patches cannot make this producer regress.
        self.inner.set(Slot::Value, value, |value| Patch::Value {
            value: Change::Set(value),
            revision: 1,
        });
        self
    }
    pub fn purpose(mut self, value: impl Into<Lotus<Purpose>>) -> Self {
        self.inner.set(Slot::Purpose, value, |value| {
            Patch::Purpose(Change::Set(value))
        });
        self
    }
    pub fn multiline(mut self, value: bool) -> Self {
        self.inner.set(Slot::Multiline, value, |value| {
            Patch::Multiline(Change::Set(value))
        });
        self
    }
}
impl Check {
    pub fn mode(mut self, value: impl Into<Lotus<glory_render::CheckMode>>) -> Self {
        self.inner.set(Slot::CheckMode, value, |value| {
            Patch::CheckMode(Change::Set(value))
        });
        self
    }

    pub fn checked(mut self, value: impl Into<Lotus<bool>>) -> Self {
        self.inner.set(Slot::Checked, value, |value| {
            Patch::Checked(Change::Set(value))
        });
        self
    }
}
impl Image {
    pub fn source(mut self, value: impl Into<Lotus<String>>) -> Self {
        self.inner.set(Slot::Source, value, |value| Patch::Source {
            source: Change::Set(value),
            revision: 1,
        });
        self
    }
    pub fn alt(mut self, value: impl Into<Lotus<String>>) -> Self {
        self.inner.needs_alt = false;
        self.inner.set(Slot::Alternative, value, |value| {
            Patch::Alternative(Change::Set(Some(value)))
        });
        self
    }
    pub fn decorative(mut self) -> Self {
        self.inner.needs_alt = false;
        self.inner.set(Slot::Alternative, None::<String>, |value| {
            Patch::Alternative(Change::Set(value))
        });
        self
    }
}
impl Link {
    pub fn target(mut self, value: impl Into<Lotus<String>>) -> Self {
        self.inner.set(Slot::Target, value, |value| {
            Patch::Target(Change::Set(value))
        });
        self
    }
}

pub fn row() -> View {
    view().style(
        Style::new()
            .display(Display::Flex)
            .flex_direction(FlexDirection::Row),
    )
}
pub fn col() -> View {
    view().style(
        Style::new()
            .display(Display::Flex)
            .flex_direction(FlexDirection::Column),
    )
}

pub struct Component<F>(F);
pub fn component<F: FnMut(&Scope) -> Result + 'static>(build: F) -> Component<F> {
    Component(build)
}
impl<F: FnMut(&Scope) -> Result + 'static> Widget for Component<F> {
    fn build(&mut self, scope: &Scope) -> Result {
        (self.0)(scope)
    }
}

pub mod events {
    pub use glory_render::Event;
    use glory_render::EventKind;
    #[allow(non_upper_case_globals)]
    pub const activate: EventKind = EventKind::Activate;
    #[allow(non_upper_case_globals)]
    pub const value_changed: EventKind = EventKind::ValueChanged;
    #[allow(non_upper_case_globals)]
    pub const checked_changed: EventKind = EventKind::CheckedChanged;
    #[allow(non_upper_case_globals)]
    pub const select_changed: EventKind = EventKind::SelectChanged;
    #[allow(non_upper_case_globals)]
    pub const choice_command: EventKind = EventKind::Choice;
    #[allow(non_upper_case_globals)]
    pub const tags_command: EventKind = EventKind::Tags;
    #[allow(non_upper_case_globals)]
    pub const commit: EventKind = EventKind::Commit;
    #[allow(non_upper_case_globals)]
    pub const focus_changed: EventKind = EventKind::FocusChanged;
    #[allow(non_upper_case_globals)]
    pub const focus_within: EventKind = EventKind::FocusWithin;
    #[allow(non_upper_case_globals)]
    pub const navigate: EventKind = EventKind::Navigate;
    #[allow(non_upper_case_globals)]
    pub const typeahead: EventKind = EventKind::Typeahead;
    #[allow(non_upper_case_globals)]
    pub const menu_command: EventKind = EventKind::Menu;
    #[allow(non_upper_case_globals)]
    pub const pointer_boundary: EventKind = EventKind::Pointer;
    #[allow(non_upper_case_globals)]
    pub const pointer_within: EventKind = EventKind::PointerWithin;
    #[allow(non_upper_case_globals)]
    pub const keyboard: EventKind = EventKind::Keyboard;
    #[allow(non_upper_case_globals)]
    pub const drag: EventKind = EventKind::Drag;
    #[allow(non_upper_case_globals)]
    pub const composition: EventKind = EventKind::Composition;
    #[allow(non_upper_case_globals)]
    pub const scroll: EventKind = EventKind::Scroll;
    #[allow(non_upper_case_globals)]
    pub const resize: EventKind = EventKind::Resize;
}

impl Select {
    /// Static bounded intrinsic options; selection has an independent revision.
    pub fn configuration(mut self, value: glory_render::SelectProps) -> Self {
        self.inner
            .descriptor
            .as_mut()
            .expect("unmounted builder")
            .props = Props::Select(value);
        self
    }
    pub fn selected(mut self, value: impl Into<Lotus<Vec<glory_render::SemanticId>>>) -> Self {
        self.inner.set(Slot::SelectSelection, value, |keys| {
            Patch::SelectSelection {
                keys: Change::Set(keys),
                revision: 1,
            }
        });
        self
    }
    pub fn enabled(mut self, value: impl Into<Lotus<bool>>) -> Self {
        self.inner
            .set(Slot::Enabled, value, |v| Patch::Enabled(Change::Set(v)));
        self
    }
}
