//! Current nonmodal focus policy, independent of engine top-layer boxes/AX.
use super::*;
use display::{FocusTarget, eligible, focus};
use glory_render::FocusScopeConfig;

#[derive(Clone)]
struct Open {
    id: NodeId,
    config: FocusScopeConfig,
    previous: Option<FocusTarget>,
    previous_scope: Option<NodeId>,
    focused: bool,
}
#[derive(Default)]
pub(super) struct Scopes(Vec<Open>);
fn live(state: &State, open: &Open) -> bool {
    layers::visible(state.mapper.tree(), open.id)
        && state.mapper.tree().enabled(open.id)
        && state.mapper.tree().node(open.id).ok().is_some_and(|node| {
            node.descriptor
                .view()
                .semantics
                .as_ref()
                .is_some_and(|semantics| {
                    semantics
                        .focus_scope
                        .as_ref()
                        .is_some_and(|scope| scope.active)
                })
        })
        && state.layers.allowed(state.mapper.tree(), open.id)
}
#[cfg(all(test, feature = "shell"))]
impl Scopes {
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
fn current(state: &State) -> Option<FocusTarget> {
    display::current_focus(state)
}
fn inside(state: &State, id: NodeId, scope: NodeId) -> bool {
    layers::inside(state.mapper.tree(), id, scope)
        || state.layers.open.iter().any(|open| {
            layers::inside(state.mapper.tree(), id, open.id)
                && open
                    .config
                    .anchor
                    .as_ref()
                    .and_then(|name| state.mapper.tree().named(name))
                    .is_some_and(|anchor| layers::inside(state.mapper.tree(), anchor, scope))
        })
}
fn candidates(state: &State, root: NodeId) -> Vec<FocusTarget> {
    display::focus_candidates(state, root)
}
fn first(state: &State, open: &Open) -> FocusTarget {
    open.config
        .initial_focus
        .as_ref()
        .and_then(|name| state.mapper.tree().named(name))
        .filter(|id| inside(state, *id, open.id) && layers::eligible(state, *id))
        .and_then(|id| display::focus_target(state, id))
        .or_else(|| candidates(state, open.id).into_iter().next())
        .unwrap_or_else(|| display::focus_target(state, open.id).expect("focus scope control"))
}
pub(super) fn allowed(state: &State, id: NodeId) -> bool {
    state
        .focus_scopes
        .0
        .iter()
        .rev()
        .find(|open| {
            live(state, open)
                && state.mapper.tree().node(open.id).ok().is_some_and(|node| {
                    node.descriptor
                        .view()
                        .semantics
                        .as_ref()
                        .is_some_and(|semantics| {
                            semantics
                                .focus_scope
                                .as_ref()
                                .is_some_and(|scope| scope.contain_focus)
                        })
                })
        })
        .is_none_or(|open| inside(state, id, open.id))
}
pub(super) fn sync(state: &mut State) {
    let declarations = state
        .mapper
        .tree()
        .focus_scope_roots()
        .filter_map(|id| {
            let config = state
                .mapper
                .tree()
                .node(id)
                .ok()?
                .descriptor
                .view()
                .semantics
                .as_ref()?
                .focus_scope
                .as_ref()?;
            (config.active
                && layers::visible(state.mapper.tree(), id)
                && state.mapper.tree().enabled(id))
            .then(|| (id, config.clone()))
        })
        .collect::<Vec<_>>();
    if declarations.is_empty() && state.focus_scopes.0.is_empty() {
        return;
    }
    if state.interactive {
        state.dom.resolve();
    }
    let active = current(state);
    let mut scopes = std::mem::take(&mut state.focus_scopes.0);
    let mut removed = Vec::new();
    scopes.retain_mut(|open| {
        if let Some((_, config)) = declarations.iter().find(|(id, _)| *id == open.id) {
            open.config = config.clone();
            true
        } else {
            removed.push(open.clone());
            false
        }
    });
    for index in 0..removed.len() {
        let old = removed[index].clone();
        for live in scopes.iter_mut().chain(removed.iter_mut()) {
            if live.previous_scope == Some(old.id) {
                live.previous = old.previous;
                live.previous_scope = old.previous_scope;
            }
        }
    }
    let mut target = removed
        .iter()
        .rev()
        .filter(|open| open.config.restore_focus && open.focused)
        .flat_map(|open| {
            [
                open.config
                    .return_focus
                    .as_ref()
                    .and_then(|name| state.mapper.tree().named(name))
                    .and_then(|id| display::focus_target(state, id)),
                open.previous,
            ]
        })
        .flatten()
        .find(|target| eligible(state, *target));
    for (id, config) in declarations {
        if scopes.iter().any(|open| open.id == id) {
            continue;
        }
        let previous = target.or(active);
        let previous_scope = scopes
            .iter()
            .rev()
            .find(|open| previous.is_some_and(|target| inside(state, target.owner, open.id)))
            .map(|open| open.id);
        let open = Open {
            id,
            config,
            previous,
            previous_scope,
            focused: false,
        };
        if state.interactive
            && open.config.auto_focus
            && state.layers.allowed(state.mapper.tree(), id)
        {
            target = Some(first(state, &open));
        }
        let index = scopes
            .iter()
            .position(|old| layers::inside(state.mapper.tree(), old.id, id))
            .unwrap_or(scopes.len());
        scopes.insert(index, open);
    }
    state.focus_scopes.0 = scopes;
    if state.interactive {
        if let Some(target) = target {
            focus(state, target);
        }
        repair(state);
    }
}
pub(super) fn repair(state: &mut State) {
    if !state.interactive {
        return;
    }
    let active = current(state);
    let scope = state
        .focus_scopes
        .0
        .iter()
        .rev()
        .find(|open| open.config.contain_focus && live(state, open))
        .cloned();
    if let Some(open) = scope
        && (open.focused || active.map(|target| target.owner) != state.focused)
        && !active
            .is_some_and(|target| inside(state, target.owner, open.id) && eligible(state, target))
    {
        focus(state, first(state, &open));
    }
    let active = current(state);
    let focused = state
        .focus_scopes
        .0
        .iter()
        .map(|open| active.is_some_and(|target| inside(state, target.owner, open.id)))
        .collect::<Vec<_>>();
    for (open, focused) in state.focus_scopes.0.iter_mut().zip(focused) {
        open.focused = focused;
    }
}
#[cfg(feature = "shell")]
pub(super) fn prevent_pointer_focus(
    state: &State,
    target: EngineId,
    event: &blitz_traits::events::DomEventData,
) -> bool {
    matches!(event, blitz_traits::events::DomEventData::PointerDown(_))
        && state
            .dom
            .owner(target)
            .is_some_and(|id| !allowed(state, id))
        && state
            .dom
            .doc
            .get_node(target)
            .is_some_and(|node| node.is_focussable())
}
#[cfg(feature = "shell")]
pub(super) fn receive(
    state: &mut State,
    target: EngineId,
    event: &blitz_traits::events::DomEventData,
) -> bool {
    use blitz_traits::events::DomEventData;
    use keyboard_types::{Key, Modifiers, NamedKey};
    let DomEventData::KeyDown(key) = event else {
        return false;
    };
    if key.key != Key::Named(NamedKey::Tab)
        || key.is_composing
        || !(key.modifiers & !Modifiers::SHIFT).is_empty()
    {
        return false;
    }
    let composing = state
        .dom
        .doc
        .get_node(target)
        .and_then(|node| node.element_data())
        .and_then(|element| element.text_input_data())
        .is_some_and(|input| input.editor.is_composing());
    if composing {
        return false;
    }
    let active = current(state);
    let Some(open) = state
        .focus_scopes
        .0
        .iter()
        .rev()
        .find(|open| {
            live(state, open) && active.is_some_and(|target| inside(state, target.owner, open.id))
        })
        .cloned()
    else {
        return false;
    };
    // Anchored popups retain their own keyboard protocol.
    if active.is_some_and(|target| !layers::inside(state.mapper.tree(), target.owner, open.id)) {
        return false;
    }
    let values = candidates(state, open.id);
    let position = values.iter().position(|id| Some(*id) == active);
    let backwards = key.modifiers.contains(Modifiers::SHIFT);
    let next = match (position, backwards) {
        (Some(index), false) if index + 1 < values.len() => Some(index + 1),
        (Some(index), true) if index > 0 => Some(index - 1),
        (_, true) if open.config.loop_tab && !values.is_empty() => Some(values.len() - 1),
        (_, false) if open.config.loop_tab && !values.is_empty() => Some(0),
        _ => None,
    };
    if let Some(index) = next {
        focus(state, values[index]);
        true
    } else if open.config.contain_focus {
        if values.is_empty() {
            focus(
                state,
                display::focus_target(state, open.id).expect("focus scope control"),
            );
        }
        true
    } else {
        false
    }
}
