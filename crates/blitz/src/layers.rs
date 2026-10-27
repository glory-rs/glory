//! Current native layer leases. The engine owns viewport layout, paint and hit
//! testing; this adapter owns finite semantic placement and queued input policy.
use super::*;
use glory_render::{Action, Change, DismissReason, Layer, LayerPlacement, Patch};

#[derive(Clone)]
pub(super) struct Open {
    pub id: NodeId,
    pub config: Layer,
    pub previous: Option<NodeId>,
    previous_scope: Option<NodeId>,
    restore: bool,
    outside_pending: bool,
    physical: bool,
    scope: bool,
}

#[derive(Default)]
pub(super) struct Layers {
    pub(super) declared: BTreeMap<NodeId, Layer>,
    pub open: Vec<Open>,
    lost: BTreeSet<NodeId>,
    scroll: Option<blitz_dom::Point<f64>>,
}

// Blitz's client rect includes the queried node's own scroll. Placement and
// outside-hit policy need its border box, retaining all ancestor/viewport scroll.
fn border_box(dom: &Dom, physical: EngineId) -> Option<Rect> {
    let rect = dom.doc.get_client_bounding_rect(physical)?;
    let own = dom.doc.get_node(physical)?.scroll_offset();
    Some(Rect {
        x: rect.x + own.x,
        y: rect.y + own.y,
        width: rect.width,
        height: rect.height,
    })
}

pub(super) fn inside(tree: &glory_render::memory::Memory, mut id: NodeId, scope: NodeId) -> bool {
    loop {
        if id == scope {
            return true;
        }
        let Some(parent) = tree.node(id).ok().and_then(|node| node.parent) else {
            return false;
        };
        id = parent;
    }
}

impl Layers {
    pub fn boundary(&self, tree: &glory_render::memory::Memory, mut id: NodeId) -> Option<NodeId> {
        loop {
            if self
                .open
                .iter()
                .any(|entry| entry.physical && entry.id == id)
            {
                return Some(id);
            }
            id = tree.node(id).ok()?.parent?;
        }
    }
    pub fn modal(&self) -> Option<NodeId> {
        self.open
            .iter()
            .rev()
            .find(|entry| entry.config.modal)
            .map(|entry| entry.id)
    }
    pub fn allowed(&self, tree: &glory_render::memory::Memory, id: NodeId) -> bool {
        self.modal().is_none_or(|modal| {
            inside(tree, id, modal)
                || self
                    .open
                    .iter()
                    .any(|entry| entry.scope && inside(tree, id, entry.id))
        })
    }
    /// Accessibility keeps transparent ancestors of the current modal while
    /// omitting unrelated branches. No business source participates.
    #[cfg(all(
        feature = "accesskit",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    pub fn ax_branch(&self, tree: &glory_render::memory::Memory, id: NodeId) -> bool {
        self.modal().is_none_or(|modal| {
            self.allowed(tree, id)
                || inside(tree, modal, id)
                || self
                    .open
                    .iter()
                    .any(|entry| entry.scope && inside(tree, entry.id, id))
        })
    }
}

pub(super) fn visible(tree: &glory_render::memory::Memory, mut id: NodeId) -> bool {
    loop {
        let Ok(node) = tree.node(id) else {
            return false;
        };
        let d = node.descriptor.view();
        if !node.placed
            || node.suspended
            || d.hidden()
            || d.semantics.as_ref().is_some_and(|s| s.inert)
            || d.style.display == Some(glory_render::style::Display::None)
        {
            return false;
        }
        let Some(parent) = node.parent else {
            return true;
        };
        id = parent;
    }
}

pub(super) fn eligible(state: &State, id: NodeId) -> bool {
    state.mapper.tree().active(id)
        && state.mapper.tree().enabled(id)
        && state.layers.allowed(state.mapper.tree(), id)
        && state.dom.control(id).ok().is_some_and(|control| {
            state.dom.focus_visible(control)
                && state
                    .dom
                    .doc
                    .get_node(control)
                    .is_some_and(|node| node.is_focussable())
        })
}

fn first_focus(state: &State, root: NodeId) -> Option<NodeId> {
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        let Ok(node) = state.mapper.tree().node(id) else {
            continue;
        };
        if !visible(state.mapper.tree(), id) {
            continue;
        }
        if eligible(state, id) {
            return Some(id);
        }
        pending.extend(node.children.iter().rev());
    }
    None
}

fn handoff_previous(
    tree: &glory_render::memory::Memory,
    closed: &[Open],
    previous_top: Option<NodeId>,
) -> Option<NodeId> {
    let mut entry = closed
        .iter()
        .rev()
        .find(|entry| Some(entry.id) == previous_top && entry.restore)?;
    // Only this synchronization's closing scopes participate. Implicit previous
    // targets keep their exact lifetime; no closed-layer history is retained.
    for _ in 0..closed.len() {
        let valid = |id| {
            visible(tree, id)
                && tree.enabled(id)
                && tree.node(id).is_ok_and(|node| {
                    Action::Focus(id)
                        .validate_target(node.descriptor.view())
                        .is_ok()
                })
        };
        if let Some(id) = entry
            .config
            .return_focus
            .as_ref()
            .and_then(|name| tree.named(name))
            .filter(|id| valid(*id))
            .or(entry.previous.filter(|id| valid(*id)))
        {
            return Some(id);
        }
        entry = closed
            .iter()
            .find(|parent| Some(parent.id) == entry.previous_scope && parent.restore)?;
    }
    None
}

pub(super) fn sync(state: &mut State, mutations: &[Mutation]) -> Result<(), Error> {
    let mut layers = std::mem::take(&mut state.layers);
    let result = sync_inner(state, &mut layers, mutations);
    state.layers = layers;
    let opened = result?;
    #[cfg(feature = "shell")]
    if state
        .drag
        .as_ref()
        .is_some_and(|drag| !state.layers.allowed(state.mapper.tree(), drag.from))
        && let Some(drag) = state.drag.take()
        && drag.started
    {
        state.suppress_click = true;
        if state.mapper.tree().node(drag.from).is_ok() {
            state.inputs.push(Input::Event {
                node: drag.from,
                event: Event::Drag {
                    from: drag.from,
                    phase: glory_render::DragPhase::Cancel,
                    x: drag.x,
                    y: drag.y,
                },
            });
        }
    }
    if state.interactive
        && let Some(scope) = opened.filter(|id| {
            state.layers.open.iter().any(|entry| {
                entry.id == *id && entry.config.focus == glory_render::LayerFocus::Manage
            })
        })
    {
        let requested = state
            .layers
            .open
            .iter()
            .find(|entry| entry.id == scope)
            .and_then(|entry| entry.config.initial_focus.as_ref())
            .and_then(|name| state.mapper.tree().named(name))
            .filter(|id| eligible(state, *id));
        if let Some(id) = requested
            .or_else(|| first_focus(state, scope))
            .or(Some(scope))
        {
            let _ = state.focus(id);
        }
    }
    repair_focus(state);
    Ok(())
}

pub(super) fn raise(state: &mut State, id: NodeId) -> Result<(), Error> {
    sync(state, &[])?;
    let tree = state.mapper.tree();
    let root = state
        .layers
        .open
        .iter()
        .position(|entry| entry.id == id && entry.physical)
        .ok_or(Error::Unsupported("native layer is no longer open"))?;
    if state.layers.open[root].config.modal || !state.layers.allowed(tree, id) || !tree.enabled(id)
    {
        return Err(Error::Unsupported(
            "native layer raise crosses the current modal",
        ));
    }
    let mut members = vec![false; state.layers.open.len()];
    members[root] = true;
    let mut queue = std::collections::VecDeque::from([root]);
    while let Some(parent) = queue.pop_front() {
        let parent = state.layers.open[parent].id;
        for (index, entry) in state.layers.open.iter().enumerate() {
            if !members[index]
                && (inside(tree, entry.id, parent)
                    || entry
                        .config
                        .anchor
                        .as_ref()
                        .and_then(|name| tree.named(name))
                        .is_some_and(|anchor| inside(tree, anchor, parent)))
            {
                members[index] = true;
                queue.push_back(index);
            }
        }
    }
    if !members[..root].iter().any(|member| *member) && members[root..].iter().all(|member| *member)
    {
        return Ok(());
    }
    let mut root_entry = None;
    let mut related = Vec::new();
    let mut retained = Vec::new();
    for (index, entry) in std::mem::take(&mut state.layers.open)
        .into_iter()
        .enumerate()
    {
        if index == root {
            root_entry = Some(entry)
        } else if members[index] {
            related.push(entry)
        } else {
            retained.push(entry)
        }
    }
    let root_entry = root_entry.expect("current root");
    let mut raised = vec![root_entry];
    raised.extend(related);
    for entry in &raised {
        let main = state.dom.main(entry.id)?;
        state
            .dom
            .doc
            .set_top_layer(main, None)
            .map_err(|_| invariant("native layer raise"))?;
    }
    retained.extend(raised);
    state.layers.open = retained;
    state.dom.invalidate();
    sync(state, &[])
}

fn sync_inner(
    state: &mut State,
    layers: &mut Layers,
    mutations: &[Mutation],
) -> Result<Option<NodeId>, Error> {
    let tree = state.mapper.tree();
    let mut touched = BTreeSet::new();
    for mutation in mutations {
        match mutation {
            Mutation::Create { id, node }
                if node.semantics.as_ref().is_some_and(|s| s.layer.is_some()) =>
            {
                touched.insert(*id);
            }
            Mutation::Patch {
                id,
                patch: Patch::Semantics(Change::Set(Some(semantics))),
            } if semantics.layer.is_some() => {
                touched.insert(*id);
            }
            Mutation::CreatePlan { ids, plan } => {
                touched.extend(ids.iter().zip(plan.nodes()).filter_map(|(id, node)| {
                    node.descriptor
                        .semantics
                        .as_ref()
                        .and_then(|s| s.layer.as_ref())
                        .map(|_| *id)
                }));
            }
            _ => {}
        }
    }
    touched.extend(layers.declared.keys());
    for id in touched {
        let config = tree
            .node(id)
            .ok()
            .and_then(|node| node.descriptor.view().semantics.as_ref())
            .and_then(|s| s.layer.clone());
        if let Some(config) = config {
            layers.declared.insert(id, config);
        } else {
            layers.declared.remove(&id);
            layers.lost.remove(&id);
        }
    }
    if layers.declared.is_empty() && layers.open.is_empty() {
        return Ok(None);
    }
    let mut wanted = BTreeMap::new();
    for (&id, config) in &layers.declared {
        if !visible(tree, id) {
            layers.lost.remove(&id);
            continue;
        }
        if !tree.layer_anchor_live(id) {
            if layers.open.iter().any(|entry| entry.id == id) && layers.lost.insert(id) {
                state.inputs.push(Input::Event {
                    node: id,
                    event: Event::Dismiss(DismissReason::AnchorLost),
                });
            }
            continue;
        }
        layers.lost.remove(&id);
        wanted.insert(id, config.clone());
    }
    let old = std::mem::take(&mut layers.open);
    let previous_top = old
        .iter()
        .rev()
        .find(|entry| entry.physical)
        .map(|entry| entry.id);
    let mut closed = Vec::new();
    for mut entry in old {
        if let Some(config) = wanted.remove(&entry.id) {
            if config.focus == glory_render::LayerFocus::Preserve {
                entry.restore = false;
            }
            entry.config = config;
            layers.open.push(entry);
        } else {
            if let Ok(main) = state.dom.main(entry.id) {
                state
                    .dom
                    .doc
                    .set_top_layer(main, None)
                    .map_err(|_| invariant("native layer close"))?;
                state.dom.invalidate();
            }
            closed.push(entry);
        }
    }
    let previous = state
        .dom
        .doc
        .get_focussed_node_id()
        .and_then(|id| state.dom.owner(id))
        .or_else(|| handoff_previous(tree, &closed, previous_top));
    let previous_scope = previous.and_then(|previous| {
        layers
            .open
            .iter()
            .rev()
            .chain(closed.iter().rev())
            .find(|entry| inside(tree, previous, entry.id))
            .map(|entry| entry.id)
    });
    let mut opened = Vec::new();
    for (id, config) in wanted {
        let restore = config.focus == glory_render::LayerFocus::Manage;
        layers.open.push(Open {
            id,
            config,
            previous,
            previous_scope,
            restore,
            outside_pending: false,
            physical: false,
            scope: false,
        });
        opened.push(id);
    }
    // Transfer only current fallback links before discarding closed entries.
    // No closed-layer history survives this synchronization.
    for closed in closed.iter().rev() {
        for entry in &mut layers.open {
            if entry.previous_scope == Some(closed.id) {
                entry.previous = closed
                    .config
                    .return_focus
                    .as_ref()
                    .and_then(|name| tree.named(name))
                    .or(closed.previous);
                entry.previous_scope = closed.previous_scope;
            }
        }
    }
    let modal_index = layers.open.iter().rposition(|entry| entry.config.modal);
    for index in 0..layers.open.len() {
        let scope = if let Some(modal_index) = modal_index {
            let modal = layers.open[modal_index].id;
            index == modal_index
                || index > modal_index
                    && (inside(tree, layers.open[index].id, modal)
                        || layers.open[index]
                            .config
                            .anchor
                            .as_ref()
                            .and_then(|name| tree.named(name))
                            .is_some_and(|anchor| {
                                inside(tree, anchor, modal)
                                    || layers.open[modal_index..index]
                                        .iter()
                                        .any(|entry| entry.scope && inside(tree, anchor, entry.id))
                            }))
        } else {
            true
        };
        layers.open[index].scope = scope;
        layers.open[index].physical = modal_index.is_none_or(|modal| index <= modal) || scope;
    }
    let opened = opened.into_iter().rev().find(|id| {
        layers.open.iter().any(|entry| {
            entry.id == *id
                && entry.physical
                && entry.config.focus == glory_render::LayerFocus::Manage
        })
    });
    if layers.open.iter().any(|entry| entry.config.modal) {
        layers
            .scroll
            .get_or_insert_with(|| state.dom.doc.viewport_scroll());
    } else if let Some(scroll) = layers.scroll.take() {
        state.dom.doc.set_viewport_scroll(scroll);
        state.dom.invalidate();
    }
    if layers.open.is_empty() && closed.is_empty() {
        return Ok(None);
    }
    for entry in &layers.open {
        let main = state.dom.main(entry.id)?;
        if !entry.physical {
            if state
                .dom
                .doc
                .set_top_layer(main, None)
                .map_err(|_| invariant("native inactive layer"))?
            {
                state.dom.invalidate();
            }
            continue;
        }
        let config = blitz_dom::TopLayerConfig {
            modal: entry.config.modal,
            contain_scroll: true,
            backdrop: entry
                .config
                .modal
                .then(|| blitz_dom::util::Color::from_rgba8(0, 0, 0, 89)),
        };
        if state
            .dom
            .doc
            .set_top_layer(main, Some(config))
            .map_err(|_| invariant("native layer admission"))?
        {
            state.dom.invalidate();
        }
    }
    state.dom.resolve();
    for entry in &layers.open {
        if !entry.physical {
            continue;
        }
        let main = state.dom.main(entry.id)?;
        let Some(panel) = border_box(&state.dom, main) else {
            continue;
        };
        let (width, height) = state.dom.doc.viewport().logical_size();
        let (x, y) = match entry.config.placement {
            LayerPlacement::Center => (
                (f64::from(width) - panel.width) / 2.,
                (f64::from(height) - panel.height) / 2.,
            ),
            LayerPlacement::ContextPoint(point) => (
                point
                    .x
                    .clamp(8.0, (f64::from(width) - panel.width - 8.0).max(8.0)),
                point
                    .y
                    .clamp(8.0, (f64::from(height) - panel.height - 8.0).max(8.0)),
            ),
            LayerPlacement::Viewport(placement) => {
                let position = placement.resolve(
                    Rect {
                        width: panel.width,
                        height: panel.height,
                        ..Default::default()
                    },
                    Rect {
                        width: f64::from(width),
                        height: f64::from(height),
                        ..Default::default()
                    },
                )?;
                (position.x, position.y)
            }
            LayerPlacement::Anchor(policy) => {
                let Some(anchor) = entry
                    .config
                    .anchor
                    .as_ref()
                    .and_then(|name| tree.named(name))
                else {
                    continue;
                };
                let anchor = state.dom.main(anchor)?;
                let Some(anchor) = border_box(&state.dom, anchor) else {
                    continue;
                };
                let position = policy.resolve(
                    Rect {
                        x: anchor.x,
                        y: anchor.y,
                        width: anchor.width,
                        height: anchor.height,
                    },
                    Rect {
                        x: panel.x,
                        y: panel.y,
                        width: panel.width,
                        height: panel.height,
                    },
                    Rect {
                        x: 0.,
                        y: 0.,
                        width: f64::from(width),
                        height: f64::from(height),
                    },
                )?;
                (position.x, position.y)
            }
            _ => continue,
        };
        let old = state
            .dom
            .doc
            .get_node(main)
            .and_then(|node| node.attr(blitz_dom::LocalName::from("style")))
            .unwrap_or("");
        let css = state
            .dom
            .doc
            .style_attr_set_property(old, "--gly-layer-x", &format!("{x}px"), false)
            .and_then(|css| {
                state.dom.doc.style_attr_set_property(
                    &css,
                    "--gly-layer-y",
                    &format!("{y}px"),
                    false,
                )
            })
            .ok_or_else(|| invariant("native anchor style"))?;
        if css != old {
            state
                .dom
                .doc
                .mutate()
                .set_attribute(main, attr("style"), &css);
            state.dom.invalidate();
        }
    }
    state.dom.resolve();
    // Focus work follows the complete physical stack update. Installing the
    // current leases first makes programmatic focus validate the same scope.
    let allowed = |id| layers.allowed(tree, id) && visible(tree, id) && tree.enabled(id);
    let restored = closed.iter().rev().find_map(|entry| {
        entry
            .config
            .return_focus
            .as_ref()
            .and_then(|name| tree.named(name))
            .filter(|id| allowed(*id))
            .or(entry.previous.filter(|id| allowed(*id)))
    });
    let target = if opened.is_none()
        && closed
            .iter()
            .any(|entry| Some(entry.id) == previous_top && entry.restore)
    {
        restored
    } else {
        None
    };
    if state.interactive
        && let Some(id) = target
        && visible(tree, id)
        && tree.enabled(id)
        && let Ok(control) = state.dom.control(id)
        && state.dom.focus_visible(control)
        && state.dom.doc.set_focus_to(control)
    {
        state.dom.invalidate();
    }
    Ok(opened)
}

pub(super) fn repair_focus(state: &mut State) {
    if !state.interactive {
        return;
    }
    if state.layers.modal().is_none() {
        return;
    }
    let Some(scope) = state
        .layers
        .open
        .iter()
        .rev()
        .find(|entry| {
            entry.physical && entry.scope && entry.config.focus == glory_render::LayerFocus::Manage
        })
        .map(|entry| entry.id)
    else {
        return;
    };
    let current = state
        .dom
        .doc
        .get_focussed_node_id()
        .and_then(|id| state.dom.owner(id));
    if current.is_some_and(|id| eligible(state, id)) {
        return;
    }
    if let Some(id) = first_focus(state, scope).or(Some(scope))
        && let Ok(control) = state.dom.control(id)
        && state.dom.focus_visible(control)
        && state.dom.doc.set_focus_to(control)
    {
        state.dom.invalidate();
    }
}

#[cfg(feature = "shell")]
fn dismiss(state: &mut State, id: NodeId, reason: DismissReason) {
    let Some(entry) = state.layers.open.iter_mut().find(|entry| entry.id == id) else {
        return;
    };
    if reason == DismissReason::Outside {
        entry.restore = entry.config.modal;
        if std::mem::replace(&mut entry.outside_pending, true) {
            return;
        }
    } else {
        entry.restore = entry.config.focus == glory_render::LayerFocus::Manage;
    }
    state.inputs.push(Input::Event {
        node: id,
        event: Event::Dismiss(reason),
    });
}

#[cfg(feature = "shell")]
pub(super) fn outside_focus(state: &mut State, next: Option<NodeId>) {
    let Some(next) = next else { return };
    let Some(top) = state.layers.open.iter().rev().find(|entry| entry.physical) else {
        return;
    };
    if !top.config.modal
        && top.config.outside
        && top.config.focus == glory_render::LayerFocus::Manage
        && !inside(state.mapper.tree(), next, top.id)
    {
        dismiss(state, top.id, DismissReason::Outside);
    }
}

pub(super) fn drained(state: &mut State) {
    for entry in &mut state.layers.open {
        entry.outside_pending = false;
    }
}

#[cfg(feature = "shell")]
pub(super) fn lock_viewport(state: &mut State) {
    if let Some(scroll) = state.layers.scroll
        && state.dom.doc.viewport_scroll() != scroll
    {
        state.dom.doc.set_viewport_scroll(scroll);
        state.dom.invalidate();
    }
}

#[cfg(feature = "shell")]
pub(super) fn receive(
    state: &mut State,
    target: EngineId,
    event: &blitz_traits::events::DomEventData,
) -> bool {
    use blitz_traits::events::DomEventData;
    use keyboard_types::{Key, Modifiers, NamedKey};
    let origin = state.dom.owner(target);
    let key_event = matches!(event, DomEventData::KeyDown(_));
    let Some(top) = state
        .layers
        .open
        .iter()
        .rev()
        .find(|entry| {
            entry.physical
                && (!key_event
                    || entry.config.focus == glory_render::LayerFocus::Manage
                    || origin.is_some_and(|origin| {
                        inside(state.mapper.tree(), origin, entry.id)
                            || entry
                                .config
                                .anchor
                                .as_ref()
                                .and_then(|name| state.mapper.tree().named(name))
                                .is_some_and(|anchor| inside(state.mapper.tree(), origin, anchor))
                    }))
        })
        .cloned()
    else {
        return false;
    };
    let belongs = origin.is_some_and(|id| inside(state.mapper.tree(), id, top.id));
    let allowed = origin.is_some_and(|id| state.layers.allowed(state.mapper.tree(), id));
    match event {
        DomEventData::KeyDown(key) if key.key == Key::Named(NamedKey::Escape) => {
            let composing = state
                .dom
                .doc
                .get_node(target)
                .and_then(|node| node.element_data())
                .and_then(|element| element.text_input_data())
                .is_some_and(|input| input.editor.is_composing());
            if key.is_composing
                || composing
                || !key.modifiers.is_empty()
                || !belongs && state.layers.modal().is_none()
            {
                return false;
            }
            if top.config.escape {
                dismiss(state, top.id, DismissReason::Escape);
            }
            true
        }
        DomEventData::KeyDown(key)
            if key.key == Key::Named(NamedKey::Tab)
                && (key.modifiers & !Modifiers::SHIFT).is_empty()
                && !key.is_composing =>
        {
            // The existing native Tab driver performs the traversal. Its root
            // and eligibility predicate below use this same current scope.
            false
        }
        DomEventData::PointerDown(pointer) => {
            let x = f64::from(pointer.client_x());
            let y = f64::from(pointer.client_y());
            if !x.is_finite() || !y.is_finite() {
                return true;
            }
            state.dom.resolve();
            let hit = belongs
                && state
                    .dom
                    .main(top.id)
                    .ok()
                    .and_then(|id| border_box(&state.dom, id))
                    .is_some_and(|rect| {
                        x >= rect.x
                            && y >= rect.y
                            && x < rect.x + rect.width
                            && y < rect.y + rect.height
                    });
            if !hit {
                if top.config.outside {
                    dismiss(state, top.id, DismissReason::Outside);
                }
                return !allowed && state.layers.modal().is_some();
            }
            !allowed && state.layers.modal().is_some()
        }
        DomEventData::Focus(_)
            if !belongs
                && state.layers.modal().is_none()
                && top.config.outside
                && top.config.focus == glory_render::LayerFocus::Manage =>
        {
            dismiss(state, top.id, DismissReason::Outside);
            false
        }
        DomEventData::Focus(_) if belongs => {
            if let Some(entry) = state.layers.open.last_mut() {
                entry.restore = entry.config.focus == glory_render::LayerFocus::Manage;
            }
            false
        }
        DomEventData::Click(pointer) if state.layers.modal().is_some() => {
            let x = f64::from(pointer.client_x());
            let y = f64::from(pointer.client_y());
            !allowed
                || origin.is_some_and(|id| state.layers.modal() == Some(id))
                    && state
                        .dom
                        .main(state.layers.modal().expect("modal branch"))
                        .ok()
                        .and_then(|id| border_box(&state.dom, id))
                        .is_some_and(|rect| {
                            x < rect.x
                                || y < rect.y
                                || x >= rect.x + rect.width
                                || y >= rect.y + rect.height
                        })
        }
        DomEventData::Wheel(_) if state.layers.modal().is_some() => {
            // Engine scrolling preserves editor/touch/fling behavior and stops
            // its user-scroll chain at the explicit visual-root boundary.
            !allowed
        }
        _ => false,
    }
}

#[cfg(test)]
mod handoff_tests {
    use super::*;
    use glory_render::{Descriptor, NodeIds, Props, Render, RuntimeId, SemanticId, Semantics};

    fn closed(id: NodeId, previous: Option<NodeId>, previous_scope: Option<NodeId>) -> Open {
        Open {
            id,
            config: Layer::default(),
            previous,
            previous_scope,
            restore: true,
            outside_pending: false,
            physical: false,
            scope: false,
        }
    }
    fn target(tree: &mut glory_render::memory::Memory, id: NodeId) {
        let mut descriptor = Descriptor::new(Props::Button(Default::default()));
        descriptor.semantics = Some(Box::new(Semantics {
            id: Some(SemanticId::new("handoff-opener").unwrap()),
            ..Default::default()
        }));
        tree.apply(&[
            Mutation::Create {
                id,
                node: Box::new(descriptor),
            },
            Mutation::Place {
                id,
                parent: None,
                before: None,
            },
        ])
        .unwrap();
    }
    #[test]
    fn native_handoff_keeps_exact_local_lifetimes_and_rejects_retired_or_foreign_targets() {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let mut tree = glory_render::memory::Memory::new(runtime);
        let opener = ids.allocate();
        let old_layer = ids.allocate();
        target(&mut tree, opener);
        let local = closed(old_layer, Some(opener), None);
        assert_eq!(
            handoff_previous(&tree, std::slice::from_ref(&local), Some(old_layer)),
            Some(opener)
        );
        assert_eq!(
            handoff_previous(&tree, std::slice::from_ref(&local), Some(opener)),
            None
        );
        let foreign_runtime = RuntimeId::new();
        let foreign = NodeIds::new(foreign_runtime).allocate();
        let mut foreign_tree = glory_render::memory::Memory::new(foreign_runtime);
        target(&mut foreign_tree, foreign);
        assert_eq!(
            handoff_previous(
                &tree,
                &[closed(old_layer, Some(foreign), None)],
                Some(old_layer)
            ),
            None
        );
        tree.apply(&[Mutation::Dispose { id: opener }]).unwrap();
        let replacement = ids.allocate();
        target(&mut tree, replacement);
        assert_eq!(
            handoff_previous(&tree, &[local], Some(old_layer)),
            None,
            "implicit focus must not rebind to a replacement with the same semantic name"
        );
        assert!(foreign_tree.node(foreign).is_ok());
    }
    #[test]
    fn native_handoff_excludes_hidden_inert_disabled_and_suspended_current_targets() {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let mut tree = glory_render::memory::Memory::new(runtime);
        let opener = ids.allocate();
        let old_layer = ids.allocate();
        target(&mut tree, opener);
        let parent = ids.allocate();
        tree.apply(&[
            Mutation::Create {
                id: parent,
                node: Box::new(Descriptor::new(Props::View)),
            },
            Mutation::Place {
                id: parent,
                parent: None,
                before: None,
            },
            Mutation::Place {
                id: opener,
                parent: Some(parent),
                before: None,
            },
        ])
        .unwrap();
        let closed = [closed(old_layer, Some(opener), None)];
        for semantics in [
            Semantics {
                hidden: true,
                ..Default::default()
            },
            Semantics {
                inert: true,
                ..Default::default()
            },
        ] {
            tree.apply(&[Mutation::Patch {
                id: parent,
                patch: Patch::Semantics(Change::Set(Some(Box::new(semantics)))),
            }])
            .unwrap();
            assert_eq!(handoff_previous(&tree, &closed, Some(old_layer)), None);
        }
        tree.apply(&[
            Mutation::Patch {
                id: parent,
                patch: Patch::Semantics(Change::Clear),
            },
            Mutation::Patch {
                id: opener,
                patch: Patch::Enabled(Change::Set(false)),
            },
        ])
        .unwrap();
        assert_eq!(handoff_previous(&tree, &closed, Some(old_layer)), None);
        tree.apply(&[
            Mutation::Patch {
                id: opener,
                patch: Patch::Enabled(Change::Set(true)),
            },
            Mutation::Suspend { id: opener },
        ])
        .unwrap();
        assert_eq!(handoff_previous(&tree, &closed, Some(old_layer)), None);
        tree.apply(&[Mutation::Resume {
            id: opener,
            parent: Some(parent),
            before: None,
        }])
        .unwrap();
        assert_eq!(
            handoff_previous(&tree, &closed, Some(old_layer)),
            Some(opener)
        );
    }
    #[test]
    fn native_handoff_walks_only_current_managed_closed_scopes_and_honors_explicit_return() {
        let runtime = RuntimeId::new();
        let mut ids = NodeIds::new(runtime);
        let mut tree = glory_render::memory::Memory::new(runtime);
        let opener = ids.allocate();
        let parent = ids.allocate();
        let top = ids.allocate();
        let retired_child = ids.allocate();
        target(&mut tree, opener);
        let outer = closed(parent, Some(opener), None);
        let inner = closed(top, Some(retired_child), Some(parent));
        assert_eq!(
            handoff_previous(&tree, &[outer.clone(), inner.clone()], Some(top)),
            Some(opener)
        );
        let mut preserve = outer.clone();
        preserve.restore = false;
        assert_eq!(
            handoff_previous(&tree, &[preserve, inner.clone()], Some(top)),
            None
        );
        let mut preserve = inner.clone();
        preserve.restore = false;
        assert_eq!(handoff_previous(&tree, &[outer, preserve], Some(top)), None);
        let mut explicit = inner;
        explicit.config.return_focus = Some(SemanticId::new("handoff-opener").unwrap());
        assert_eq!(
            handoff_previous(&tree, &[explicit], Some(top)),
            Some(opener)
        );
        let a = closed(parent, None, Some(top));
        let b = closed(top, None, Some(parent));
        assert_eq!(
            handoff_previous(&tree, &[a, b], Some(top)),
            None,
            "current closed-scope traversal is bounded"
        );
    }
}
