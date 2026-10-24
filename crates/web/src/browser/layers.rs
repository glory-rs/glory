//! Document top-layer integration. Only mounted layer declarations are indexed;
//! native listeners enqueue requests and never call application handlers.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use glory_render::{
    DismissReason, Error, Event, Layer, LayerFocus, LayerPlacement, Mutation, NodeId,
};
use wasm_bindgen::JsCast;
use web_sys::{Element, KeyboardEvent, Node};

use super::{
    backend,
    dom::{self, Dom},
};
#[path = "layer_document.rs"]
mod document;
pub(super) use document::DocumentLayers;

#[derive(Default)]
pub(super) struct Layers {
    declarations: BTreeMap<NodeId, Layer>,
    scope_declarations: BTreeMap<NodeId, glory_render::FocusScopeConfig>,
    stack: Vec<Open>,
    blocked: BTreeSet<NodeId>,
    anchors: BTreeMap<NodeId, bool>,
    lost_anchors: BTreeSet<NodeId>,
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
        self.scope_declarations.clear();
        for id in tree.focus_scope_roots() {
            if tree.active(id)
                && !tree.inert(id)
                && tree.enabled(id)
                && let Some(config) = tree.node(id).ok().and_then(|node| {
                    node.descriptor
                        .view()
                        .semantics
                        .as_ref()?
                        .focus_scope
                        .as_ref()
                })
            {
                self.scope_declarations.insert(id, config.clone());
            }
        }
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
                    .view()
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
        self.anchors
            .retain(|id, _| self.declarations.contains_key(id));
        self.lost_anchors
            .retain(|id| self.declarations.contains_key(id));
        for (id, layer) in &self.declarations {
            let live = tree.layer_anchor_live(*id);
            if self.anchors.insert(*id, live) == Some(true)
                && !live
                && layer.anchor.is_some()
                && tree.active(*id)
            {
                self.lost_anchors.insert(*id);
            }
            if live {
                self.lost_anchors.remove(id);
            }
        }
        self.blocked.clear();
        self.blocked.extend(
            self.declarations
                .keys()
                .copied()
                .filter(|id| tree.inert(*id)),
        );
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
    let id = dom.id_for_token(&token)?;
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
struct Position {
    element: Element,
    x: f64,
    y: f64,
}
fn position(dom: &Dom, open: &Open, offset: (f64, f64)) -> Result<Option<Position>, Error> {
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
    if let LayerPlacement::Viewport(placement) = open.config.placement {
        let resolved = placement.resolve(
            glory_render::Rect {
                width: bounds.width(),
                height: bounds.height(),
                ..Default::default()
            },
            glory_render::Rect {
                width,
                height,
                ..Default::default()
            },
        )?;
        return Ok(Some(Position {
            element: open.element.clone(),
            x: resolved.x,
            y: resolved.y,
        }));
    }
    let Some(anchor) = named(dom, open.config.anchor.as_ref()) else {
        return Ok(None);
    };
    let anchor = anchor.get_bounding_client_rect();
    if let LayerPlacement::ContextPoint(point) = open.config.placement {
        return Ok(Some(Position {
            element: open.element.clone(),
            x: point.x.clamp(8.0, (width - bounds.width() - 8.0).max(8.0)),
            y: point
                .y
                .clamp(8.0, (height - bounds.height() - 8.0).max(8.0)),
        }));
    }
    let LayerPlacement::Anchor(placement) = open.config.placement else {
        return Ok(None);
    };
    let resolved = placement.resolve(
        glory_render::Rect {
            x: anchor.x() + offset.0,
            y: anchor.y() + offset.1,
            width: anchor.width(),
            height: anchor.height(),
        },
        glory_render::Rect {
            width: bounds.width(),
            height: bounds.height(),
            ..Default::default()
        },
        glory_render::Rect {
            width,
            height,
            ..Default::default()
        },
    )?;
    Ok(Some(Position {
        element: open.element.clone(),
        x: resolved.x,
        y: resolved.y,
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
    // Resolve only the current live layer graph. An anchor inside another
    // pending layer moves by that layer's planned delta. Topological read order
    // also handles identity-slot reuse; stack/NodeId order is not ancestry.
    let indices = stack
        .iter()
        .enumerate()
        .map(|(index, open)| (open.id, index))
        .collect::<BTreeMap<_, _>>();
    let mut parents = vec![None; stack.len()];
    let mut children = vec![Vec::new(); stack.len()];
    let mut ready = VecDeque::new();
    for (index, open) in stack.iter().enumerate() {
        let mut cursor = named(dom, open.config.anchor.as_ref());
        while let Some(element) = cursor {
            if let Some(parent) = element
                .get_attribute("data-gly-node")
                .and_then(|token| dom.id_for_token(&token))
                .and_then(|id| indices.get(&id))
                .copied()
            {
                parents[index] = Some(parent);
                children[parent].push(index);
                break;
            }
            cursor = element.parent_element();
        }
        if parents[index].is_none() {
            ready.push_back(index)
        }
    }
    let mut pending = (0..stack.len())
        .map(|_| None)
        .collect::<Vec<Option<Position>>>();
    let mut processed = 0;
    while let Some(index) = ready.pop_front() {
        let offset = parents[index]
            .and_then(|parent| pending[parent].as_ref())
            .map_or((0.0, 0.0), |parent| {
                let current = parent.element.get_bounding_client_rect();
                (parent.x - current.x(), parent.y - current.y())
            });
        pending[index] = position(dom, &stack[index], offset)?;
        processed += 1;
        ready.extend(children[index].iter().copied());
    }
    if processed != stack.len() {
        return Err(Error::InvalidProps(
            "layer anchor geometry cannot contain cycles",
        ));
    }
    // No layout-affecting writes occur until every observation is collected.
    for position in pending.into_iter().flatten() {
        place(position)?;
    }
    Ok(())
}
pub(super) fn layout(dom: &Dom) -> Result<(), Error> {
    positions(dom, &dom.layers.borrow().stack)
}
pub(super) fn raise(dom: &Dom, id: NodeId) -> Result<(), Error> {
    dom.document_layers.raise(dom, id)?;
    let mut layers = dom.layers.borrow_mut();
    if let Some(index) = layers.stack.iter().position(|entry| entry.id == id) {
        let entry = layers.stack.remove(index);
        layers.stack.push(entry);
    }
    drop(layers);
    layout(dom)
}
pub(super) fn sync(dom: &Dom) -> Result<(), Error> {
    dom.document_layers.prepare();
    // Native focus events may fire synchronously. Do not hold this borrow while
    // invoking a browser method; event callbacks see an empty temporary stack.
    let mut state = Working {
        dom,
        layers: std::mem::take(&mut *dom.layers.borrow_mut()),
    };
    let layers = &mut *state;
    for id in std::mem::take(&mut layers.lost_anchors) {
        dom.queue(id, Event::Dismiss(DismissReason::AnchorLost));
    }
    // Top-layer elements must carry effective inert themselves. The live
    // semantic index also covers boxless ancestry and inherited inert changes.
    for id in layers.declarations.keys() {
        let Ok(element) = dom.entry(*id).and_then(|entry| entry.element()) else {
            continue;
        };
        if layers.blocked.contains(id) {
            element.set_attribute("inert", "").map_err(backend)?;
        } else if !element.has_attribute("data-gly-inert") {
            element.remove_attribute("inert").map_err(backend)?;
        }
    }
    let active = dom.document.active_element();
    let mut restore = None;
    let mut retained = Vec::new();
    for mut open in layers.stack.iter().rev().cloned() {
        let live = layers.declarations.get(&open.id).filter(|_| {
            layers.anchors.get(&open.id) != Some(&false)
                && open.element.is_connected()
                && open.element.closest("[hidden]").ok().flatten().is_none()
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
            if focused && open.config.focus == LayerFocus::Manage {
                restore = named_control(dom, open.config.return_focus.as_ref()).or(open.previous);
            }
        }
    }
    retained.reverse();
    layers.stack = retained;
    let mut autofocus = None;
    for (id, config) in &layers.declarations {
        if layers.anchors.get(id) == Some(&false) {
            continue;
        }
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
            previous: restore.clone().or_else(|| active.clone()),
        };
        popover(&open.element, "showPopover")?;
        if open.config.focus == LayerFocus::Manage {
            autofocus = Some(first(dom, &open));
        }
        layers.stack.push(open);
    }
    positions(dom, &layers.stack)?;
    let target = autofocus.or(restore);
    drop(state);
    dom.document_layers.sync(target)
}
pub(super) fn clear(dom: &Dom) {
    dom.document_layers.prepare();
    let layers = std::mem::take(&mut *dom.layers.borrow_mut());
    let mut restore = None;
    let active = dom.document.active_element();
    for open in layers.stack.into_iter().rev() {
        if open.config.focus == LayerFocus::Manage
            && active
                .as_ref()
                .is_some_and(|e| open.element.contains(Some(e)))
        {
            restore = named_control(dom, open.config.return_focus.as_ref()).or(open.previous);
        }
        if open.element.matches(":popover-open").unwrap_or(false) {
            let _ = popover(&open.element, "hidePopover");
        }
    }
    dom.document_layers.unregister(dom);
    let _ = dom.document_layers.sync(restore);
}
/// Actual mounted document traversal belongs to the host. Bound the physical
/// walk, exclude the complete Menu tree, and never touch a business collection.
pub(super) fn exit_menu(dom: &Dom, id: NodeId, backwards: bool) -> Result<(), Error> {
    let config = dom
        .layers
        .borrow()
        .declarations
        .get(&id)
        .cloned()
        .ok_or(Error::Unsupported("menu exit target"))?;
    let context = matches!(config.placement, LayerPlacement::ContextPoint(_));
    let trigger = named_control(dom, config.return_focus.as_ref())
        .or_else(|| {
            context
                .then(|| named_control(dom, config.anchor.as_ref()))
                .flatten()
        })
        .ok_or(Error::Unsupported("menu exit return target"))?;
    let bar = if trigger.has_attribute("data-gly-menubar-item") {
        trigger.closest("[role=menubar]").map_err(backend)?
    } else {
        None
    };
    let trigger = bar.clone().unwrap_or(trigger);
    let boundary = context || bar.is_some();
    let scope = dom
        .document_layers
        .modal_scope()
        .or_else(|| {
            dom.layers
                .borrow()
                .stack
                .iter()
                .rev()
                .find(|open| open.config.modal && open.element.contains(Some(&trigger)))
                .map(|open| open.element.clone())
        })
        .or_else(|| dom.document.body().map(Into::into))
        .ok_or(Error::Unsupported("menu exit document"))?;
    let mut pending = vec![scope];
    let mut values = Vec::new();
    let mut inspected = 0usize;
    while let Some(node) = pending.pop() {
        inspected += 1;
        if inspected > 65_536 {
            return Err(Error::Unsupported("menu exit mounted-node budget"));
        }
        if node.has_attribute("hidden")
            || node.has_attribute("inert")
            || node.get_attribute("role").as_deref() == Some("menu")
        {
            continue;
        }
        if js_sys::Reflect::get(&node, &"tabIndex".into())
            .ok()
            .and_then(|v| v.as_f64())
            .is_some_and(|v| v >= 0.0)
            && !node.has_attribute("disabled")
            && visible(&node)
        {
            values.push(node.clone());
        }
        let mut child = node.last_element_child();
        while let Some(node) = child {
            child = node.previous_element_sibling();
            pending.push(node);
        }
    }
    let mut positions = values
        .iter()
        .enumerate()
        .filter(|(_, value)| *value == &trigger || boundary && trigger.contains(Some(value)))
        .map(|(index, _)| index);
    let index = if backwards {
        positions.next()
    } else {
        positions.next_back()
    }
    .ok_or(Error::Unsupported("menu exit trigger is not a tab stop"))?;
    let next = if backwards {
        index.checked_sub(1)
    } else {
        index.checked_add(1).filter(|index| *index < values.len())
    };
    if let Some(index) = next {
        dom::focus(&values[index])?;
    } else if let Some(active) = dom
        .document
        .active_element()
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    {
        active.blur().map_err(backend)?;
    }
    Ok(())
}
pub(super) fn receive(dom: &Dom, name: &str, event: &web_sys::Event) -> Result<bool, Error> {
    if !matches!(
        name,
        "keydown"
            | "focusin"
            | "pointerdown"
            | "click"
            | "scroll"
            | "resize"
            | "wheel"
            | "touchmove"
    ) {
        return Ok(false);
    }
    if !dom.live.get() || event.default_prevented() {
        return Ok(false);
    }
    dom.document_layers.receive(name, event)
}
fn receive_local(dom: &Dom, name: &str, event: &web_sys::Event) -> Result<bool, Error> {
    let stack: Vec<_> = dom
        .layers
        .borrow()
        .stack
        .iter()
        .filter(|open| open.element.closest("[inert]").ok().flatten().is_none())
        .cloned()
        .collect();
    let target = event.target().and_then(|v| v.dyn_into::<Node>().ok());
    let owned_target = target
        .as_ref()
        .and_then(|target| target.dyn_ref::<Element>())
        .is_some_and(|element| super::events::target(dom, element).is_some());
    let unfocused_target = target
        .as_ref()
        .and_then(|target| target.dyn_ref::<Element>())
        .is_some_and(|element| matches!(element.tag_name().as_str(), "BODY" | "HTML"));
    let top = if name == "keydown" {
        stack.iter().rev().find(|open| {
            open.config.focus == LayerFocus::Manage
                || owned_target
                || (unfocused_target
                    && (open.element.matches(":hover").unwrap_or(false)
                        || named(dom, open.config.anchor.as_ref())
                            .is_some_and(|anchor| anchor.matches(":hover").unwrap_or(false))))
        })
    } else {
        stack.last()
    };
    let Some(top) = top else {
        return Ok(false);
    };
    let menu_root = (top.element.get_attribute("role").as_deref() == Some("menu")).then(|| {
        stack
            .iter()
            .find(|open| {
                open.element.get_attribute("role").as_deref() == Some("menu")
                    && open.element.contains(Some(&top.element))
            })
            .unwrap_or(top)
    });
    let inside = target
        .as_ref()
        .is_some_and(|node| top.element.contains(Some(node)));
    let modal = stack.iter().rev().find(|open| open.config.modal);
    let inside_modal = dom.document_layers.allows(target.as_ref());
    let managed = stack
        .iter()
        .rev()
        .find(|open| open.config.focus == LayerFocus::Manage);
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
        if menu_root.is_some()
            && key.key() == "Tab"
            && !key.alt_key()
            && !key.ctrl_key()
            && !key.meta_key()
            && let Some((from, _)) = target
                .as_ref()
                .and_then(|node| node.dyn_ref::<Element>())
                .and_then(|element| super::events::target(dom, element))
        {
            event.prevent_default();
            dom.queue(
                from,
                Event::Menu {
                    from,
                    command: glory_render::MenuCommand::Exit {
                        backwards: key.shift_key(),
                    },
                },
            );
            return Ok(true);
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
            && dom.document_layers.modal_scope().is_some()
            && !key.alt_key()
            && !key.ctrl_key()
            && !key.meta_key()
        {
            let scope = &managed.expect("modal layers manage focus").element;
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
            dom::focus(&first(dom, managed.unwrap_or(modal)))?;
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
            let dismiss = menu_root
                .filter(|root| {
                    !target
                        .as_ref()
                        .is_some_and(|node| root.element.contains(Some(node)))
                })
                .unwrap_or(top);
            dom.queue(dismiss.id, Event::Dismiss(DismissReason::Outside));
        }
        if !inside_modal {
            event.prevent_default();
            event.stop_immediate_propagation();
            return Ok(true);
        }
    }
    if name == "click" && !inside_modal {
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
