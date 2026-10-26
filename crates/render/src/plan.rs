//! Immutable semantic construction, shared by explicit Rust declarations.
//! There are no instructions, callbacks, reactive handles or host objects here.
use crate::{Descriptor, Error, FocusMode, Props, TextKind, TextProps};
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    rc::{Rc, Weak},
};

pub const MAX_PLAN_NODES: usize = 256;

#[derive(Clone, Debug, PartialEq)]
pub struct PlanNode {
    pub parent: Option<usize>,
    pub descriptor: Descriptor,
}

#[derive(Clone, Debug)]
pub struct Plan(Rc<Declaration>);
#[derive(Clone, Debug)]
pub struct WeakPlan(Weak<Declaration>);

/// Only facts about the immutable declaration. No identity, live parent,
/// adapter configuration or instance override may enter this shared storage.
#[derive(Debug)]
struct Declaration {
    nodes: Box<[PlanNode]>,
    facts: Box<[Facts]>,
    layers: usize,
    focus_scopes: usize,
    semantic_nodes: Box<[usize]>,
    document_roots: Box<[usize]>,
    choice_roots: Box<[usize]>,
    external_options: bool,
}
#[derive(Debug)]
pub(crate) struct Facts {
    pub layer: bool,
    pub live_names: bool,
    // A plan contains at most 256 nodes; zero means no declared focus scope.
    pub focus_nodes: u16,
}
impl Declaration {
    fn new(nodes: Vec<PlanNode>) -> Self {
        let mut facts = Vec::with_capacity(nodes.len());
        let mut semantic_nodes = Vec::new();
        let mut document_roots = Vec::new();
        let mut choice_roots = Vec::new();
        let mut document_contexts = Vec::with_capacity(nodes.len());
        let mut choice_contexts = Vec::with_capacity(nodes.len());
        let mut layers = 0;
        let mut focus_scopes = 0;
        let mut external_options = false;
        for (index, node) in nodes.iter().enumerate() {
            let d = node.descriptor.view();
            let layer = d.semantics.as_ref().is_some_and(|s| s.layer.is_some());
            let focus = crate::focus_scope::declared(d);
            let live_names = d.semantics.is_some();
            layers += usize::from(layer);
            focus_scopes += usize::from(focus);
            facts.push(Facts {
                layer,
                live_names,
                focus_nodes: u16::from(focus),
            });
            if live_names {
                semantic_nodes.push(index);
            }
            let exposed = node.parent.is_none_or(|parent| {
                document_contexts[parent] && matches!(nodes[parent].descriptor.props, Props::Group)
            });
            document_contexts.push(exposed);
            if exposed {
                document_roots.push(index);
            }
            let owned = node.parent.is_some_and(|parent| choice_contexts[parent]);
            let option = d.semantics.as_ref().is_some_and(|s| s.option.is_some());
            if !owned {
                external_options |= option;
                if option || d.props.interactive() || d.role == Some(crate::Role::Listbox) {
                    choice_roots.push(index);
                }
            }
            choice_contexts.push(owned || d.role == Some(crate::Role::Listbox));
        }
        if focus_scopes > 0 {
            let mut sizes = vec![1u16; nodes.len()];
            for (index, node) in nodes.iter().enumerate().rev() {
                if let Some(parent) = node.parent {
                    sizes[parent] += sizes[index];
                }
                if facts[index].focus_nodes != 0 {
                    facts[index].focus_nodes = sizes[index];
                }
            }
        }
        Self {
            nodes: nodes.into_boxed_slice(),
            facts: facts.into_boxed_slice(),
            layers,
            focus_scopes,
            semantic_nodes: semantic_nodes.into_boxed_slice(),
            document_roots: document_roots.into_boxed_slice(),
            choice_roots: choice_roots.into_boxed_slice(),
            external_options,
        }
    }
}
impl Plan {
    /// One root followed by its descendants in parent-before-child order.
    /// Construction validates the complete declaration once; instances still
    /// check live identity/name/context and the selected adapter's limits.
    pub fn new(mut nodes: Vec<PlanNode>) -> Result<Self, Error> {
        if nodes.is_empty() || nodes.len() > MAX_PLAN_NODES {
            return Err(Error::InvalidProps("plan requires 1..=256 nodes"));
        }
        let mut names = HashSet::new();
        let mut contexts = Vec::with_capacity(nodes.len());
        for index in 0..nodes.len() {
            let parent = nodes[index].parent;
            if parent.is_none_or(|parent| parent < index) {
                let mut boxed_parent = parent;
                while let Some(index) = boxed_parent {
                    if !matches!(nodes[index].descriptor.view().props, Props::Group) {
                        break;
                    }
                    boxed_parent = nodes[index].parent;
                }
                crate::document::validate_child(
                    nodes[index].descriptor.view(),
                    boxed_parent.map(|parent| nodes[parent].descriptor.view()),
                    boxed_parent.is_none(),
                )?;
            }
            let (interactive, button, form, tooltip) = match parent {
                None if index == 0 => (false, false, false, false),
                Some(parent) if parent < index => {
                    if nodes[parent].descriptor.view().role == Some(crate::Role::Separator) {
                        return Err(Error::InvalidProps(
                            "passive separators cannot contain children",
                        ));
                    }
                    if nodes[index].descriptor.view().field_part() == Some(crate::FieldPart::Legend)
                        && nodes[parent].descriptor.view().field_part()
                            != Some(crate::FieldPart::Fieldset)
                    {
                        return Err(Error::InvalidPlacement(
                            "legend requires a direct fieldset parent",
                        ));
                    }
                    if !nodes[parent].descriptor.view().props.accepts_children() {
                        return Err(Error::InvalidProps("leaf widget cannot contain children"));
                    }
                    contexts[parent]
                }
                _ => {
                    return Err(Error::InvalidPlacement(
                        "plan parent must precede its child",
                    ));
                }
            };
            let descriptor = &mut nodes[index].descriptor;
            descriptor.normalize_input()?;
            descriptor.validate()?;
            if form && descriptor.field_part() == Some(crate::FieldPart::Form) {
                return Err(Error::InvalidPlacement("forms cannot be nested"));
            }
            validate_context(descriptor.view(), interactive, button, tooltip)?;
            if let Some(name) = descriptor.semantic_id()
                && !names.insert(name.clone())
            {
                return Err(Error::InvalidProps("duplicate semantic id"));
            }
            contexts.push((
                interactive
                    || descriptor.props.interactive()
                    || matches!(
                        descriptor.field_part(),
                        Some(crate::FieldPart::Label | crate::FieldPart::Legend)
                    ),
                button || matches!(descriptor.props, Props::Button(_) | Props::File(_)),
                form || descriptor.field_part() == Some(crate::FieldPart::Form),
                tooltip
                    || matches!(
                        descriptor.role,
                        Some(crate::Role::Tooltip | crate::Role::ProgressBar | crate::Role::Slider)
                    ),
            ));
        }
        let plan = Self(Rc::new(Declaration::new(nodes)));
        crate::choice::validate_plan_owners(&plan, None, true)?;
        Ok(plan)
    }
    pub fn nodes(&self) -> &[PlanNode] {
        &self.0.nodes
    }
    pub(crate) fn facts(&self, index: usize) -> &Facts {
        &self.0.facts[index]
    }
    pub(crate) fn layers(&self) -> usize {
        self.0.layers
    }
    pub(crate) fn focus_scopes(&self) -> usize {
        self.0.focus_scopes
    }
    /// Declaration indices with a Semantics payload, in construction order.
    /// Adapters can inspect those immutable facets without visiting every
    /// instance. Live overrides still arrive through explicit field patches.
    pub fn semantic_nodes(&self) -> &[usize] {
        &self.0.semantic_nodes
    }
    pub(crate) fn document_roots(&self) -> &[usize] {
        &self.0.document_roots
    }
    pub(crate) fn choice_roots(&self) -> &[usize] {
        &self.0.choice_roots
    }
    pub(crate) fn external_options(&self) -> bool {
        self.0.external_options
    }
    pub fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    pub fn downgrade(&self) -> WeakPlan {
        WeakPlan(Rc::downgrade(&self.0))
    }
    pub fn identity(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }
}
impl WeakPlan {
    pub fn upgrade(&self) -> Option<Plan> {
        self.0.upgrade().map(Plan)
    }
}
impl PartialEq for Plan {
    fn eq(&self, other: &Self) -> bool {
        self.same(other)
    }
}
impl Eq for Plan {}
impl Hash for Plan {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identity().hash(state);
    }
}

pub(crate) fn validate_context<'a>(
    descriptor: impl Into<crate::DescriptorView<'a>>,
    interactive: bool,
    button: bool,
    tooltip: bool,
) -> Result<(), Error> {
    let descriptor = descriptor.into();
    if tooltip
        && (descriptor.props.interactive()
            || descriptor.focus != FocusMode::Auto
            || matches!(
                descriptor.role,
                Some(
                    crate::Role::Dialog
                        | crate::Role::AlertDialog
                        | crate::Role::Menu
                        | crate::Role::Toolbar
                        | crate::Role::TabList
                        | crate::Role::RadioGroup
                )
            )
            || descriptor.semantics.as_ref().is_some_and(|s| {
                s.layer.is_some()
                    || s.navigation.is_some()
                    || s.tab
                    || s.pressed.is_some()
                    || s.popup.is_some()
                    || s.expanded.is_some()
                    || s.menu_item.is_some()
                    || s.form_action.is_some()
                    || s.swipe_capture.is_some()
            }))
    {
        return Err(Error::InvalidProps(
            "tooltip/progress content must remain passive",
        ));
    }
    if interactive
        && (descriptor.props.interactive()
            || descriptor.focus != FocusMode::Auto
            || descriptor
                .semantics
                .as_ref()
                .is_some_and(|s| s.swipe_capture.is_some()))
    {
        return Err(Error::InvalidProps("interactive controls cannot be nested"));
    }
    if button
        && !matches!(
            descriptor.props,
            Props::Group
                | Props::Text(TextProps {
                    kind: TextKind::Inline | TextKind::Code | TextKind::Kbd | TextKind::Mark,
                    ..
                })
                | Props::Image(_)
                | Props::Graphic(_)
        )
    {
        return Err(Error::InvalidProps(
            "button children must be inline text, images or groups",
        ));
    }
    Ok(())
}
