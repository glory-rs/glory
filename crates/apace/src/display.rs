//! Native physical template nodes and sparse field targets.
use super::*;
use glory_html::{
    display::{Frame, Tree, Value},
    template::Kind as SlotKind,
};
pub(super) struct Instance {
    pub frame: Rc<Frame>,
    pub targets: Vec<EngineId>,
    pub controls: Vec<EngineId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FocusTarget {
    pub owner: NodeId,
    pub control: EngineId,
}
pub(super) fn focus_target(state: &State, owner: NodeId) -> Option<FocusTarget> {
    state
        .dom
        .control(owner)
        .ok()
        .map(|control| FocusTarget { owner, control })
}
pub(super) fn current_focus(state: &State) -> Option<FocusTarget> {
    let control = state.dom.doc.get_focussed_node_id()?;
    Some(FocusTarget {
        owner: state.dom.owner(control)?,
        control,
    })
}
pub(super) fn eligible(state: &State, target: FocusTarget) -> bool {
    state.mapper.tree().active(target.owner)
        && state.mapper.tree().enabled(target.owner)
        && state.layers.allowed(state.mapper.tree(), target.owner)
        && state.dom.owner(target.control) == Some(target.owner)
        && state.dom.focus_visible(target.control)
        && state.dom.doc.get_node(target.control).is_some_and(|node| {
            (node.is_focussable()
                || state
                    .mapper
                    .tree()
                    .node(target.owner)
                    .ok()
                    .is_some_and(|owner| {
                        owner.descriptor.view().focus == glory_render::FocusMode::Programmatic
                    })
                    && state.dom.control(target.owner).ok() == Some(target.control))
                && node.element_data().is_none_or(|element| {
                    element
                        .attr(apace_dom::LocalName::from("disabled"))
                        .is_none()
                })
        })
}
pub(super) fn focus_candidates(state: &State, root: NodeId) -> Vec<FocusTarget> {
    let mut pending = state.dom.nodes(root).unwrap_or_default();
    pending.reverse();
    let mut result = Vec::new();
    while let Some(control) = pending.pop() {
        let Some(node) = state.dom.doc.get_node(control) else {
            continue;
        };
        if let Some(owner) = state.dom.owner(control) {
            let target = FocusTarget { owner, control };
            if node.is_focussable()
                && state.mapper.tree().node(owner).ok().is_some_and(|node| {
                    !matches!(
                        node.descriptor.view().focus,
                        glory_render::FocusMode::Programmatic
                    )
                })
                && eligible(state, target)
            {
                result.push(target);
            }
        }
        pending.extend(node.children.iter().rev().copied());
    }
    result
}
pub(super) fn focus(state: &mut State, target: FocusTarget) {
    if eligible(state, target) && state.dom.doc.set_focus_to(target.control) {
        state.dom.invalidate();
    }
}
impl Dom {
    pub(super) fn create_display(&mut self, id: NodeId, frame: Rc<Frame>) -> Result<(), Error> {
        fn walk(
            dom: &mut Dom,
            frame: &Frame,
            nodes: &[Tree],
            path: &mut Vec<u16>,
            parent: EngineId,
            before: Option<EngineId>,
            targets: &mut [Option<EngineId>],
            controls: &mut Vec<EngineId>,
        ) {
            for (index, item) in nodes.iter().enumerate() {
                path.push(index as u16);
                let node = match item {
                    Tree::Text(text) => dom.doc.mutate().create_text_node(text),
                    Tree::Element {
                        tag: name, attrs, ..
                    } => {
                        let node = dom.doc.mutate().create_element(tag(name), Vec::new());
                        for (name, value) in attrs {
                            dom.doc.mutate().set_attribute(node, attr(name), value);
                        }
                        if *name == "button" || attrs.contains_key("data-gly-dclick") {
                            controls.push(node);
                        }
                        node
                    }
                    Tree::Outlet(_) => dom.doc.mutate().create_comment_node("gly:slot"),
                };
                for (slot, target_path) in frame.compiled().paths.iter().enumerate() {
                    if target_path == path
                        && !(matches!(item, Tree::Outlet(_))
                            && frame.compiled().slots[slot].kind == SlotKind::Children)
                    {
                        targets[slot] = Some(node);
                    }
                }
                if let Some(before) = before {
                    dom.doc.mutate().insert_nodes_before(before, &[node]);
                } else {
                    dom.doc.mutate().append_children(parent, &[node]);
                }
                if let Tree::Element { children, .. } = item {
                    walk(dom, frame, children, path, node, None, targets, controls);
                }
                path.pop();
            }
        }
        let park = self
            .range(id)?
            .park
            .ok_or(Error::InvalidPlacement("native display park"))?;
        let end = self.range(id)?.last;
        let mut targets = vec![None; frame.compiled().slots.len()];
        let mut controls = Vec::new();
        walk(
            self,
            &frame,
            &frame.tree(),
            &mut Vec::new(),
            park,
            Some(end),
            &mut targets,
            &mut controls,
        );
        let targets: Vec<_> = targets
            .into_iter()
            .map(|target| target.ok_or(Error::InvalidProps("native display target")))
            .collect::<Result<_, _>>()?;
        for control in &controls {
            let slot = targets.iter().enumerate().find_map(|(slot, target)| {
                (*target == *control && frame.compiled().slots[slot].kind == SlotKind::Click)
                    .then_some(slot as u16)
            });
            self.display_controls.insert(*control, (id, slot));
        }
        frame.bind(&self.display_queue, id);
        self.displays.insert(
            id,
            Instance {
                frame,
                targets,
                controls,
            },
        );
        Ok(())
    }
    pub(super) fn flush_display(&mut self) -> Result<(), Error> {
        let pending = self.display_queue.take();
        let mut changed = false;
        for id in pending {
            let Some(display) = self.displays.get(&id) else {
                continue;
            };
            for (slot, value) in display.frame.take_changes() {
                let node = display.targets[slot as usize];
                match (display.frame.compiled().slots[slot as usize].kind, value) {
                    (SlotKind::Text, Value::Text(text)) => {
                        self.doc.mutate().set_node_text(node, &text)
                    }
                    (SlotKind::Attribute(name), Value::Text(text)) => {
                        self.doc.mutate().set_attribute(node, attr(name), &text)
                    }
                    (SlotKind::Class(token, _), Value::Flag(enabled)) => {
                        let value = self
                            .doc
                            .get_node(node)
                            .and_then(|node| node.element_data())
                            .and_then(|node| node.attr(apace_dom::LocalName::from("class")))
                            .unwrap_or("")
                            .to_owned();
                        let mut parts: Vec<_> = value
                            .split_ascii_whitespace()
                            .filter(|part| *part != token)
                            .collect();
                        if enabled {
                            parts.push(token);
                        }
                        self.doc
                            .mutate()
                            .set_attribute(node, attr("class"), &parts.join(" "));
                    }
                    _ => return Err(Error::InvalidProps("native display patch kind")),
                }
                changed = true;
            }
        }
        if changed {
            self.invalidate();
        }
        Ok(())
    }
    #[cfg(feature = "shell")]
    pub(super) fn display_control(
        &self,
        mut node: EngineId,
        cross_widget: bool,
    ) -> Option<(EngineId, NodeId, Option<u16>)> {
        loop {
            if !cross_widget
                && (self.controls.contains_key(&node) || self.owners.contains_key(&node))
            {
                return None;
            }
            if let Some((owner, slot)) = self.display_controls.get(&node) {
                return Some((node, *owner, *slot));
            }
            node = self.doc.get_node(node)?.parent?;
        }
    }
}

#[cfg(feature = "shell")]
pub(super) fn receive(
    state: &mut State,
    target: EngineId,
    event: &apace_traits::events::DomEventData,
) -> bool {
    use apace_traits::events::DomEventData;
    use keyboard_types::{Key, NamedKey};
    let click = matches!(event, DomEventData::Click(_));
    let direct = state.dom.display_control(target, false).is_some();
    let Some((control, owner, slot)) = state.dom.display_control(target, click) else {
        return false;
    };
    let available = state.mapper.tree().active(owner)
        && state.mapper.tree().enabled(owner)
        && state.layers.allowed(state.mapper.tree(), owner)
        && state.dom.focus_visible(control)
        && state
            .dom
            .doc
            .get_node(control)
            .and_then(|node| node.element_data())
            .is_some_and(|node| node.attr(apace_dom::LocalName::from("disabled")).is_none());
    let activate = match event {
        DomEventData::Click(_) => {
            if direct
                && available
                && focus_scope::allowed(state, owner)
                && state
                    .dom
                    .doc
                    .get_node(control)
                    .is_some_and(|node| node.is_focussable())
                && state.dom.doc.set_focus_to(control)
            {
                state.dom.invalidate();
            }
            available
        }
        DomEventData::Focus(_) => {
            state.focus_changed(None);
            return true;
        }
        DomEventData::Blur(_) => {
            state.dom.display_space = None;
            return true;
        }
        DomEventData::KeyDown(key)
            if !key.is_composing
                && key.modifiers.is_empty()
                && key.key == Key::Named(NamedKey::Enter)
                && state
                    .dom
                    .doc
                    .get_node(control)
                    .is_some_and(|node| node.is_focussable()) =>
        {
            available
        }
        DomEventData::KeyDown(key)
            if key.modifiers.is_empty()
                && !key.is_composing
                && state
                    .dom
                    .doc
                    .get_node(control)
                    .is_some_and(|node| node.is_focussable())
                && matches!(&key.key,Key::Character(value) if value==" ") =>
        {
            state.dom.display_space = available.then_some(control);
            return true;
        }
        DomEventData::KeyUp(key) if matches!(&key.key,Key::Character(value) if value==" ") => {
            available && state.dom.display_space.take() == Some(control)
        }
        _ => return false,
    };
    if activate
        && let Some(slot) = slot
        && state
            .subscriptions
            .contains(&(owner, EventKind::DisplayClick))
    {
        state.inputs.push(Input::Event {
            node: owner,
            event: Event::DisplayClick(slot),
        });
    }
    direct
}
