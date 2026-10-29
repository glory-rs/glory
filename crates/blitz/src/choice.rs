//! Native connections for the shared choice protocol. Components own matching,
//! active cursors and controlled selection; the host owns physical input/reveal.
use super::*;

pub(super) fn reveal(state: &mut State, id: NodeId) -> Result<(), Error> {
    let tree = state.mapper.tree();
    let owner = tree
        .node(id)?
        .descriptor
        .view()
        .semantics
        .as_ref()
        .and_then(|s| s.option.as_ref())
        .map(|s| &s.owner);
    let mut cursor = tree.node(id)?.parent;
    while let Some(parent) = cursor {
        let node = tree.node(parent)?;
        let d = node.descriptor.view();
        if d.role == Some(glory_render::Role::Listbox) {
            if d.semantic_id() != owner
                || !layers::visible(tree, parent)
                || !tree.active(id)
                || tree.inert(id)
                || !state.layers.allowed(tree, id)
            {
                return Ok(());
            }
            let boundary = state.dom.main(parent)?;
            return scroll_into_view::reveal_within(state, id, Some(boundary));
        }
        cursor = node.parent;
    }
    Ok(())
}

#[cfg(feature = "shell")]
pub(super) fn key(
    state: &mut State,
    origin: NodeId,
    target: EngineId,
    key: &blitz_traits::events::BlitzKeyEvent,
    key_name: &str,
) -> bool {
    use glory_render::{ComboboxKind, KeyboardInput};
    use keyboard_types::Modifiers;
    let tree = state.mapper.tree();
    if state.focused != Some(origin)
        || state.dom.control(origin).ok() != Some(target)
        || !tree.active(origin)
        || !tree.enabled(origin)
        || !state.layers.allowed(tree, origin)
        || !focus_scope::allowed(state, origin)
    {
        return false;
    }
    let Ok(node) = tree.node(origin) else {
        return false;
    };
    let d = node.descriptor.view();
    let editable = matches!(d.props, glory_render::Props::Input(_));
    let input = KeyboardInput {
        key: key_name,
        control: key.modifiers.contains(Modifiers::CONTROL),
        meta: key.modifiers.contains(Modifiers::META),
        alt: key.modifiers.contains(Modifiers::ALT),
        shift: key.modifiers.contains(Modifiers::SHIFT),
        repeat: key.is_auto_repeating,
        composing: key.is_composing,
        editable,
    };
    if let Some(s) = d.semantics.as_ref()
        && let Some(kind) = s.combobox
    {
        if d.readonly() {
            return false;
        }
        if state.subscriptions.contains(&(origin, EventKind::Choice)) {
            let empty = kind == ComboboxKind::Multiple
                && state
                    .dom
                    .doc
                    .get_node(target)
                    .and_then(|n| n.element_data())
                    .and_then(|e| e.text_input_data())
                    .is_some_and(|i| i.editor.raw_text().is_empty());
            let remove_last = input.choice_remove_last(kind, empty);
            #[cfg(target_os = "macos")]
            let remove_last = if input.key == "Backspace"
                && state.apple_input.take() != Some((origin, super::shell::AppleInput::EmptyDelete))
            {
                // The preceding AppKit command may have deleted the final
                // character. Only an originally empty editor removes a token.
                None
            } else {
                remove_last
            };
            if let Some(command) =
                remove_last.or_else(|| input.choice(kind, s.expanded == Some(true)))
            {
                state.inputs.push(Input::Event {
                    node: origin,
                    event: Event::Choice {
                        from: origin,
                        command,
                    },
                });
                return true;
            }
        }
        if kind == ComboboxKind::Select
            && s.typeahead
            && state
                .subscriptions
                .contains(&(origin, EventKind::Typeahead))
        {
            return typeahead(state, origin, input);
        }
    }
    // Only an explicit boundary or navigation item participates. Descendant
    // editors and ordinary buttons retain their own input/activation behavior.
    let root = if d.semantics.as_ref().is_some_and(|s| s.navigation.is_some()) {
        Some(origin)
    } else {
        tree.navigation(origin).map(|(root, _)| root)
    };
    if let Some(root) = root
        && tree.node(root).is_ok_and(|n| {
            n.descriptor
                .view()
                .semantics
                .as_ref()
                .is_some_and(|s| s.typeahead)
        })
        && state.subscriptions.contains(&(root, EventKind::Typeahead))
    {
        return typeahead(state, origin, input);
    }
    false
}

#[cfg(feature = "shell")]
pub(super) fn apple(state: &mut State, origin: NodeId, target: EngineId, command: &str) -> bool {
    if command != "deleteBackward:"
        || state.focused != Some(origin)
        || state.dom.control(origin).ok() != Some(target)
        || !state.mapper.tree().active(origin)
        || !state.mapper.tree().enabled(origin)
        || !state.layers.allowed(state.mapper.tree(), origin)
        || !focus_scope::allowed(state, origin)
        || !state.subscriptions.contains(&(origin, EventKind::Choice))
    {
        return false;
    }
    let Some(kind) = state
        .mapper
        .tree()
        .node(origin)
        .ok()
        .filter(|n| !n.descriptor.view().readonly())
        .and_then(|n| n.descriptor.view().semantics.as_ref())
        .and_then(|s| s.combobox)
    else {
        return false;
    };
    let Some(editor) = state
        .dom
        .doc
        .get_node(target)
        .and_then(|n| n.element_data())
        .and_then(|e| e.text_input_data())
    else {
        return false;
    };
    if kind != glory_render::ComboboxKind::Multiple
        || editor.editor.is_composing()
        || !editor.editor.raw_text().is_empty()
    {
        return false;
    }
    // Winit delivers doCommandBySelector before KeyboardInput. Cancel the
    // no-op deletion now, then let that physical key carry real modifiers.
    state.apple_input = Some((origin, super::shell::AppleInput::EmptyDelete));
    true
}

#[cfg(feature = "shell")]
fn typeahead(state: &mut State, origin: NodeId, input: glory_render::KeyboardInput<'_>) -> bool {
    let at_millis = state
        .keyboard_clock
        .elapsed()
        .as_millis()
        .min(u128::from(glory_render::MAX_TYPEAHEAD_MILLIS)) as u64;
    let Some(text) = input.typeahead(at_millis) else {
        return false;
    };
    state.inputs.push(Input::Event {
        node: origin,
        event: Event::Typeahead {
            from: origin,
            text: text.into(),
            at_millis,
        },
    });
    true
}

#[cfg(feature = "shell")]
pub(super) fn prevent_pointer_focus(
    state: &State,
    id: NodeId,
    event: &blitz_traits::events::DomEventData,
) -> bool {
    if !matches!(event, blitz_traits::events::DomEventData::PointerDown(pointer)
        if pointer.is_primary && pointer.button == blitz_traits::events::MouseEventButton::Main)
    {
        return false;
    }
    let mut cursor = Some(id);
    while let Some(id) = cursor {
        let Ok(node) = state.mapper.tree().node(id) else {
            return false;
        };
        if node
            .descriptor
            .view()
            .semantics
            .as_ref()
            .is_some_and(|s| s.option.is_some())
        {
            return true;
        }
        cursor = node.parent;
    }
    false
}

#[cfg(feature = "shell")]
pub(super) fn pointer(
    state: &mut State,
    id: NodeId,
    previous: Option<EngineId>,
    event: &blitz_traits::events::DomEventData,
) {
    use blitz_traits::events::DomEventData;
    use glory_render::PointerPhase;
    let (pointer, phase, related) = match event {
        DomEventData::PointerMove(pointer) => (pointer, PointerPhase::Move, None),
        DomEventData::PointerOver(pointer) => (pointer, PointerPhase::Enter, previous),
        DomEventData::PointerOut(pointer) => (
            pointer,
            PointerPhase::Leave,
            state.dom.doc.get_hover_node_id(),
        ),
        _ => return,
    };
    if !(pointer.is_mouse() || pointer.is_pen()) || !state.layers.allowed(state.mapper.tree(), id) {
        return;
    }
    let related = related
        .and_then(|engine| state.dom.owner(engine))
        .filter(|id| {
            state.mapper.tree().active(*id)
                && state.mapper.tree().node(*id).is_ok_and(|node| {
                    !matches!(node.descriptor.view().props, glory_render::Props::Group)
                })
        });
    if related == Some(id) {
        return;
    }
    state.inputs.push(Input::Event {
        node: id,
        event: Event::Pointer {
            from: id,
            related,
            phase,
        },
    });
}
