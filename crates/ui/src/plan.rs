//! Explicit reusable construction from the ordinary handwritten Rust builders.
use crate::{Result, Scope, WeakScope, Widget};
use glory_core::Vary;
use glory_render::{Change, Event, EventKind, NodeId, Patch};
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Plan {
    pub(crate) semantic: glory_render::Plan,
    marks: Rc<[(&'static str, usize)]>,
}
#[derive(Clone, Debug)]
pub struct PlanPart {
    plan: glory_render::Plan,
    index: usize,
}

/// Compile declarations once, outside a row/data loop. Control flow, bindings,
/// event captures and node_ref handles belong to instances, never this plan.
pub fn plan(mut widget: impl Widget) -> Result<Plan> {
    widget.static_plan()
}
impl Plan {
    pub(crate) fn from_nodes(
        nodes: Vec<glory_render::PlanNode>,
        marks: Vec<(&'static str, usize)>,
    ) -> Result<Self> {
        let mut names = std::collections::HashSet::new();
        if marks.iter().any(|(name, _)| !names.insert(*name)) {
            return Err(glory_render::Error::InvalidProps("duplicate plan mark").into());
        }
        Ok(Self {
            semantic: glory_render::Plan::new(nodes)?,
            marks: marks.into(),
        })
    }
    pub fn part(&self, name: &str) -> Result<PlanPart> {
        let index = self
            .marks
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, index)| *index)
            .ok_or(glory_render::Error::InvalidProps("unknown plan mark"))?;
        Ok(PlanPart {
            plan: self.semantic.clone(),
            index,
        })
    }
    pub(crate) fn append(
        &self,
        parent: usize,
        nodes: &mut Vec<glory_render::PlanNode>,
        marks: &mut Vec<(&'static str, usize)>,
    ) {
        let offset = nodes.len();
        nodes.extend(
            self.semantic
                .nodes()
                .iter()
                .map(|node| glory_render::PlanNode {
                    parent: Some(node.parent.map_or(parent, |p| p + offset)),
                    descriptor: node.descriptor.clone(),
                }),
        );
        marks.extend(
            self.marks
                .iter()
                .map(|(name, index)| (*name, index + offset)),
        );
    }
}

/// Non-owning access to the live nodes of one declared construction. Cloning
/// a plan never clones an instance's nodes, event captures or reactive state.
pub struct PlanInstance {
    pub(crate) scope: WeakScope,
    pub(crate) plan: glory_render::Plan,
    pub(crate) ids: Vec<NodeId>,
}
impl PlanInstance {
    fn resolve(&self, part: &PlanPart) -> Result<(Scope, NodeId)> {
        if !self.plan.same(&part.plan) {
            return Err(glory_render::Error::InvalidProps("part belongs to another plan").into());
        }
        let scope = self.scope.upgrade().ok_or(crate::Error::Gone)?;
        Ok((scope, self.ids[part.index]))
    }
    pub fn node(&self, part: &PlanPart) -> Result<NodeId> {
        self.resolve(part).map(|(_, id)| id)
    }
    pub fn patch(&self, part: &PlanPart, patch: Patch) -> Result {
        let (scope, id) = self.resolve(part)?;
        scope.patch(id, patch)
    }
    pub fn bind<T: Clone + PartialEq + 'static>(
        &self,
        part: &PlanPart,
        value: impl Into<Vary<T>>,
        patch: impl FnMut(T) -> Patch + 'static,
    ) -> Result {
        let (scope, id) = self.resolve(part)?;
        bind(&scope, id, value.into(), patch)
    }
    /// One pure property observer, without an intermediate Bond. Use Bond for
    /// shared/cached computations; this closure runs under the instance Owner.
    pub fn bind_fn<T: Clone + PartialEq + 'static>(
        &self,
        part: &PlanPart,
        read: impl FnMut() -> T + 'static,
        patch: impl FnMut(T) -> Patch + 'static,
    ) -> Result {
        let (scope, id) = self.resolve(part)?;
        bind_fn(&scope, id, read, patch, None)
    }
    pub fn text_fn(&self, part: &PlanPart, read: impl FnMut() -> String + 'static) -> Result {
        self.bind_fn(part, read, |v| Patch::Text(Change::Set(v)))
    }
    pub fn label_fn(&self, part: &PlanPart, read: impl FnMut() -> String + 'static) -> Result {
        self.bind_fn(part, read, |v| Patch::Label(Change::Set(v)))
    }
    pub fn class_fn(&self, part: &PlanPart, read: impl FnMut() -> String + 'static) -> Result {
        self.bind_fn(part, read, |v| {
            Patch::Classes(Change::Set(
                v.split_whitespace().map(str::to_owned).collect(),
            ))
        })
    }
    pub fn text(&self, part: &PlanPart, value: impl Into<Vary<String>>) -> Result {
        self.bind(part, value, |v| Patch::Text(Change::Set(v)))
    }
    pub fn label(&self, part: &PlanPart, value: impl Into<Vary<String>>) -> Result {
        self.bind(part, value, |v| Patch::Label(Change::Set(v)))
    }
    pub fn class(&self, part: &PlanPart, value: impl Into<Vary<String>>) -> Result {
        self.bind(part, value, |v| {
            Patch::Classes(Change::Set(
                v.split_whitespace().map(str::to_owned).collect(),
            ))
        })
    }
    pub fn on(
        &self,
        part: &PlanPart,
        kind: EventKind,
        handler: impl FnMut(Event) + 'static,
    ) -> Result {
        let (scope, id) = self.resolve(part)?;
        scope.on(id, kind, handler)
    }
}

pub(crate) fn bind<T: Clone + PartialEq + 'static>(
    scope: &Scope,
    node: NodeId,
    value: Vary<T>,
    mut patch: impl FnMut(T) -> Patch + 'static,
) -> Result {
    if let Vary::Value(value) = value {
        return match patch(value) {
            Patch::RichDocument {
                document: Change::Set(value),
                ..
            } => scope.session()?.rich_document(node, value),
            Patch::Value {
                value: Change::Set(value),
                ..
            } => scope.session()?.input_value(node, value),
            Patch::SelectSelection {
                keys: Change::Set(keys),
                ..
            } => scope.session()?.select_selection(node, keys),
            Patch::Source {
                source: Change::Set(value),
                ..
            } => scope.session()?.image_source(node, value),
            update => scope.patch(node, update),
        };
    }
    let session = Rc::downgrade(&scope.session()?);
    let mut previous = None;
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
            Patch::RichDocument {
                document: Change::Set(value),
                ..
            } => session.rich_document(node, value),
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
}

pub(crate) fn bind_fn<T: Clone + PartialEq + 'static>(
    scope: &Scope,
    node: NodeId,
    mut read: impl FnMut() -> T + 'static,
    mut patch: impl FnMut(T) -> Patch + 'static,
    mut previous: Option<T>,
) -> Result {
    let session = Rc::downgrade(&scope.session()?);
    scope.binding(move || {
        let Some(session) = session.upgrade() else {
            return;
        };
        let value = read();
        if previous.as_ref() == Some(&value) {
            return;
        }
        let result = match patch(value.clone()) {
            Patch::RichDocument {
                document: Change::Set(value),
                ..
            } => session.rich_document(node, value),
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
}
