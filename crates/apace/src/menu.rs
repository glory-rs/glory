//! Native connections to the shared, component-owned menu/context protocols.
//! Only current semantic ancestry and bounded mounted Tab traversal participate.
use super::*;
use glory_render::{
    ContextEditPolicy, ContextPoint, ContextSource, KeyboardInput, LayerPlacement, Props, Role,
};

fn live(state: &State, id: NodeId) -> bool {
    let tree = state.mapper.tree();
    tree.active(id)
        && tree.enabled(id)
        && !tree.inert(id)
        && layers::visible(tree, id)
        && state.layers.allowed(tree, id)
        && focus_scope::allowed(state, id)
}
fn editor(state: &State, id: NodeId) -> bool {
    state.mapper.tree().node(id).is_ok_and(|n| {
        matches!(
            n.descriptor.view().props,
            Props::Input(_) | Props::Select(_) | Props::RichEditor(_)
        )
    })
}
fn composing(state: &State, target: EngineId) -> bool {
    state
        .dom
        .doc
        .get_node(target)
        .and_then(|n| n.element_data())
        .and_then(|e| e.text_input_data())
        .is_some_and(|i| i.editor.is_composing())
}
fn input<'a>(key: &ApaceKeyEvent, name: &'a str, editable: bool) -> KeyboardInput<'a> {
    KeyboardInput {
        key: name,
        control: key.modifiers.contains(Modifiers::CONTROL),
        meta: key.modifiers.contains(Modifiers::META),
        alt: key.modifiers.contains(Modifiers::ALT),
        shift: key.modifiers.contains(Modifiers::SHIFT),
        repeat: key.is_auto_repeating,
        composing: key.is_composing,
        editable,
    }
}
fn nearest(state: &State, mut id: NodeId, role: Role) -> Option<NodeId> {
    loop {
        let node = state.mapper.tree().node(id).ok()?;
        if node.descriptor.view().role == Some(role) {
            return Some(id);
        }
        id = node.parent?;
    }
}
pub(super) fn key(
    state: &mut State,
    origin: NodeId,
    target: EngineId,
    key: &ApaceKeyEvent,
    name: &str,
) -> bool {
    if state.focused != Some(origin)
        || state.dom.control(origin).ok() != Some(target)
        || !live(state, origin)
        || composing(state, target)
    {
        return false;
    }
    let d = state
        .mapper
        .tree()
        .node(origin)
        .expect("live menu origin")
        .descriptor
        .view();
    let input = input(key, name, editor(state, origin));
    if d.semantics
        .as_ref()
        .and_then(|s| s.menu_item.as_ref())
        .is_some()
        && matches!(d.props, Props::Check(_))
        && !d.readonly()
        && input.menu_check_activation()
    {
        activate_control(state, origin);
        return true;
    }
    let trigger = d.semantics.as_ref().is_some_and(|s| {
        s.popup == Some(glory_render::PopupKind::Menu) && (s.menu_item.is_none() || s.menubar_item)
    });
    let root = nearest(state, origin, Role::Menu);
    let boundary = root.or_else(|| nearest(state, origin, Role::Menubar));
    let direction = boundary
        .and_then(|id| state.mapper.tree().node(id).ok())
        .and_then(|n| n.descriptor.view().semantics.as_ref())
        .map(|s| s.navigation_direction)
        .unwrap_or_default();
    let observer = if trigger { Some(origin) } else { root };
    if let Some(observer) = observer
        && state.subscriptions.contains(&(observer, EventKind::Menu))
        && let Some(command) = input.menu(direction, trigger)
    {
        state.inputs.push(Input::Event {
            node: origin,
            event: Event::Menu {
                from: origin,
                command,
            },
        });
        return true;
    }
    false
}

pub(super) fn context(state: &mut State, target: EngineId, event: &DomEventData) -> bool {
    use apace_traits::events::ApacePointerId;
    let Some(origin) = state.dom.owner(target) else {
        return false;
    };
    if !live(state, origin) || composing(state, target) {
        return false;
    }
    let source = match event {
        DomEventData::ContextMenu(pointer)
            if matches!(pointer.id, ApacePointerId::Mouse | ApacePointerId::Pen) =>
        {
            ContextSource::Pointer
        }
        DomEventData::KeyDown(key)
            if state.focused == Some(origin)
                && state.dom.control(origin).ok() == Some(target)
                && input(key, &key.key.to_string(), false).context_menu() =>
        {
            ContextSource::Keyboard
        }
        _ => return false,
    };
    let mut cursor = Some(origin);
    while let Some(id) = cursor {
        let Ok(node) = state.mapper.tree().node(id) else {
            return false;
        };
        let d = node.descriptor.view();
        if let Some(policy) = d.semantics.as_ref().and_then(|s| s.context_menu) {
            // A declared region is a boundary even with no live subscription.
            if !state.subscriptions.contains(&(id, EventKind::ContextMenu))
                || !state.mapper.tree().enabled(id)
                || !layers::visible(state.mapper.tree(), id)
                || !state.layers.allowed(state.mapper.tree(), id)
                || editor(state, origin) && policy == ContextEditPolicy::Native
            {
                return false;
            }
            let point = match event {
                DomEventData::ContextMenu(pointer) => ContextPoint {
                    x: f64::from(pointer.client_x()),
                    y: f64::from(pointer.client_y()),
                },
                _ => {
                    state.dom.resolve();
                    let Some(rect) = state.dom.doc.get_client_bounding_rect(target) else {
                        return false;
                    };
                    let Some(host) = state.dom.doc.get_node(target) else {
                        return false;
                    };
                    ContextPoint {
                        x: rect.x + host.scroll_offset().x,
                        y: rect.y + host.scroll_offset().y + rect.height,
                    }
                }
            };
            if point.validate().is_err() {
                return false;
            }
            state.inputs.push(Input::Event {
                node: id,
                event: Event::ContextMenu {
                    from: origin,
                    point,
                    source,
                },
            });
            return true;
        }
        cursor = node.parent;
    }
    false
}

/// Resolve the current return target after controlled mutations have flushed.
/// A refused close leaves the menu visible, but does not make it a Tab trap.
pub(in crate::document) fn exit(
    state: &mut State,
    id: NodeId,
    backwards: bool,
) -> Result<(), Error> {
    if !state.interactive {
        return Err(Error::Unsupported("headless Apace menu focus"));
    }
    let mut cursor = Some(id);
    while let Some(current) = cursor {
        let node = state.mapper.tree().node(current)?;
        if !node.placed || node.suspended || current != id && node.descriptor.view().hidden() {
            return Err(Error::Unsupported("menu exit on disconnected range"));
        }
        cursor = node.parent;
    }
    if state.mapper.tree().inert(id) || !state.mapper.tree().enabled(id) {
        return Err(Error::Unsupported("menu exit on unavailable range"));
    }
    let config = state
        .layers
        .declared
        .get(&id)
        .ok_or(Error::Unsupported("menu exit target"))?;
    let context = matches!(config.placement, LayerPlacement::ContextPoint(_));
    let tree = state.mapper.tree();
    let trigger = config
        .return_focus
        .as_ref()
        .and_then(|n| tree.named(n))
        .or_else(|| {
            context
                .then(|| config.anchor.as_ref().and_then(|n| tree.named(n)))
                .flatten()
        })
        .ok_or(Error::Unsupported("menu exit return target"))?;
    if !tree.active(trigger)
        || !tree.enabled(trigger)
        || tree.inert(trigger)
        || !state.layers.allowed(tree, trigger)
        || !focus_scope::allowed(state, trigger)
    {
        return Err(Error::Unsupported("menu exit unavailable return target"));
    }
    let bar = nearest(state, trigger, Role::Menubar);
    let boundary = context || bar.is_some();
    let trigger = bar.unwrap_or(trigger);
    state.dom.resolve();
    let scope = state
        .layers
        .modal()
        .map(|id| state.dom.main(id))
        .transpose()?
        .unwrap_or(state.dom.body);
    let mut pending = vec![scope];
    let mut values = Vec::new();
    let mut inspected = 0usize;
    while let Some(physical) = pending.pop() {
        inspected += 1;
        if inspected > 65_536 {
            return Err(Error::Unsupported("menu exit mounted-node budget"));
        }
        let Some(node) = state.dom.doc.get_node(physical) else {
            continue;
        };
        if let Some(owner) = state.dom.owner(physical)
            && state
                .mapper
                .tree()
                .node(owner)
                .is_ok_and(|n| n.descriptor.view().role == Some(Role::Menu))
        {
            continue;
        }
        if !state.dom.focus_visible(physical) {
            continue;
        }
        values.push(physical);
        pending.extend(node.children.iter().rev().copied());
    }
    let eligible = |physical| {
        state
            .dom
            .doc
            .get_node(physical)
            .is_some_and(|node| node.is_focussable())
            && state
                .dom
                .owner(physical)
                .is_some_and(|owner| live(state, owner))
    };
    let mut radio_stops = std::collections::HashMap::new();
    values.retain(|physical| {
        eligible(*physical) && radio::tab_stop(state, *physical, &eligible, &mut radio_stops)
    });
    let mut positions = values
        .iter()
        .enumerate()
        .filter(|(_, physical)| {
            state.dom.owner(**physical).is_some_and(|owner| {
                owner == trigger || boundary && layers::inside(state.mapper.tree(), owner, trigger)
            })
        })
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
        index.checked_add(1).filter(|i| *i < values.len())
    };
    if let Some(index) = next {
        let owner = state.dom.owner(values[index]).expect("live tab target");
        state.focus(owner)?;
    } else {
        state.dom.doc.clear_focus();
        state.dom.invalidate();
        state.sync_focus();
    }
    Ok(())
}
