//! Intrinsic radio behavior. Explicit Navigate handlers keep their own roving
//! policy; all grouping is delegated to the current native document/tree/forms.
use super::*;
use std::collections::HashMap;

pub(super) fn is_radio(state: &State, id: NodeId) -> bool {
    state.mapper.tree().node(id).is_ok_and(|n| {
        matches!(n.descriptor.view().props,
        glory_render::Props::Check(p) if p.mode == glory_render::CheckMode::Radio)
    })
}
fn auto(state: &State, physical: EngineId) -> bool {
    state
        .dom
        .owner(physical)
        .and_then(|id| state.mapper.tree().node(id).ok())
        .is_some_and(|n| n.descriptor.view().focus == glory_render::FocusMode::Auto)
}
pub(super) fn tab_stop(
    state: &State,
    id: EngineId,
    eligible: &impl Fn(EngineId) -> bool,
    cache: &mut HashMap<EngineId, bool>,
) -> bool {
    if let Some(allowed) = cache.get(&id) {
        return *allowed;
    }
    if !auto(state, id) {
        return true;
    }
    let members = state.dom.doc.radio_group(id);
    if members.is_empty() {
        return true;
    }
    let mut first = None;
    let mut selected = None;
    let mut explicit = false;
    for peer in &members {
        if !eligible(*peer) {
            continue;
        }
        if !auto(state, *peer) {
            explicit = true;
            continue;
        }
        first = first.or(Some(*peer));
        if state
            .dom
            .doc
            .get_node(*peer)
            .and_then(|n| n.element_data())
            .and_then(|e| e.checkbox_input_checked())
            == Some(true)
        {
            selected = Some(*peer);
        }
    }
    let selected = if explicit { None } else { selected.or(first) };
    // This numeric cache lasts for one Tab search and only includes a radio
    // group actually encountered by it. Ordinary Tab stays lazy; no eager
    // eligibility/focus-scope scan of every mounted control is introduced.
    for peer in members {
        if auto(state, peer) {
            cache.insert(peer, Some(peer) == selected);
        }
    }
    cache.get(&id).copied().unwrap_or(true)
}

fn editable(state: &State, physical: EngineId) -> Option<NodeId> {
    let id = state.dom.owner(physical)?;
    (is_radio(state, id)
        && state.mapper.tree().active(id)
        && state.mapper.tree().enabled(id)
        && !state
            .mapper
            .tree()
            .node(id)
            .ok()?
            .descriptor
            .view()
            .readonly()
        && state.dom.focus_visible(physical)
        && state.layers.allowed(state.mapper.tree(), id)
        && focus_scope::allowed(state, id))
    .then_some(id)
}
pub(super) fn key(
    state: &mut State,
    origin: NodeId,
    physical: EngineId,
    key: &ApaceKeyEvent,
) -> bool {
    if !is_radio(state, origin) || state.dom.control(origin).ok() != Some(physical) {
        return false;
    }
    if state
        .mapper
        .tree()
        .navigation(origin)
        .is_some_and(|(root, _)| state.subscriptions.contains(&(root, EventKind::Navigate)))
    {
        // The caller already offered supported commands to that listener. Do
        // not invent a second selection policy for its other axis keys.
        return false;
    }
    let backwards = match key.key {
        Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowUp | NamedKey::Home) => true,
        Key::Named(NamedKey::ArrowRight | NamedKey::ArrowDown | NamedKey::End) => false,
        _ => return false,
    };
    state.dom.resolve();
    let group = state.dom.doc.radio_group(physical);
    let Some(index) = group.iter().position(|id| *id == physical) else {
        return false;
    };
    let order: Box<dyn Iterator<Item = usize>> = match key.key {
        Key::Named(NamedKey::Home) => Box::new(0..group.len()),
        Key::Named(NamedKey::End) => Box::new((0..group.len()).rev()),
        _ if backwards => {
            Box::new((1..=group.len()).map(|step| (index + group.len() - step) % group.len()))
        }
        _ => Box::new((1..=group.len()).map(|step| (index + step) % group.len())),
    };
    if let Some(id) = order
        .filter_map(|index| editable(state, group[index]))
        .next()
        && state.focus(id).is_ok()
    {
        activate_control(state, id);
    }
    true
}
