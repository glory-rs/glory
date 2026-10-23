//! Native rich editing gestures. Only finite semantic document events escape.
use super::*;
use glory_render::rich_text::{Command, Selection};
fn emit(
    state: &mut State,
    id: NodeId,
    command: Option<Command>,
    commit: bool,
) -> Result<(), Error> {
    let revision = state.dom.range(id)?.editing.revision;
    let Event::RichTextChanged {
        document,
        selection,
        ..
    } = state.dom.rich_event(id, false, revision)?
    else {
        unreachable!()
    };
    let event = if let Some(command) = command {
        Event::RichTextCommand {
            document,
            selection,
            revision,
            command,
        }
    } else if commit {
        Event::RichTextCommit {
            document,
            selection,
            revision,
        }
    } else {
        Event::RichTextChanged {
            document,
            selection,
            revision,
            composing: false,
        }
    };
    state.inputs.push(Input::Event { node: id, event });
    Ok(())
}
pub(super) fn event(
    state: &mut State,
    id: NodeId,
    target: EngineId,
    event: &DomEventData,
) -> Result<bool, Error> {
    if state.dom.range(id)?.edit != EditKind::RichText {
        return Ok(false);
    }
    let readonly = matches!(&state.mapper.tree().node(id)?.descriptor.props,glory_render::Props::RichText(p)if p.read_only);
    if readonly && matches!(event, DomEventData::Ime(_)) {
        return Ok(true);
    }
    if state
        .dom
        .range(id)?
        .rich
        .as_ref()
        .unwrap()
        .tasks
        .contains_key(&target)
    {
        if matches!(event, DomEventData::Click(_)) {
            if !readonly {
                let rich = state.dom.range(id)?.rich.as_ref().unwrap();
                let block = rich.tasks[&target];
                let mut document = rich.document.clone();
                if let glory_render::rich_text::Kind::Item(Some(value)) =
                    &mut document.root.find_mut(block).unwrap().kind
                {
                    *value = !*value;
                }
                state.dom.rich_write(id, &document)?;
                state.dom.resolve();
                emit(state, id, None, false)?;
            }
            return Ok(true);
        }
        if matches!(
            event,
            DomEventData::PointerDown(_) | DomEventData::PointerUp(_)
        ) {
            return Ok(false);
        }
    }
    match event {
        DomEventData::PointerDown(pointer) => {
            if !pointer.is_primary || pointer.button != blitz_traits::events::MouseEventButton::Main
            {
                return Ok(false);
            }
            state.end_editing(id)?;
            if let Some(point) = state
                .dom
                .rich_point(id, pointer.client_x(), pointer.client_y())
            {
                let selection = Selection {
                    anchor: point,
                    focus: point,
                };
                state.dom.rich_set_selection(id, selection)?;
                state
                    .dom
                    .ranges
                    .get_mut(&id)
                    .unwrap()
                    .rich
                    .as_mut()
                    .unwrap()
                    .drag = Some(point);
                state.dom.rich_drag = Some((id, pointer.id));
                return Ok(true);
            }
        }
        DomEventData::PointerMove(pointer) => {
            let anchor = state.dom.range(id)?.rich.as_ref().unwrap().drag;
            if let Some(anchor) = anchor {
                if let Some(focus) =
                    state
                        .dom
                        .rich_point(id, pointer.client_x(), pointer.client_y())
                {
                    state
                        .dom
                        .rich_set_selection(id, Selection { anchor, focus })?;
                }
                return Ok(true);
            }
        }
        DomEventData::PointerUp(_) | DomEventData::PointerCancel(_) => {
            state
                .dom
                .ranges
                .get_mut(&id)
                .unwrap()
                .rich
                .as_mut()
                .unwrap()
                .drag = None;
            state.dom.rich_drag = None;
        }
        DomEventData::KeyDown(key) => {
            if key.key.to_string() == "Tab" {
                state.end_editing(id)?;
            }
            if key.is_composing || state.dom.range(id)?.editing.composing {
                return Ok(false);
            }
            let chord = key
                .modifiers
                .intersects(Modifiers::CONTROL | Modifiers::META);
            let shift = key.modifiers.contains(Modifiers::SHIFT);
            let alt = key.modifiers.contains(Modifiers::ALT);
            let name = key.key.to_string();
            if matches!(
                name.as_str(),
                "ArrowLeft"
                    | "ArrowRight"
                    | "ArrowUp"
                    | "ArrowDown"
                    | "Home"
                    | "End"
                    | "PageUp"
                    | "PageDown"
            ) {
                state
                    .dom
                    .ranges
                    .get_mut(&id)
                    .unwrap()
                    .rich
                    .as_mut()
                    .unwrap()
                    .typing = None;
            }
            if let Some(command) = Command::shortcut(&name, chord, shift, alt) {
                if !readonly {
                    emit(state, id, Some(command), false)?;
                }
                return Ok(true);
            }
            if chord && !alt && !shift {
                match name.to_ascii_lowercase().as_str() {
                    "a" => {
                        let selection = state.dom.rich_all_selection(id)?;
                        state.dom.rich_set_selection(id, selection)?;
                        return Ok(true);
                    }
                    "c" | "x" => {
                        let value = state.dom.rich_selected_text(id)?;
                        state
                            .dom
                            .doc
                            .shell_provider
                            .set_clipboard_text(value)
                            .map_err(|_| Error::Unsupported("native rich clipboard unavailable"))?;
                        if name.eq_ignore_ascii_case("x") && !readonly {
                            let selection = state.dom.rich_selection(id)?;
                            state.dom.rich_replace(id, selection, "")?;
                            emit(state, id, None, false)?;
                        }
                        return Ok(true);
                    }
                    "v" => {
                        if !readonly {
                            let value = state.dom.doc.shell_provider.get_clipboard_text().map_err(
                                |_| Error::Unsupported("native rich clipboard unavailable"),
                            )?;
                            if value.len() > glory_render::rich_text::MAX_BYTES {
                                return Err(Error::InvalidProps("rich paste exceeds budget"));
                            }
                            let value = normalize_text(&value, true);
                            let selection = state.dom.rich_selection(id)?;
                            state.dom.rich_replace(id, selection, &value)?;
                            emit(state, id, None, false)?;
                        }
                        return Ok(true);
                    }
                    _ => {}
                }
            }
            if name == "Enter" {
                if !readonly {
                    if chord {
                        emit(state, id, None, true)?;
                    } else if !shift {
                        state.dom.rich_split(id)?;
                        emit(state, id, None, false)?;
                    } else {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            if matches!(
                name.as_str(),
                "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight"
            ) && !chord
                && !alt
            {
                let current = state.dom.rich_selection(id)?;
                let total = state
                    .dom
                    .range(id)?
                    .rich
                    .as_ref()
                    .unwrap()
                    .document
                    .root
                    .find(current.focus.block)
                    .unwrap()
                    .units();
                if (matches!(name.as_str(), "ArrowUp" | "ArrowLeft") && current.focus.offset == 0)
                    || (matches!(name.as_str(), "ArrowDown" | "ArrowRight")
                        && current.focus.offset == total)
                {
                    if let Some(focus) = state
                        .dom
                        .rich_adjacent(id, matches!(name.as_str(), "ArrowDown" | "ArrowRight"))?
                    {
                        state.dom.rich_set_selection(
                            id,
                            Selection {
                                anchor: if shift { current.anchor } else { focus },
                                focus,
                            },
                        )?;
                        return Ok(true);
                    }
                }
            }
            if name == "Backspace" && !readonly && state.dom.rich_join_previous(id)? {
                emit(state, id, None, false)?;
                return Ok(true);
            }
            if readonly {
                return Ok(!matches!(
                    name.as_str(),
                    "ArrowLeft"
                        | "ArrowRight"
                        | "ArrowUp"
                        | "ArrowDown"
                        | "Home"
                        | "End"
                        | "PageUp"
                        | "PageDown"
                        | "Tab"
                        | "Escape"
                ));
            }
            let selection = state.dom.rich_selection(id)?;
            if selection.anchor.block != selection.focus.block {
                if name == "Backspace" || name == "Delete" {
                    state.dom.rich_replace(id, selection, "")?;
                    emit(state, id, None, false)?;
                    return Ok(true);
                }
                if !chord && !alt {
                    if let Key::Character(value) = &key.key {
                        state.dom.rich_replace(id, selection, value)?;
                        emit(state, id, None, false)?;
                        return Ok(true);
                    }
                }
            }
        }
        _ => {}
    }
    Ok(false)
}
