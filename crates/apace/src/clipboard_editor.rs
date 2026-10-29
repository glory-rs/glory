//! One current editor receipt; no clipboard history or editor text snapshot.
use super::super::shell::{AppleInput, editing, tags};
use super::*;
use apace_traits::events::{ApaceKeyEvent, UiEvent};
use glory_render::{Props, Purpose, TagSeparators};
use keyboard_types::Modifiers;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Copy,
    Cut,
    Paste,
}
#[derive(Clone, Copy)]
pub(super) struct Lease {
    node: NodeId,
    target: EngineId,
    command: Command,
    revision: u64,
    anchor: usize,
    focus: usize,
    purpose: Purpose,
    tags: Option<TagSeparators>,
}
fn input(state: &State, target: EngineId) -> Option<&apace_dom::node::TextInputData> {
    state
        .dom
        .doc
        .get_node(target)?
        .element_data()?
        .text_input_data()
}
fn lease(state: &State, node: NodeId, target: EngineId, command: Command) -> Option<Lease> {
    let tree = state.mapper.tree();
    if state.focused != Some(node)
        || state.dom.doc.get_focussed_node_id() != Some(target)
        || state.dom.control(node).ok() != Some(target)
        || !tree.active(node)
        || !tree.enabled(node)
        || tree.inert(node)
        || !state.layers.allowed(tree, node)
        || !focus_scope::allowed(state, node)
    {
        return None;
    }
    let descriptor = tree.node(node).ok()?.descriptor.view();
    let Props::Input(props) = descriptor.props else {
        return None;
    };
    if descriptor.readonly() && command != Command::Copy
        || props.purpose == Purpose::Password && command != Command::Paste
    {
        return None;
    }
    let editor = input(state, target)?;
    let range = state.dom.range(node).ok()?;
    if editor.editor.is_composing() || range.editing.composing {
        return None;
    }
    let selection = editor.editor.raw_selection();
    Some(Lease {
        node,
        target,
        command,
        revision: range.editing.revision,
        anchor: selection.anchor().index(),
        focus: selection.focus().index(),
        purpose: props.purpose,
        tags: descriptor
            .semantics
            .as_ref()
            .and_then(|s| s.tags_input)
            .filter(|_| state.subscriptions.contains(&(node, EventKind::Tags))),
    })
}
pub(super) fn current(state: &State, receipt: Lease) -> bool {
    !state.clipboard.editor_cancelled
        && lease(state, receipt.node, receipt.target, receipt.command).is_some_and(|now| {
            now.revision == receipt.revision
                && now.anchor == receipt.anchor
                && now.focus == receipt.focus
                && now.purpose == receipt.purpose
                && now.tags == receipt.tags
        })
}
pub(in crate::document) fn cancel(state: &mut State) {
    // Independent of Work: foreign poll/constructor code executes without the
    // document borrow, and may invalidate a receipt while Work is taken out.
    state.clipboard.editor_cancelled = true;
}
fn key_command(key: &ApaceKeyEvent, name: &str) -> Option<Command> {
    if key.is_composing {
        return None;
    }
    let action = if cfg!(any(target_os = "macos", target_os = "ios")) {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    if key.modifiers == action {
        if name.eq_ignore_ascii_case("c") {
            return Some(Command::Copy);
        }
        if name.eq_ignore_ascii_case("x") {
            return Some(Command::Cut);
        }
        if name.eq_ignore_ascii_case("v") {
            return Some(Command::Paste);
        }
    }
    if name.eq_ignore_ascii_case("v") && key.modifiers == (action | Modifiers::SHIFT)
        || name == "Paste" && key.modifiers.is_empty()
        || name == "Insert" && key.modifiers == Modifiers::SHIFT
    {
        return Some(Command::Paste);
    }
    if name == "Copy" && key.modifiers.is_empty()
        || name == "Insert" && key.modifiers == Modifiers::CONTROL
    {
        return Some(Command::Copy);
    }
    if name == "Cut" && key.modifiers.is_empty()
        || name == "Delete" && key.modifiers == Modifiers::SHIFT
    {
        return Some(Command::Cut);
    }
    None
}
pub(in crate::document) fn before_input(state: &mut State, event: &UiEvent) {
    let invalidates = match event {
        UiEvent::KeyDown(key) => key_command(key, &key.key.to_string()).is_none(),
        UiEvent::AppleStandardKeybinding(command) => {
            !matches!(command.as_str(), "copy:" | "cut:" | "paste:")
        }
        UiEvent::Ime(_) | UiEvent::PointerDown(_) => true,
        _ => false,
    };
    if invalidates {
        cancel(state);
    }
}
fn marker(command: Command) -> AppleInput {
    match command {
        Command::Copy => AppleInput::Copy,
        Command::Cut => AppleInput::Cut,
        Command::Paste => AppleInput::Paste,
    }
}
pub(in crate::document) fn key(
    state: &mut State,
    node: NodeId,
    target: EngineId,
    key: &ApaceKeyEvent,
    name: &str,
) -> bool {
    let Some(command) = key_command(key, name) else {
        return false;
    };
    // Consume the current AppKit pair even after a callback removes the policy
    // or configuration. Never execute a clipboard operation twice.
    if state.apple_input == Some((node, marker(command))) {
        state.apple_input = None;
        return true;
    }
    begin(state, node, target, command)
}
pub(in crate::document) fn apple(
    state: &mut State,
    node: NodeId,
    target: EngineId,
    command: &str,
) -> bool {
    let command = match command {
        "copy:" => Command::Copy,
        "cut:" => Command::Cut,
        "paste:" => Command::Paste,
        _ => return false,
    };
    if begin(state, node, target, command) {
        state.apple_input = Some((node, marker(command)));
        true
    } else {
        false
    }
}
fn begin(state: &mut State, node: NodeId, target: EngineId, command: Command) -> bool {
    let Some(config) = state.clipboard.config.clone() else {
        return false;
    };
    // Configured text controls consume forbidden commands too, preventing the
    // engine's synchronous provider fallback from bypassing these guards.
    if state.focused != Some(node)
        || state.dom.control(node).ok() != Some(target)
        || !state
            .mapper
            .tree()
            .node(node)
            .is_ok_and(|n| matches!(n.descriptor.view().props, Props::Input(_)))
    {
        return false;
    }
    let Some(receipt) = lease(state, node, target, command) else {
        return true;
    };
    if state.clipboard.active.is_some() || state.clipboard.next == u64::MAX {
        return true;
    }
    let future: Pin<Box<dyn Future<Output = Completion> + Send>> = if command == Command::Paste {
        let limit = if receipt.tags.is_some() {
            glory_render::MAX_TAG_PASTE_BYTES
        } else {
            glory_render::MAX_CLIPBOARD_BYTES
        };
        Box::pin(async move { config.driver.read(limit).await.map(Payload::Text) })
    } else {
        let editor = input(state, target).expect("validated editor");
        let selection = editor.editor.raw_selection().text_range();
        if selection.is_empty() || selection.len() > glory_render::MAX_CLIPBOARD_BYTES {
            return true;
        }
        let text = editor.editor.raw_text()[selection].to_owned();
        Box::pin(async move { config.driver.write(text).await.map(|()| Payload::Written) })
    };
    state.clipboard.next += 1;
    schedule(
        state,
        state.clipboard.next,
        Consumer::Editor(receipt),
        future,
        config.timeout,
    );
    true
}
fn queue_edit(state: &mut State, node: NodeId) {
    match editing::current_event(state, node) {
        Ok(event) => state.inputs.push(Input::Event { node, event }),
        Err(error) => state.input_error = Some(error),
    }
    state.dom.doc.shell_provider.invalidate_ime_context();
    state.dom.invalidate();
}
pub(super) fn finish(state: &mut State, receipt: Lease, result: Completion) {
    let Lease {
        node,
        target,
        command,
        tags: policy,
        ..
    } = receipt;
    match (command, result) {
        (Command::Cut, Ok(Payload::Written)) => {
            editing::capture_clipboard(state, node);
            state.dom.doc.with_text_input(target, |mut driver| {
                driver.delete_selection();
            });
            queue_edit(state, node);
        }
        (Command::Paste, result) => {
            let raw = match result {
                Ok(Payload::Text(raw)) => raw,
                Err(ClipboardError::TooLong) => {
                    if policy.is_some() {
                        reject_tag(state, node);
                    }
                    return;
                }
                _ => return,
            };
            let limit = if policy.is_some() {
                glory_render::MAX_TAG_PASTE_BYTES
            } else {
                glory_render::MAX_CLIPBOARD_BYTES
            };
            if raw.len() > limit {
                if policy.is_some() {
                    reject_tag(state, node);
                }
                return;
            }
            if let Some(policy) = policy {
                editing::capture_clipboard(state, node);
                tags::paste_text(state, node, target, policy, &raw);
                state.dom.doc.shell_provider.invalidate_ime_context();
                return;
            }
            let descriptor = state
                .mapper
                .tree()
                .node(node)
                .expect("live editor")
                .descriptor
                .view();
            let Props::Input(props) = descriptor.props else {
                return;
            };
            let text = normalize_text(&raw, props.multiline);
            if text.len() > limit {
                return;
            }
            if let Some(capture) = descriptor.semantics.as_ref().and_then(|s| s.input_edit) {
                let editor = input(state, target).expect("live editor");
                let size = editor.editor.raw_text().len()
                    - editor.editor.raw_selection().text_range().len();
                if size.saturating_add(text.len()) > capture.max_bytes {
                    state.inputs.push(Input::Event {
                        node,
                        event: Event::InputEdit(glory_render::InputEdit::Rejected {
                            revision: receipt.revision,
                            reason: glory_render::InputEditRejection::TooLong,
                        }),
                    });
                    return;
                }
            }
            editing::capture_clipboard(state, node);
            state.dom.doc.with_text_input(target, |mut driver| {
                driver.insert_or_replace_selection(&text)
            });
            queue_edit(state, node);
        }
        _ => {}
    }
}
fn reject_tag(state: &mut State, node: NodeId) {
    let revision = state
        .dom
        .range(node)
        .expect("live tags editor")
        .editing
        .revision;
    state.inputs.push(Input::Event {
        node,
        event: Event::Tags {
            from: node,
            revision,
            command: glory_render::TagsCommand::Reject(
                glory_render::TagHostRejection::ClipboardBudget,
            ),
        },
    });
}
