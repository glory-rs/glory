//! Shared Widget mounting on HTML-capable renderers, without DOM dependencies.
use super::*;
use glory_html::display::{Frame, Placement};
use glory_render::{Descriptor, Event, EventKind, NodeId, Props};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
pub struct Mount<'a> {
    pub(crate) frame: Rc<Frame>,
    pub(crate) root: NodeId,
    pub(crate) scope: &'a glory_ui::Scope,
    pub(crate) handlers: Rc<RefCell<BTreeMap<u16, Box<dyn FnMut()>>>>,
}
impl Mount<'_> {
    pub(crate) fn outlet(&self, outlet: Outlet) -> glory_ui::Result<glory_ui::Scope> {
        let (slot, kind) = self.frame.slot(outlet.0)?;
        if !matches!(kind, Kind::Children | Kind::Range) {
            return Err(glory_render::Error::InvalidProps("display outlet kind").into());
        }
        let parent = self.scope.child_in(self.root)?;
        let mut descriptor = Descriptor::new(Props::Group);
        descriptor.options.insert(Placement::Outlet {
            frame: self.frame.clone(),
            slot,
        });
        let node = parent.create(descriptor)?;
        parent.place(node, Some(self.root), None)?;
        let content = parent.child_in(node)?;
        self.scope.retain(parent)?;
        self.scope.retain(content.clone())?;
        Ok(content)
    }
}
pub(crate) fn build<A: Actions>(
    template: &'static Template,
    actions: A,
    scope: &glory_ui::Scope,
) -> glory_ui::Result {
    let frame = Frame::new(template)?;
    let mut descriptor = Descriptor::new(Props::Group);
    descriptor.options.insert(Placement::Root(frame.clone()));
    descriptor.options.insert(glory_render::DisplayEvents(
        frame
            .compiled()
            .slots
            .iter()
            .enumerate()
            .filter_map(|(slot, value)| (value.kind == Kind::Click).then_some(slot as u16))
            .collect(),
    ));
    let root = scope.create(descriptor)?;
    scope.place(root, scope.construction_parent(), None)?;
    let handlers = Rc::new(RefCell::new(BTreeMap::new()));
    actions.apply(&Mount {
        frame,
        root,
        scope,
        handlers: handlers.clone(),
    })?;
    if !handlers.borrow().is_empty() {
        scope.on(root, EventKind::DisplayClick, move |event| {
            if let Event::DisplayClick(slot) = event
                && let Some(handler) = handlers.borrow_mut().get_mut(&slot)
            {
                handler();
            }
        })?;
    }
    Ok(())
}
pub(crate) fn failure(error: Error) -> glory_ui::Error {
    glory_render::Error::Backend(error.to_string()).into()
}
