//! Native editing protocol, isolated from the window/event-loop implementation.
use super::*;

fn multiline(state: &State, node: NodeId) -> bool {
    state
        .mapper
        .tree()
        .node(node)
        .is_ok_and(|node| matches!(&node.descriptor.view().props, glory_render::Props::Input(input) if input.multiline))
}

pub(super) fn sanitize_ime(state: &State, event: &mut UiEvent) {
    let Some(node) = state
        .dom
        .doc
        .get_focussed_node_id()
        .and_then(|node| state.dom.owner(node))
    else {
        return;
    };
    let multiline = multiline(state, node);
    match event {
        UiEvent::Ime(ApaceImeEvent::Preedit(text, cursor)) => {
            *cursor = cursor
                .filter(|(start, end)| {
                    start <= end && text.is_char_boundary(*start) && text.is_char_boundary(*end)
                })
                .map(|(start, end)| {
                    (
                        normalize_text(&text[..start], multiline).len(),
                        normalize_text(&text[..end], multiline).len(),
                    )
                });
            if let Cow::Owned(value) = normalize_text(text, multiline) {
                *text = value;
            }
        }
        UiEvent::Ime(ApaceImeEvent::Commit(text)) => {
            if let Cow::Owned(value) = normalize_text(text, multiline) {
                *text = value;
            }
        }
        _ => {}
    }
}

pub(super) fn text_event(state: &mut State, node: NodeId, text: &str) -> Result<Event, Error> {
    #[cfg(feature = "clipboard")]
    clipboard_io::editor::cancel(state);
    let capture = state
        .mapper
        .tree()
        .node(node)?
        .descriptor
        .view()
        .semantics
        .as_ref()
        .and_then(|s| s.input_edit);
    if capture.is_some_and(|c| text.len() > c.max_bytes) {
        return Ok(Event::InputEdit(glory_render::InputEdit::Rejected {
            revision: state.dom.range(node)?.editing.revision,
            reason: glory_render::InputEditRejection::TooLong,
        }));
    }
    let multiline = multiline(state, node);
    let value = normalize_text(text, multiline);
    if let Cow::Owned(value) = &value {
        let control = state.dom.control(node)?;
        let mut selection = None;
        state.dom.doc.with_text_input(control, |driver| {
            let selection_raw = driver.editor.raw_selection();
            let text = driver.editor.raw_text();
            selection = Some((
                normalize_text(&text[..selection_raw.anchor().index()], multiline).len(),
                normalize_text(&text[..selection_raw.focus().index()], multiline).len(),
            ));
        });
        state
            .dom
            .doc
            .mutate()
            .set_attribute(control, attr("value"), value);
        state.dom.property_writes += 1;
        if let Some((anchor, focus)) = selection {
            state.dom.doc.with_text_input(control, |mut driver| {
                driver.select_byte_range(anchor, focus)
            });
        }
    }
    if let Some(capture) = capture {
        let range = state
            .dom
            .ranges
            .get_mut(&node)
            .ok_or(Error::NodeGone(node))?;
        let revision = range.editing.revision;
        let composing = range.editing.composing;
        let before = range
            .editing
            .input_before
            .take()
            .filter(|(r, _, _)| *r == revision)
            .map(|(_, s, i)| (s, i));
        if value.len() > capture.max_bytes {
            return Ok(Event::InputEdit(glory_render::InputEdit::Rejected {
                revision,
                reason: glory_render::InputEditRejection::TooLong,
            }));
        }
        let Some(selection) = capture_selection(state, node, capture.max_bytes) else {
            return Ok(Event::InputEdit(glory_render::InputEdit::Rejected {
                revision,
                reason: glory_render::InputEditRejection::SelectionUnavailable,
            }));
        };
        let (before, intent) = before
            .map_or((None, glory_render::InputEditIntent::Replace), |(s, i)| {
                (Some(s), i)
            });
        return Ok(Event::InputEdit(glory_render::InputEdit::Value {
            value: value.into_owned(),
            revision,
            composing,
            selection,
            before,
            intent,
        }));
    }
    let range = state.dom.range(node)?;
    Ok(Event::ValueChanged {
        value: value.into_owned(),
        composing: range.editing.composing,
        revision: range.editing.revision,
    })
}
fn capture_selection(
    state: &State,
    node: NodeId,
    max: usize,
) -> Option<glory_render::TextSelection> {
    let control = state.dom.control(node).ok()?;
    let input = state
        .dom
        .doc
        .get_node(control)?
        .element_data()?
        .text_input_data()?;
    let text = input.editor.raw_text();
    if text.len() > max {
        return None;
    }
    let selection = input.editor.raw_selection();
    let a = selection.anchor().index();
    let f = selection.focus().index();
    let start = text.get(..a.min(f))?.encode_utf16().count();
    let end = text.get(..a.max(f))?.encode_utf16().count();
    Some(glory_render::TextSelection {
        start,
        end,
        direction: if a > f {
            glory_render::SelectionDirection::Backward
        } else if a < f {
            glory_render::SelectionDirection::Forward
        } else {
            glory_render::SelectionDirection::None
        },
    })
}
pub(super) fn capture_before(state: &mut State, event: &UiEvent) {
    let intent = match event {
        UiEvent::KeyDown(key) => match key.key {
            Key::Named(NamedKey::Backspace) => glory_render::InputEditIntent::DeleteBackward,
            Key::Named(NamedKey::Delete) => glory_render::InputEditIntent::DeleteForward,
            Key::Character(_)
                if !key
                    .modifiers
                    .intersects(Modifiers::CONTROL | Modifiers::META) =>
            {
                glory_render::InputEditIntent::Insert
            }
            _ => glory_render::InputEditIntent::Replace,
        },
        UiEvent::AppleStandardKeybinding(command) => match command.as_str() {
            "deleteBackward:"
            | "deleteWordBackward:"
            | "deleteToBeginningOfLine:"
            | "deleteToBeginningOfParagraph:"
            | "deleteBackwardByDecomposingPreviousCharacter:" => {
                glory_render::InputEditIntent::DeleteBackward
            }
            "deleteForward:"
            | "deleteWordForward:"
            | "deleteToEndOfLine:"
            | "deleteToEndOfParagraph:" => glory_render::InputEditIntent::DeleteForward,
            _ => glory_render::InputEditIntent::Replace,
        },
        _ => return,
    };
    let Some(node) = state
        .dom
        .doc
        .get_focussed_node_id()
        .and_then(|id| state.dom.owner(id))
    else {
        return;
    };
    let Some(capture) = state.mapper.tree().node(node).ok().and_then(|n| {
        n.descriptor
            .view()
            .semantics
            .as_ref()
            .and_then(|s| s.input_edit)
    }) else {
        return;
    };
    let before = capture_selection(state, node, capture.max_bytes);
    if let Some(range) = state.dom.ranges.get_mut(&node) {
        range.editing.input_before = before.map(|s| (range.editing.revision, s, intent));
    }
}
pub(in crate::document) fn current_event(state: &mut State, node: NodeId) -> Result<Event, Error> {
    if let Some(max) = state
        .mapper
        .tree()
        .node(node)?
        .descriptor
        .view()
        .semantics
        .as_ref()
        .and_then(|s| s.input_edit)
        .map(|c| c.max_bytes)
    {
        let control = state.dom.control(node)?;
        let len = state
            .dom
            .doc
            .get_node(control)
            .and_then(|n| n.element_data())
            .and_then(|e| e.text_input_data())
            .map(|i| i.editor.raw_text().len())
            .unwrap_or(0);
        if len > max {
            return Ok(Event::InputEdit(glory_render::InputEdit::Rejected {
                revision: state.dom.range(node)?.editing.revision,
                reason: glory_render::InputEditRejection::TooLong,
            }));
        }
    }
    let value = state.dom.text_value(node)?;
    text_event(state, node, &value)
}
#[cfg(feature = "clipboard")]
pub(in crate::document) fn capture_clipboard(state: &mut State, node: NodeId) {
    let before = state
        .mapper
        .tree()
        .node(node)
        .ok()
        .and_then(|n| {
            n.descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|s| s.input_edit)
        })
        .and_then(|capture| capture_selection(state, node, capture.max_bytes));
    if let Some(range) = state.dom.ranges.get_mut(&node) {
        range.editing.input_before = before.map(|selection| {
            (
                range.editing.revision,
                selection,
                glory_render::InputEditIntent::Replace,
            )
        });
    }
}
pub(super) fn queue_commit(state: &mut State, node: NodeId, sync_value: bool) -> Result<(), Error> {
    let revision = state.dom.range(node)?.editing.revision;
    if state
        .mapper
        .tree()
        .node(node)?
        .descriptor
        .view()
        .semantics
        .as_ref()
        .is_some_and(|s| s.input_edit.is_some())
    {
        let event = current_event(state, node)?;
        let value = match &event {
            Event::InputEdit(glory_render::InputEdit::Value { value, .. }) => Some(value.clone()),
            _ => None,
        };
        state.inputs.push(Input::Event { node, event });
        if let Some(value) = value {
            state.inputs.push(Input::Event {
                node,
                event: Event::Commit { value, revision },
            });
        }
    } else {
        let value = state.dom.text_value(node)?;
        if sync_value {
            state.inputs.push(Input::Event {
                node,
                event: Event::ValueChanged {
                    value: value.clone(),
                    revision,
                    composing: false,
                },
            });
        }
        if !multiline(state, node) {
            state.inputs.push(Input::Event {
                node,
                event: Event::Commit { value, revision },
            });
        }
    }
    Ok(())
}

// IME preedit offsets are UTF-8 bytes (the native protocol). Query::Selection
// converts to UTF-16 separately. Never slice text at unvalidated IME boundaries.
pub(super) fn ime_event(
    state: &mut State,
    node: NodeId,
    event: &ApaceImeEvent,
) -> Result<(), Error> {
    use std::num::NonZeroUsize;
    let control = state.dom.control(node)?;
    state.dom.resolve();
    if !state
        .dom
        .doc
        .get_node(control)
        .and_then(|node| node.element_data())
        .is_some_and(|element| element.text_input_data().is_some())
    {
        return Err(Error::Unsupported("Apace text editor is not initialized"));
    }
    match event {
        ApaceImeEvent::Enabled => {}
        ApaceImeEvent::Preedit(text, cursor) if !text.is_empty() => {
            let cursor = cursor.filter(|(start, end)| {
                start <= end && text.is_char_boundary(*start) && text.is_char_boundary(*end)
            });
            let original = if !state.dom.range(node)?.editing.composing {
                let input = state
                    .dom
                    .doc
                    .get_node(control)
                    .and_then(|node| node.element_data())
                    .and_then(|element| element.text_input_data())
                    .expect("validated editor");
                let selection = input.editor.raw_selection();
                Some((
                    input.editor.raw_text()[selection.text_range()].to_owned(),
                    selection.anchor().index() > selection.focus().index(),
                ))
            } else {
                None
            };
            let editing = &mut state.dom.ranges.get_mut(&node).expect("live text").editing;
            editing.after_preedit = None;
            if let Some(original) = original {
                editing.replacement = Some(original);
            }
            if !std::mem::replace(&mut editing.composing, true) {
                state.inputs.push(Input::Event {
                    node,
                    event: Event::Composition { active: true },
                });
            }
            state
                .dom
                .doc
                .with_text_input(control, |mut driver| driver.set_compose(text, cursor));
            native_value(state, node)?;
        }
        ApaceImeEvent::Preedit(_, _) => {
            // Empty preedit may mean cancel, or precede Commit in a later OS
            // event/turn. Complete now; a differing program write supersedes
            // the trailing commit instead of inserting it into the new value.
            state
                .dom
                .ranges
                .get_mut(&node)
                .expect("live text")
                .editing
                .after_preedit
                .get_or_insert(false);
            if let Some((value, revision)) = state.dom.finish_composition(node, false)? {
                state.inputs.push(Input::Event {
                    node,
                    event: Event::Composition { active: false },
                });
                let event = state.terminal_input_event(node, value, revision)?;
                state.inputs.push(Input::Event { node, event });
            }
        }
        ApaceImeEvent::Commit(text) => {
            let superseded = state
                .dom
                .ranges
                .get_mut(&node)
                .expect("live text")
                .editing
                .after_preedit
                .take()
                .unwrap_or(false);
            if !superseded {
                // Clear the preedit before replacing; direct Commit without
                // empty Preedit must replace the whole composing range too.
                state.dom.doc.with_text_input(control, |mut driver| {
                    driver.clear_compose();
                    driver.insert_or_replace_selection(text);
                });
                let revision = state.dom.range(node)?.editing.revision;
                let value = state.captured_text_value(node)?;
                if state.dom.finish_composition(node, true)?.is_some() {
                    state.inputs.push(Input::Event {
                        node,
                        event: Event::Composition { active: false },
                    });
                }
                let event = state.terminal_input_event(node, value, revision)?;
                state.inputs.push(Input::Event { node, event });
            }
        }
        ApaceImeEvent::Disabled => {
            state.end_editing(node)?;
        }
        ApaceImeEvent::DeleteSurrounding {
            before_bytes,
            after_bytes,
        } => {
            // Parley's public driver safely clamps byte boundaries. Preserve
            // preedit independently, as required by the native IME protocol.
            state.dom.doc.with_text_input(control, |mut driver| {
                let preedit = driver
                    .editor
                    .raw_compose()
                    .as_ref()
                    .map(|range| driver.editor.raw_text()[range.clone()].to_owned());
                let selection = driver.editor.raw_selection().text_range();
                let compose = driver.editor.raw_compose();
                let relative = compose
                    .as_ref()
                    .map(|range| {
                        (
                            selection.start.saturating_sub(range.start),
                            selection.end.saturating_sub(range.start),
                        )
                    })
                    .filter(|(start, end)| {
                        preedit.as_ref().is_some_and(|text| {
                            start <= end
                                && text.is_char_boundary(*start)
                                && text.is_char_boundary(*end)
                        })
                    });
                driver.clear_compose();
                if let Some(bytes) = NonZeroUsize::new(*before_bytes) {
                    driver.delete_bytes_before_selection(bytes);
                }
                if let Some(bytes) = NonZeroUsize::new(*after_bytes) {
                    driver.delete_bytes_after_selection(bytes);
                }
                if let Some(preedit) = preedit {
                    driver.set_compose(&preedit, relative);
                }
            });
            native_value(state, node)?;
        }
    }
    state.dom.invalidate();
    Ok(())
}

fn native_value(state: &mut State, node: NodeId) -> Result<(), Error> {
    let event = current_event(state, node)?;
    state.inputs.push(Input::Event { node, event });
    Ok(())
}
