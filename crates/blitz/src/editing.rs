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
        UiEvent::Ime(BlitzImeEvent::Preedit(text, cursor)) => {
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
        UiEvent::Ime(BlitzImeEvent::Commit(text)) => {
            if let Cow::Owned(value) = normalize_text(text, multiline) {
                *text = value;
            }
        }
        _ => {}
    }
}

pub(super) fn text_event(state: &mut State, node: NodeId, text: &str) -> Result<Event, Error> {
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
    let range = state.dom.range(node)?;
    Ok(Event::ValueChanged {
        value: value.into_owned(),
        composing: range.editing.composing,
        revision: range.editing.revision,
    })
}

// IME preedit offsets are UTF-8 bytes (the native protocol). Query::Selection
// converts to UTF-16 separately. Never slice text at unvalidated IME boundaries.
pub(super) fn ime_event(
    state: &mut State,
    node: NodeId,
    event: &BlitzImeEvent,
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
        return Err(Error::Unsupported("Blitz text editor is not initialized"));
    }
    match event {
        BlitzImeEvent::Enabled => {}
        BlitzImeEvent::Preedit(text, cursor) if !text.is_empty() => {
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
        BlitzImeEvent::Preedit(_, _) => {
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
                state.inputs.push(Input::Event {
                    node,
                    event: Event::ValueChanged {
                        value,
                        composing: false,
                        revision,
                    },
                });
            }
        }
        BlitzImeEvent::Commit(text) => {
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
                let value = state.dom.text_value(node)?;
                if state.dom.finish_composition(node, true)?.is_some() {
                    state.inputs.push(Input::Event {
                        node,
                        event: Event::Composition { active: false },
                    });
                }
                state.inputs.push(Input::Event {
                    node,
                    event: Event::ValueChanged {
                        value,
                        composing: false,
                        revision,
                    },
                });
            }
        }
        BlitzImeEvent::Disabled => {
            state.end_editing(node)?;
        }
        BlitzImeEvent::DeleteSurrounding {
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
    let range = state.dom.range(node)?;
    let event = Event::ValueChanged {
        value: state.dom.text_value(node)?,
        composing: range.editing.composing,
        revision: range.editing.revision,
    };
    state.inputs.push(Input::Event { node, event });
    Ok(())
}
