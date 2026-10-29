//! Physical template targets shared by CSR and hydration.
use super::{backend, dom::Dom};
use glory_html::{
    display::{Frame, Tree, Value},
    template::Kind,
};
use glory_render::{Error, Event, EventKind, NodeId};
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, Node};

#[derive(Clone)]
pub(super) struct Instance {
    pub frame: Rc<Frame>,
    pub targets: Vec<Node>,
}
impl Instance {
    pub fn create(
        document: &Document,
        frame: Rc<Frame>,
        key: u64,
        parent: &Node,
        before: Option<&Node>,
    ) -> Result<Self, Error> {
        fn walk(
            document: &Document,
            frame: &Frame,
            key: u64,
            nodes: &[Tree],
            path: &mut Vec<u16>,
            parent: &Node,
            before: Option<&Node>,
            targets: &mut [Option<Node>],
        ) -> Result<(), Error> {
            for (index, item) in nodes.iter().enumerate() {
                path.push(index as u16);
                let node: Node = match item {
                    Tree::Text(text) => {
                        parent
                            .insert_before(&document.create_comment("gly:dt"), before)
                            .map_err(backend)?;
                        document.create_text_node(text).into()
                    }
                    Tree::Element { tag, attrs, .. } => {
                        let element = document.create_element(tag).map_err(backend)?;
                        for (name, value) in attrs {
                            element.set_attribute(name, value).map_err(backend)?;
                        }
                        element.into()
                    }
                    Tree::Outlet(slot) => document
                        .create_comment(&format!("gly:slot:{key}:{slot}"))
                        .into(),
                };
                for (slot, target_path) in frame.compiled().paths.iter().enumerate() {
                    if target_path == path
                        && !(matches!(item, Tree::Outlet(_))
                            && frame.compiled().slots[slot].kind == Kind::Children)
                    {
                        targets[slot] = Some(node.clone());
                    }
                }
                parent.insert_before(&node, before).map_err(backend)?;
                if let Tree::Element { children, .. } = item {
                    walk(document, frame, key, children, path, &node, None, targets)?;
                }
                path.pop();
            }
            Ok(())
        }
        let mut targets = vec![None; frame.compiled().slots.len()];
        walk(
            document,
            &frame,
            key,
            &frame.tree(),
            &mut Vec::new(),
            parent,
            before,
            &mut targets,
        )?;
        Ok(Self {
            frame,
            targets: targets
                .into_iter()
                .map(|node| node.ok_or(Error::InvalidProps("display target shape")))
                .collect::<Result<_, _>>()?,
        })
    }
    pub fn bind(&self, dom: &Dom, id: NodeId) -> Result<(), Error> {
        let token = format!("{}.{}.{}", id.runtime().value(), id.slot(), id.generation());
        for (slot, node) in self.targets.iter().enumerate() {
            if self.frame.compiled().slots[slot].kind == Kind::Click {
                node.dyn_ref::<Element>()
                    .ok_or(Error::InvalidProps("display click element"))?
                    .set_attribute("data-gly-display-node", &token)
                    .map_err(backend)?;
            }
        }
        self.frame.bind(&dom.display_queue, id);
        Ok(())
    }
    pub fn destination(&self, slot: u16) -> Result<(Node, Option<Node>), Error> {
        let target = self
            .targets
            .get(slot as usize)
            .ok_or(Error::InvalidPlacement("display outlet"))?
            .clone();
        match self.frame.compiled().slots[slot as usize].kind {
            Kind::Children => Ok((target, None)),
            Kind::Range => Ok((
                target
                    .parent_node()
                    .ok_or(Error::InvalidPlacement("detached display outlet"))?,
                Some(target),
            )),
            _ => Err(Error::InvalidPlacement("display outlet kind")),
        }
    }
    pub fn flush(&self) -> Result<(), Error> {
        for (slot, value) in self.frame.take_changes() {
            let target = &self.targets[slot as usize];
            match (self.frame.compiled().slots[slot as usize].kind, value) {
                (Kind::Text, Value::Text(text)) => target.set_node_value(Some(&text)),
                (Kind::Attribute(name), Value::Text(text)) => target
                    .dyn_ref::<Element>()
                    .ok_or(Error::InvalidProps("display attribute target"))?
                    .set_attribute(name, &text)
                    .map_err(backend)?,
                (Kind::Class(token, _), Value::Flag(enabled)) => {
                    target
                        .dyn_ref::<Element>()
                        .ok_or(Error::InvalidProps("display class target"))?
                        .class_list()
                        .toggle_with_force(token, enabled)
                        .map_err(backend)?;
                }
                _ => return Err(Error::InvalidProps("display patch kind")),
            }
        }
        Ok(())
    }
}
pub(super) fn click(dom: &Dom, element: &Element) -> Result<bool, Error> {
    let Some(target) = element
        .closest("[data-gly-display-node]")
        .map_err(backend)?
    else {
        return Ok(false);
    };
    if target.closest("[data-gly-app]").map_err(backend)?.as_ref() != Some(&dom.root) {
        return Ok(false);
    }
    let Some(id) = target
        .get_attribute("data-gly-display-node")
        .and_then(|token| dom.id_for_token(&token))
    else {
        return Ok(false);
    };
    let Some(slot) = target
        .get_attribute("data-gly-dclick")
        .and_then(|slot| slot.parse::<u16>().ok())
    else {
        return Ok(false);
    };
    let entry = dom.entry(id)?;
    let display = entry.display.borrow();
    let Some(display) = display.as_ref() else {
        return Ok(false);
    };
    if display
        .targets
        .get(slot as usize)
        .is_some_and(|node| node == target.as_ref())
        && display.frame.compiled().slots[slot as usize].kind == Kind::Click
    {
        if !target.has_attribute("disabled")
            && entry.interests.borrow().contains(&EventKind::DisplayClick)
        {
            dom.queue(id, Event::DisplayClick(slot));
        }
        // A Widget inside a raw click target keeps its own activation too.
        return Ok(!element
            .closest("[data-gly-node]")
            .map_err(backend)?
            .is_some_and(|semantic| target.contains(Some(semantic.as_ref()))));
    }
    Ok(false)
}
