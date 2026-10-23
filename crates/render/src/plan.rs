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
pub struct Plan(Rc<[PlanNode]>);
#[derive(Clone, Debug)]
pub struct WeakPlan(Weak<[PlanNode]>);
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
                    if !matches!(nodes[index].descriptor.props, Props::Group) {
                        break;
                    }
                    boxed_parent = nodes[index].parent;
                }
                crate::document::validate_child(
                    &nodes[index].descriptor,
                    boxed_parent.map(|parent| &nodes[parent].descriptor),
                    boxed_parent.is_none(),
                )?;
            }
            let (interactive, button, form) = match parent {
                None if index == 0 => (false, false, false),
                Some(parent) if parent < index => {
                    if nodes[index].descriptor.field_part() == Some(crate::FieldPart::Legend)
                        && nodes[parent].descriptor.field_part() != Some(crate::FieldPart::Fieldset)
                    {
                        return Err(Error::InvalidPlacement(
                            "legend requires a direct fieldset parent",
                        ));
                    }
                    if !nodes[parent].descriptor.props.accepts_children() {
                        return Err(Error::InvalidProps(
                            "leaf component cannot contain children",
                        ));
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
            validate_context(descriptor, interactive, button)?;
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
                button || matches!(descriptor.props, Props::Button(_)),
                form || descriptor.field_part() == Some(crate::FieldPart::Form),
            ));
        }
        Ok(Self(nodes.into()))
    }
    pub fn nodes(&self) -> &[PlanNode] {
        &self.0
    }
    pub fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    pub fn downgrade(&self) -> WeakPlan {
        WeakPlan(Rc::downgrade(&self.0))
    }
    pub fn identity(&self) -> usize {
        Rc::as_ptr(&self.0) as *const PlanNode as usize
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

pub(crate) fn validate_context(
    descriptor: &Descriptor,
    interactive: bool,
    button: bool,
) -> Result<(), Error> {
    if interactive && (descriptor.props.interactive() || descriptor.focus != FocusMode::Auto) {
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
        )
    {
        return Err(Error::InvalidProps(
            "button children must be inline text, images or groups",
        ));
    }
    Ok(())
}
