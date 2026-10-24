//! Intrinsic combobox/option refinements and commands, independent of hosts.
use crate::{KeyboardInput, SemanticId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComboboxKind {
    /// A Button retaining activation, with a bounded listbox as its popup.
    Select,
    /// A single-line Input retaining editing, composition and selection.
    Editable,
    /// A single-line Input whose freely edited text is its successful value.
    Autocomplete,
    /// A single-line search Input controlling a multiple-selection listbox.
    Multiple,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionSemantics {
    pub owner: SemanticId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceCommand {
    OpenFirst,
    OpenLast,
    Previous,
    Next,
    First,
    Last,
    Commit,
    Close,
    RemoveLast,
}

impl KeyboardInput<'_> {
    pub fn choice_remove_last(self, kind: ComboboxKind, empty: bool) -> Option<ChoiceCommand> {
        (kind == ComboboxKind::Multiple
            && empty
            && self.key == "Backspace"
            && !self.composing
            && !self.control
            && !self.meta
            && !self.alt
            && !self.shift)
            .then_some(ChoiceCommand::RemoveLast)
    }
    pub fn choice(self, kind: ComboboxKind, expanded: bool) -> Option<ChoiceCommand> {
        use ChoiceCommand::*;
        if self.composing || self.control || self.meta || self.shift {
            return None;
        }
        if self.alt {
            return (!expanded && self.key == "ArrowDown").then_some(OpenFirst);
        }
        match self.key {
            // Interpret Select against the controller's current source. Native
            // keys can queue before a frame commits expanded metadata.
            "ArrowDown" if kind == ComboboxKind::Select => Some(Next),
            "ArrowUp" if kind == ComboboxKind::Select => Some(Previous),
            "Enter" | " " if kind == ComboboxKind::Select => Some(Commit),
            "Escape" if kind == ComboboxKind::Select => Some(Close),
            "ArrowDown" => Some(if expanded { Next } else { OpenFirst }),
            "ArrowUp" => Some(if expanded { Previous } else { OpenLast }),
            "Home" if kind == ComboboxKind::Select => Some(First),
            "End" if kind == ComboboxKind::Select => Some(Last),
            "Enter" => {
                if expanded {
                    Some(Commit)
                } else {
                    (kind == ComboboxKind::Select).then_some(OpenFirst)
                }
            }
            "Escape" if expanded => Some(Close),
            _ => None,
        }
    }
}

impl crate::Semantics {
    pub(crate) fn validate_choice(
        &self,
        node: crate::DescriptorView<'_>,
    ) -> Result<(), crate::Error> {
        use crate::{Error, FocusMode, Orientation, PopupKind, Props, Role};
        if let Some(kind) = self.combobox {
            let valid = matches!((kind, node.props), (ComboboxKind::Select, Props::Button(_)))
                || matches!(node.props, Props::Input(input) if kind != ComboboxKind::Select && !input.multiline);
            if !valid
                || self.id.is_none()
                || self.controls.is_none()
                || self.popup != Some(PopupKind::Listbox)
                || self.expanded.is_none()
                || self.tab
                || self.menu_item.is_some()
                || self.option.is_some()
                || self.form_action.is_some()
                || self.pressed.is_some()
            {
                return Err(Error::InvalidProps(
                    "combobox requires a matching intrinsic trigger, listbox popup and controls/expanded metadata",
                ));
            }
        }
        if self.active_descendant.is_some()
            && (self.combobox.is_none()
                || self.expanded != Some(true)
                || self.active_descendant == self.id)
        {
            return Err(Error::InvalidProps(
                "active descendant requires an expanded combobox and distinct option",
            ));
        }
        if let Some(option) = &self.option
            && (!matches!(node.props, Props::Button(_))
                || node.focus != FocusMode::Programmatic
                || self.id.is_none()
                || self.id.as_ref() == Some(&option.owner)
                || self.selected.is_none()
                || self.tab
                || self.menu_item.is_some()
                || self.combobox.is_some()
                || self.popup.is_some()
                || self.controls.is_some()
                || self.expanded.is_some()
                || self.pressed.is_some()
                || self.form_action.is_some()
                || self.form_control.is_some())
        {
            return Err(Error::InvalidProps(
                "listbox options require an unfocusable intrinsic button, selected state and distinct owner",
            ));
        }
        if self.listbox_multiple && node.role != Some(Role::Listbox) {
            return Err(Error::InvalidProps("multiple selection requires a listbox"));
        }
        if node.role == Some(Role::Listbox)
            && (self.id.is_none()
                || node.focus != FocusMode::Auto
                || self.navigation != Some(Orientation::Vertical))
        {
            return Err(Error::InvalidProps(
                "listbox requires a vertical unfocused collection boundary",
            ));
        }
        Ok(())
    }
}

impl crate::memory::Memory {
    pub(crate) fn validate_choice_patch(&self, id: crate::NodeId) -> Result<(), crate::Error> {
        let node = self.node(id)?;
        if !node.placed && node.parent.is_none() {
            return Ok(());
        }
        if self.nearest_listbox(node.parent)?.is_none() {
            let mut root = id;
            while let Some(parent) = self.node(root)?.parent {
                root = parent;
            }
            let root = self.node(root)?;
            if !root.placed && root.descriptor.pending_external_options() {
                return Ok(());
            }
        }
        self.validate_choice_child(crate::memory::Subtree::Existing(id), node.parent)
    }
    fn nearest_listbox(
        &self,
        mut node: Option<crate::NodeId>,
    ) -> Result<Option<crate::NodeId>, crate::Error> {
        while let Some(id) = node {
            let current = self.node(id)?;
            if current.descriptor.view().role == Some(crate::Role::Listbox) {
                return Ok(Some(id));
            }
            node = current.parent;
        }
        Ok(None)
    }

    pub(crate) fn validate_choice_child(
        &self,
        child: crate::memory::Subtree<'_>,
        parent: Option<crate::NodeId>,
    ) -> Result<(), crate::Error> {
        use crate::memory::Subtree;
        let owner = self.nearest_listbox(parent)?;
        let name = owner
            .and_then(|id| self.node(id).ok())
            .and_then(|node| node.descriptor.view().semantic_id());
        match child {
            Subtree::New(descriptor) => check_child(descriptor, name)?,
            Subtree::Plan(plan) => validate_plan_owners(plan, name, false)?,
            Subtree::Existing(id) => {
                let root = self.node(id)?;
                check_child(root.descriptor.view(), name)?;
                let old = self.nearest_listbox(root.parent)?;
                let pending_plan = !root.placed && root.descriptor.pending_external_options();
                if old != owner || pending_plan {
                    self.validate_choice_subtree(id, parent)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_choice_subtree(
        &self,
        root: crate::NodeId,
        parent: Option<crate::NodeId>,
    ) -> Result<(), crate::Error> {
        let owner = self.nearest_listbox(parent)?;
        let name = owner
            .and_then(|id| self.node(id).ok())
            .and_then(|node| node.descriptor.view().semantic_id());
        self.validate_choice_range(vec![root], name)
    }

    pub(crate) fn validate_choice_contents(&self, root: crate::NodeId) -> Result<(), crate::Error> {
        let owner = self.nearest_listbox(Some(root))?;
        let name = owner
            .and_then(|id| self.node(id).ok())
            .and_then(|node| node.descriptor.view().semantic_id());
        let children = &self.node(root)?.children;
        if children.len() > 4096 {
            return Err(crate::Error::InvalidPlacement(
                "restricted listbox subtree exceeds 4096 live nodes",
            ));
        }
        self.validate_choice_range(children.iter().copied().collect(), name)
    }

    fn validate_choice_range(
        &self,
        mut pending: Vec<crate::NodeId>,
        name: Option<&SemanticId>,
    ) -> Result<(), crate::Error> {
        let mut inspected = 0usize;
        while let Some(id) = pending.pop() {
            inspected += 1;
            if inspected > 4096 {
                return Err(crate::Error::InvalidPlacement(
                    "restricted listbox subtree exceeds 4096 live nodes",
                ));
            }
            let node = self.node(id)?;
            let descriptor = node.descriptor.view();
            check_child(descriptor, name)?;
            // Internal listboxes own their descendants; moving this complete
            // boundary cannot change those options' closest owner.
            if descriptor.role == Some(crate::Role::Listbox) {
                continue;
            }
            if pending.len() + inspected + node.children.len() > 4096 {
                return Err(crate::Error::InvalidPlacement(
                    "restricted listbox subtree exceeds 4096 live nodes",
                ));
            }
            pending.extend(node.children.iter().copied());
        }
        Ok(())
    }
    /// Named relations may resolve after construction. Missing, inactive or
    /// retired members are never reported as an active option; identity comes
    /// from the current live name registry, not an old slot or visited history.
    pub fn resolve_active_descendant(
        &self,
        trigger: crate::NodeId,
    ) -> Result<Option<crate::NodeId>, crate::Error> {
        let descriptor = self.node(trigger)?.descriptor.view();
        let Some(semantics) = descriptor
            .semantics
            .as_deref()
            .filter(|s| s.combobox.is_some() && s.expanded == Some(true))
        else {
            return Ok(None);
        };
        let Some(owner) = semantics
            .controls
            .as_ref()
            .and_then(|name| self.named(name))
        else {
            return Ok(None);
        };
        let Some(option) = semantics
            .active_descendant
            .as_ref()
            .and_then(|name| self.named(name))
        else {
            return Ok(None);
        };
        if !self.active(trigger)
            || !self.active(owner)
            || !self.active(option)
            || !self.enabled(option)
        {
            return Ok(None);
        }
        let owner_descriptor = self.node(owner)?.descriptor.view();
        let option_descriptor = self.node(option)?.descriptor.view();
        if !option_descriptor.activation_enabled() {
            return Ok(None);
        }
        if owner_descriptor.role != Some(crate::Role::Listbox)
            || owner_descriptor
                .semantics
                .as_ref()
                .is_some_and(|owner| owner.listbox_multiple)
                != (semantics.combobox == Some(ComboboxKind::Multiple))
            || !option_descriptor
                .semantics
                .as_ref()
                .and_then(|s| s.option.as_ref())
                .is_some_and(|s| Some(&s.owner) == semantics.controls.as_ref())
        {
            return Ok(None);
        }
        let mut cursor = self.node(option)?.parent;
        while let Some(id) = cursor {
            if id == owner {
                return Ok(Some(option));
            }
            if self.node(id)?.descriptor.view().role == Some(crate::Role::Listbox) {
                return Ok(None);
            }
            cursor = self.node(id)?.parent;
        }
        Ok(None)
    }

    pub(crate) fn validate_choice_reference(
        &self,
        descriptor: crate::DescriptorView<'_>,
    ) -> Result<(), crate::Error> {
        let Some(semantics) = descriptor.semantics.as_deref() else {
            return Ok(());
        };
        if semantics.combobox.is_some()
            && let Some(owner) = semantics
                .controls
                .as_ref()
                .and_then(|name| self.named(name))
        {
            let owner = self.node(owner)?.descriptor.view();
            if owner.role != Some(crate::Role::Listbox)
                || owner
                    .semantics
                    .as_ref()
                    .is_some_and(|owner| owner.listbox_multiple)
                    != (semantics.combobox == Some(ComboboxKind::Multiple))
            {
                return Err(crate::Error::InvalidProps(
                    "combobox controls must resolve to a matching single or multiple listbox",
                ));
            }
        }
        let Some(active) = semantics
            .active_descendant
            .as_ref()
            .and_then(|name| self.named(name))
        else {
            return Ok(());
        };
        let target = self.node(active)?.descriptor.view();
        if !target
            .semantics
            .as_ref()
            .and_then(|s| s.option.as_ref())
            .is_some_and(|option| Some(&option.owner) == semantics.controls.as_ref())
        {
            return Err(crate::Error::InvalidProps(
                "active descendant must identify an option belonging to the controlled listbox",
            ));
        }
        Ok(())
    }
}

fn check_child(
    descriptor: crate::DescriptorView<'_>,
    owner: Option<&SemanticId>,
) -> Result<(), crate::Error> {
    let option = descriptor
        .semantics
        .as_ref()
        .and_then(|s| s.option.as_ref());
    if let Some(option) = option {
        if owner != Some(&option.owner) {
            return Err(crate::Error::InvalidPlacement(
                "option requires its nearest owning listbox",
            ));
        }
    } else if owner.is_some()
        && (descriptor.props.interactive() || descriptor.role == Some(crate::Role::Listbox))
    {
        return Err(crate::Error::InvalidPlacement(
            "listboxes contain options and passive groups, not unrelated controls",
        ));
    }
    Ok(())
}

/// A finite declaration may await an external owner until attachment, just as
/// named relations may await later construction. Internal ownership is checked
/// immediately; the full live parent context is checked before Place.
pub(crate) fn validate_plan_owners(
    plan: &crate::Plan,
    owner: Option<&SemanticId>,
    pending: bool,
) -> Result<(), crate::Error> {
    let mut contexts = Vec::with_capacity(plan.nodes().len());
    for node in plan.nodes() {
        let context = node.parent.map_or(owner, |parent| contexts[parent]);
        let descriptor = node.descriptor.view();
        if !(pending && context.is_none()) {
            check_child(descriptor, context)?;
        }
        contexts.push(if descriptor.role == Some(crate::Role::Listbox) {
            descriptor.semantic_id()
        } else {
            context
        });
    }
    Ok(())
}

pub(crate) fn plan_external_options(plan: &crate::Plan) -> bool {
    let mut contexts = Vec::with_capacity(plan.nodes().len());
    for node in plan.nodes() {
        let context = node.parent.is_some_and(|parent| contexts[parent]);
        if !context
            && node
                .descriptor
                .semantics
                .as_ref()
                .is_some_and(|s| s.option.is_some())
        {
            return true;
        }
        contexts.push(context || node.descriptor.role == Some(crate::Role::Listbox));
    }
    false
}
