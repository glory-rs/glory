//! Bounded host observations for the shared, component-owned free-tag policy.
use super::*;
use glory_render::{
    KeyboardInput, MAX_TAG_INPUT_BYTES, MAX_TAG_PASTE_BYTES, TagHostRejection, TagKey,
    TagSeparators, TagsCommand,
};

fn editor(state: &State, target: EngineId) -> Option<&blitz_dom::node::TextInputData> {
    state
        .dom
        .doc
        .get_node(target)?
        .element_data()?
        .text_input_data()
}

fn policy(state: &State, id: NodeId, target: EngineId) -> Option<TagSeparators> {
    let tree = state.mapper.tree();
    if state.focused != Some(id)
        || state.dom.control(id).ok() != Some(target)
        || !tree.active(id)
        || !tree.enabled(id)
        || tree.inert(id)
        || !state.layers.allowed(tree, id)
        || !focus_scope::allowed(state, id)
        || !state.subscriptions.contains(&(id, EventKind::Tags))
    {
        return None;
    }
    let descriptor = tree.node(id).ok()?.descriptor.view();
    if descriptor.readonly() {
        return None;
    }
    let editor = editor(state, target)?;
    if editor.editor.is_composing() || state.dom.range(id).ok()?.editing.composing {
        return None;
    }
    descriptor.semantics.as_ref()?.tags_input
}

fn queue(state: &mut State, id: NodeId, command: TagsCommand) {
    state.inputs.push(Input::Event {
        node: id,
        event: Event::Tags {
            from: id,
            revision: state
                .dom
                .range(id)
                .expect("live tag editor")
                .editing
                .revision,
            command,
        },
    });
}

fn commit(state: &mut State, id: NodeId, target: EngineId) {
    let editor = editor(state, target).expect("validated tag editor");
    let raw = editor.editor.raw_text();
    let command = if raw.len() > MAX_TAG_INPUT_BYTES {
        TagsCommand::Reject(TagHostRejection::InputBudget)
    } else {
        let value = normalize_text(raw, false);
        if value.len() > MAX_TAG_INPUT_BYTES {
            TagsCommand::Reject(TagHostRejection::InputBudget)
        } else {
            TagsCommand::Commit {
                value: value.into_owned(),
            }
        }
    };
    queue(state, id, command);
}

fn paste(state: &mut State, id: NodeId, target: EngineId, separators: TagSeparators) {
    // The provider owns the OS read/allocation. Bound it before normalization,
    // event copying or editing; do not read a second clipboard snapshot.
    let Ok(raw) = state.dom.doc.shell_provider.get_clipboard_text() else {
        return;
    };
    paste_text(state, id, target, separators, &raw);
}

pub(in crate::document) fn paste_text(
    state: &mut State,
    id: NodeId,
    target: EngineId,
    separators: TagSeparators,
    raw: &str,
) {
    if raw.len() > MAX_TAG_PASTE_BYTES {
        queue(
            state,
            id,
            TagsCommand::Reject(TagHostRejection::ClipboardBudget),
        );
        return;
    }
    let text = normalize_text(raw, true);
    if text.len() > MAX_TAG_PASTE_BYTES {
        queue(
            state,
            id,
            TagsCommand::Reject(TagHostRejection::ClipboardBudget),
        );
        return;
    }
    let editor = editor(state, target).expect("validated tag editor");
    let raw_value = editor.editor.raw_text();
    if raw_value.len() > MAX_TAG_INPUT_BYTES {
        queue(
            state,
            id,
            TagsCommand::Reject(TagHostRejection::InputBudget),
        );
        return;
    }
    let selection = editor.editor.raw_selection().text_range();
    let value = normalize_text(raw_value, false);
    if value.len() > MAX_TAG_INPUT_BYTES {
        queue(
            state,
            id,
            TagsCommand::Reject(TagHostRejection::InputBudget),
        );
        return;
    }
    if text.chars().any(|c| separators.contains(c)) {
        let start = normalize_text(&raw_value[..selection.start], false)
            .encode_utf16()
            .count() as u32;
        let end = normalize_text(&raw_value[..selection.end], false)
            .encode_utf16()
            .count() as u32;
        queue(
            state,
            id,
            TagsCommand::Paste {
                value: value.into_owned(),
                text: text.into_owned(),
                start,
                end,
            },
        );
        return;
    }
    if value.len() - normalize_text(&raw_value[selection], false).len() + text.len()
        > MAX_TAG_INPUT_BYTES
    {
        queue(
            state,
            id,
            TagsCommand::Reject(TagHostRejection::InputBudget),
        );
        return;
    }
    state.dom.doc.with_text_input(target, |mut driver| {
        driver.insert_or_replace_selection(&text)
    });
    match editing::current_event(state, id) {
        Ok(event) => state.inputs.push(Input::Event { node: id, event }),
        Err(error) => state.input_error = Some(error),
    }
    state.dom.invalidate();
}

pub(super) fn key(
    state: &mut State,
    id: NodeId,
    target: EngineId,
    key: &BlitzKeyEvent,
    name: &str,
) -> bool {
    if key.is_composing {
        return false;
    }
    let action_mod = if cfg!(any(target_os = "macos", target_os = "ios")) {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    let paste_key = name.eq_ignore_ascii_case("v")
        && (key.modifiers == action_mod || key.modifiers == (action_mod | Modifiers::SHIFT))
        || name == "Paste" && key.modifiers.is_empty()
        || name == "Insert" && key.modifiers == Modifiers::SHIFT;
    // The native action already ran. Its raw key remains consumed even if a
    // component callback removed the tag policy before this key arrived.
    if paste_key && state.apple_input == Some((id, AppleInput::Paste)) {
        state.apple_input = None;
        return true;
    }
    let Some(separators) = policy(state, id, target) else {
        return false;
    };
    if paste_key {
        paste(state, id, target, separators);
        return true;
    }
    let empty = editor(state, target)
        .expect("validated tag editor")
        .editor
        .raw_text()
        .is_empty();
    let command = separators.key(
        KeyboardInput {
            key: name,
            control: key.modifiers.contains(Modifiers::CONTROL),
            meta: key.modifiers.contains(Modifiers::META),
            alt: key.modifiers.contains(Modifiers::ALT),
            shift: key.modifiers.contains(Modifiers::SHIFT),
            repeat: key.is_auto_repeating,
            composing: key.is_composing,
            editable: true,
        },
        empty,
    );
    #[cfg(target_os = "macos")]
    if command == Some(TagKey::RemoveLast)
        && state.apple_input.take() != Some((id, AppleInput::EmptyDelete))
    {
        return false;
    }
    match command {
        Some(TagKey::Commit) => commit(state, id, target),
        Some(TagKey::RemoveLast) => queue(state, id, TagsCommand::RemoveLast),
        None => return false,
    }
    true
}

pub(super) fn apple(state: &mut State, id: NodeId, target: EngineId, command: &str) -> bool {
    let Some(separators) = policy(state, id, target) else {
        return false;
    };
    match command {
        "deleteBackward:"
            if editor(state, target)
                .expect("validated tag editor")
                .editor
                .raw_text()
                .is_empty() =>
        {
            state.apple_input = Some((id, AppleInput::EmptyDelete));
        }
        // The following physical Enter supplies modifiers/repeat and owns the
        // tag-vs-form decision; do not submit the engine form ahead of it.
        "insertNewline:" | "insertNewlineIgnoringFieldEditor:" => {}
        "paste:" => {
            paste(state, id, target, separators);
            state.apple_input = Some((id, AppleInput::Paste));
        }
        _ => return false,
    }
    true
}

pub(super) fn software_enter(state: &State) -> bool {
    state.dom.doc.get_focussed_node_id().is_some_and(|target| {
        state.dom.owner(target).is_some_and(|id| {
            policy(state, id, target).is_some()
                && state
                    .dom
                    .range(id)
                    .is_ok_and(|r| r.editing.after_preedit.is_none())
        })
    })
}

pub(super) fn ime(state: &mut State, id: NodeId, target: EngineId, event: &BlitzImeEvent) -> bool {
    let BlitzImeEvent::Commit(text) = event else {
        return false;
    };
    let Some(separators) = policy(state, id, target) else {
        return false;
    };
    if state
        .dom
        .range(id)
        .is_ok_and(|r| r.editing.after_preedit.is_some())
        || text.len() != 1
        || !text.chars().any(|c| separators.contains(c))
    {
        return false;
    }
    commit(state, id, target);
    state.dom.doc.shell_provider.invalidate_ime_context();
    true
}
