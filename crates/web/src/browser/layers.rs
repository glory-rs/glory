//! Document top-layer integration. Only mounted layer declarations are indexed;
//! native listeners enqueue requests and never call application handlers.
use std::collections::BTreeMap;

use glory_render::{DismissReason, Error, Event, Layer, LayerPlacement, Mutation, NodeId};
use wasm_bindgen::JsCast;
use web_sys::{Element, KeyboardEvent, Node};

use super::{
    backend,
    dom::{self, Dom},
};

#[derive(Default)]
pub(super) struct Layers {
    declarations: BTreeMap<NodeId, Layer>,
    stack: Vec<Open>,
    inert: Vec<Element>,
}
#[derive(Clone)]
struct Open {
    id: NodeId,
    element: Element,
    config: Layer,
    previous: Option<Element>,
}
struct Working<'a> {
    dom: &'a Dom,
    layers: Layers,
}
impl std::ops::Deref for Working<'_> {
    type Target = Layers;
    fn deref(&self) -> &Layers {
        &self.layers
    }
}
impl std::ops::DerefMut for Working<'_> {
    fn deref_mut(&mut self) -> &mut Layers {
        &mut self.layers
    }
}
impl Drop for Working<'_> {
    fn drop(&mut self) {
        *self.dom.layers.borrow_mut() = std::mem::take(&mut self.layers);
    }
}
impl Layers {
    pub fn update(&mut self, tree: &glory_render::memory::Memory, mutations: &[Mutation]) {
        for id in mutations
            .iter()
            .flat_map(|mutation| match mutation {
                Mutation::Create { id, .. } | Mutation::Patch { id, .. } => {
                    std::slice::from_ref(id)
                }
                Mutation::CreatePlan { ids, .. } => ids.as_slice(),
                _ => &[],
            })
            .copied()
        {
            if let Ok(node) = tree.node(id) {
                match node
                    .descriptor
                    .semantics
                    .as_ref()
                    .and_then(|s| s.layer.clone())
                {
                    Some(layer) => {
                        self.declarations.insert(id, layer);
                    }
                    None => {
                        self.declarations.remove(&id);
                    }
                }
            }
        }
        self.declarations.retain(|id, _| tree.node(*id).is_ok());
    }
}
fn popover(element: &Element, method: &str) -> Result<(), Error> {
    let function = js_sys::Reflect::get(element, &method.into())
        .map_err(backend)?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| Error::Unsupported("browser top layer"))?;
    function.call0(element).map_err(backend)?;
    Ok(())
}
fn visible(element: &Element) -> bool {
    let bounds = element.get_bounding_client_rect();
    let visibility = element
        .owner_document()
        .and_then(|d| d.default_view())
        .and_then(|w| w.get_computed_style(element).ok().flatten())
        .and_then(|s| s.get_property_value("visibility").ok());
    element.is_connected()
        && element.closest("[hidden],[inert]").ok().flatten().is_none()
        && !matches!(visibility.as_deref(), Some("hidden" | "collapse"))
        && bounds.width() > 0.0
        && bounds.height() > 0.0
}
fn focusable(element: &Element) -> bool {
    visible(element)
        && !element.has_attribute("disabled")
        && (element.has_attribute("tabindex")
            || js_sys::Reflect::get(element, &"tabIndex".into())
                .ok()
                .and_then(|v| v.as_f64())
                .is_some_and(|v| v >= 0.0))
}
fn named(dom: &Dom, id: Option<&glory_render::SemanticId>) -> Option<Element> {
    let element = dom.document.get_element_by_id(id?.as_str())?;
    (element.closest("[data-gly-app]").ok().flatten().as_ref() == Some(&dom.root))
        .then_some(element)
}
fn named_control(dom: &Dom, id: Option<&glory_render::SemanticId>) -> Option<Element> {
    let element = named(dom, id)?;
    let token = element.get_attribute("data-gly-node")?;
    let id = *dom.tokens.borrow().get(&token)?;
    dom.entry(id).ok()?.control().ok()
}
fn candidates(element: &Element) -> Vec<Element> {
    let mut result = Vec::new();
    // Walk only this mounted focus scope. Check controls expose their real input.
    let mut pending = vec![element.clone()];
    while let Some(node) = pending.pop() {
        if node != *element && (node.has_attribute("hidden") || node.has_attribute("inert")) {
            continue;
        }
        let tab = js_sys::Reflect::get(&node, &"tabIndex".into())
            .ok()
            .and_then(|v| v.as_f64());
        if tab.is_some_and(|v| v >= 0.0) && !node.has_attribute("disabled") && visible(&node) {
            result.push(node.clone());
        }
        let mut child = node.last_element_child();
        while let Some(node) = child {
            child = node.previous_element_sibling();
            pending.push(node);
        }
    }
    result
}
fn first(dom: &Dom, open: &Open) -> Element {
    named_control(dom, open.config.initial_focus.as_ref())
        .filter(|e| open.element.contains(Some(e)) && focusable(e))
        .or_else(|| candidates(&open.element).into_iter().next())
        .unwrap_or_else(|| open.element.clone())
}
fn restore_inert(layers: &mut Layers) {
    for element in layers.inert.drain(..) {
        let _ = element.remove_attribute("inert");
    }
}
fn modal_inert(layers: &mut Layers) -> Result<(), Error> {
    let Some(open) = layers.stack.iter().rev().find(|open| open.config.modal) else {
        return Ok(());
    };
    let mut branch = open.element.clone();
    while let Some(parent) = branch.parent_element() {
        let mut child = parent.first_element_child();
        while let Some(sibling) = child {
            child = sibling.next_element_sibling();
            if sibling != branch && !sibling.has_attribute("inert") {
                sibling.set_attribute("inert", "").map_err(backend)?;
                layers.inert.push(sibling);
            }
        }
        if parent.tag_name() == "BODY" {
            break;
        }
        branch = parent;
    }
    Ok(())
}
struct Position {
    element: Element,
    x: f64,
    y: f64,
}
fn position(dom: &Dom, open: &Open) -> Result<Option<Position>, Error> {
    let Some(anchor) = named(dom, open.config.anchor.as_ref()) else {
        return Ok(None);
    };
    let anchor = anchor.get_bounding_client_rect();
    let bounds = open.element.get_bounding_client_rect();
    let window = dom
        .document
        .default_view()
        .ok_or(Error::Unsupported("window"))?;
    let width = window
        .inner_width()
        .map_err(backend)?
        .as_f64()
        .unwrap_or(0.0);
    let height = window
        .inner_height()
        .map_err(backend)?
        .as_f64()
        .unwrap_or(0.0);
    let left = if open.config.placement == LayerPlacement::AnchorEnd {
        anchor.right() - bounds.width()
    } else {
        anchor.left()
    };
    let below = anchor.bottom() + 6.0;
    let top = if below + bounds.height() > height - 8.0 && anchor.top() >= bounds.height() + 6.0 {
        anchor.top() - bounds.height() - 6.0
    } else {
        below
    };
    Ok(Some(Position {
        element: open.element.clone(),
        x: left.clamp(8.0, (width - bounds.width() - 8.0).max(8.0)),
        y: top.clamp(8.0, (height - bounds.height() - 8.0).max(8.0)),
    }))
}
fn place(position: Position) -> Result<(), Error> {
    let style = position
        .element
        .dyn_ref::<web_sys::HtmlElement>()
        .ok_or(Error::Unsupported("layer element"))?
        .style();
    style
        .set_property("--gly-layer-x", &format!("{}px", position.x))
        .map_err(backend)?;
    style
        .set_property("--gly-layer-y", &format!("{}px", position.y))
        .map_err(backend)?;
    Ok(())
}
fn positions(dom: &Dom, stack: &[Open]) -> Result<(), Error> {
    let pending = stack
        .iter()
        .map(|open| position(dom, open))
        .collect::<Result<Vec<_>, _>>()?;
    for position in pending.into_iter().flatten() {
        place(position)?;
    }
    Ok(())
}
pub(super) fn layout(dom: &Dom) -> Result<(), Error> {
    positions(dom, &dom.layers.borrow().stack)
}
pub(super) fn sync(dom: &Dom) -> Result<(), Error> {
    // Native focus events may fire synchronously. Do not hold this borrow while
    // invoking a browser method; event callbacks see an empty temporary stack.
    let mut state = Working {
        dom,
        layers: std::mem::take(&mut *dom.layers.borrow_mut()),
    };
    let layers = &mut *state;
    restore_inert(layers);
    let active = dom.document.active_element();
    let mut restore = None;
    let mut retained = Vec::new();
    for mut open in layers.stack.iter().rev().cloned() {
        let live = layers.declarations.get(&open.id).filter(|_| {
            open.element.is_connected() && open.element.closest("[hidden]").ok().flatten().is_none()
        });
        if let Some(config) = live {
            open.config = config.clone();
            retained.push(open);
        } else {
            let focused = active
                .as_ref()
                .is_none_or(|e| open.element.contains(Some(e)) || e.tag_name() == "BODY");
            if open.element.matches(":popover-open").unwrap_or(false) {
                popover(&open.element, "hidePopover")?;
            }
            if focused {
                restore = named_control(dom, open.config.return_focus.as_ref()).or(open.previous);
            }
        }
    }
    retained.reverse();
    layers.stack = retained;
    let mut autofocus = None;
    for (id, config) in &layers.declarations {
        if layers.stack.iter().any(|open| open.id == *id) {
            continue;
        }
        let Ok(element) = dom.entry(*id).and_then(|e| e.element()) else {
            continue;
        };
        if !element.is_connected() || element.closest("[hidden]").map_err(backend)?.is_some() {
            continue;
        }
        let open = Open {
            id: *id,
            element,
            config: config.clone(),
            previous: active.clone(),
        };
        popover(&open.element, "showPopover")?;
        autofocus = Some(first(dom, &open));
        layers.stack.push(open);
    }
    modal_inert(layers)?;
    positions(dom, &layers.stack)?;
    let target = autofocus.or(restore).filter(focusable);
    drop(state);
    if let Some(target) = target {
        dom::focus(&target)?;
    }
    Ok(())
}
pub(super) fn clear(dom: &Dom) {
    let mut layers = std::mem::take(&mut *dom.layers.borrow_mut());
    restore_inert(&mut layers);
    let mut restore = None;
    let active = dom.document.active_element();
    for open in layers.stack.into_iter().rev() {
        if active
            .as_ref()
            .is_some_and(|e| open.element.contains(Some(e)))
        {
            restore = named_control(dom, open.config.return_focus.as_ref()).or(open.previous);
        }
        if open.element.matches(":popover-open").unwrap_or(false) {
            let _ = popover(&open.element, "hidePopover");
        }
    }
    if let Some(target) = restore.filter(focusable) {
        let _ = dom::focus(&target);
    }
}
pub(super) fn receive(dom: &Dom, name: &str, event: &web_sys::Event) -> Result<bool, Error> {
    if !matches!(
        name,
        "keydown" | "focusin" | "pointerdown" | "click" | "scroll" | "resize"
    ) {
        return Ok(false);
    }
    if !dom.live.get() || event.default_prevented() {
        return Ok(false);
    }
    let stack = dom.layers.borrow().stack.clone();
    let Some(top) = stack.last() else {
        return Ok(false);
    };
    let target = event.target().and_then(|v| v.dyn_into::<Node>().ok());
    let inside = target
        .as_ref()
        .is_some_and(|node| top.element.contains(Some(node)));
    let modal = stack.iter().rev().find(|open| open.config.modal);
    if name == "keydown" {
        let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
            return Ok(false);
        };
        let composing = target
            .as_ref()
            .and_then(|node| node.dyn_ref::<Element>())
            .and_then(|element| super::events::target(dom, element))
            .and_then(|(id, _)| dom.entry(id).ok())
            .is_some_and(|entry| entry.editing.borrow().composing);
        if key.is_composing() || composing {
            return Ok(false);
        }
        if key.key() == "Escape"
            && !key.repeat()
            && !key.alt_key()
            && !key.ctrl_key()
            && !key.meta_key()
        {
            event.prevent_default();
            event.stop_immediate_propagation();
            if top.config.escape {
                dom.queue(top.id, Event::Dismiss(DismissReason::Escape));
            }
            return Ok(true);
        }
        if key.key() == "Tab"
            && modal.is_some()
            && !key.alt_key()
            && !key.ctrl_key()
            && !key.meta_key()
        {
            let scope = &top.element;
            let values = candidates(scope);
            let active = dom.document.active_element();
            let current = values.iter().position(|e| Some(e) == active.as_ref());
            let next = if values.is_empty() {
                scope.clone()
            } else {
                let i = match (current, key.shift_key()) {
                    (Some(0), true) | (None, true) => values.len() - 1,
                    (Some(i), true) => i - 1,
                    (Some(i), false) => (i + 1) % values.len(),
                    (None, false) => 0,
                };
                values[i].clone()
            };
            event.prevent_default();
            dom::focus(&next)?;
            return Ok(true);
        }
    }
    if name == "focusin"
        && let Some(modal) = modal
    {
        let allowed = stack
            .iter()
            .skip_while(|open| open.id != modal.id)
            .any(|open| {
                target
                    .as_ref()
                    .is_some_and(|node| open.element.contains(Some(node)))
            });
        if !allowed {
            dom::focus(&first(dom, top))?;
            return Ok(true);
        }
    }
    if name == "pointerdown" && !inside {
        if event
            .dyn_ref::<web_sys::PointerEvent>()
            .is_some_and(|p| p.button() != 0 || !p.is_primary())
        {
            return Ok(false);
        }
        let trigger = target
            .as_ref()
            .and_then(|node| node.dyn_ref::<Element>())
            .and_then(|element| element.closest("[aria-controls]").ok().flatten())
            .is_some_and(|element| {
                element.get_attribute("aria-controls") == top.element.get_attribute("id")
            });
        if !trigger && top.config.outside {
            dom.queue(top.id, Event::Dismiss(DismissReason::Outside));
        }
        if modal.is_some() {
            event.prevent_default();
            event.stop_immediate_propagation();
            return Ok(true);
        }
    }
    if name == "click" && modal.is_some() && !inside {
        event.prevent_default();
        event.stop_immediate_propagation();
        return Ok(true);
    }
    if matches!(name, "scroll" | "resize") {
        dom.layer_layout.set(true);
        dom.frame.input();
    }
    Ok(false)
}
