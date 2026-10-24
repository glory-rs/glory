//! Read-only capture at the actual native edit boundary, never business reentry.
use super::{backend, dom::Entry};
use glory_render::{
    Error, Event, InputEdit, InputEditIntent, InputEditRejection, MAX_INPUT_EDIT_BYTES,
    SelectionDirection, TextSelection,
};
use wasm_bindgen::JsCast;
use web_sys::{Element, Node};
pub(super) fn budget(element: &Element) -> Option<usize> {
    element
        .get_attribute("data-gly-input-edit")?
        .parse()
        .ok()
        .filter(|n| *n > 0 && *n <= MAX_INPUT_EDIT_BYTES)
}
pub(super) fn selection(control: &Node) -> Result<TextSelection, Error> {
    let read = |name: &str| -> Result<usize, Error> {
        let v = js_sys::Reflect::get(control, &name.into())
            .map_err(backend)?
            .as_f64()
            .filter(|v| v.is_finite() && *v >= 0. && v.fract() == 0. && *v <= u32::MAX as f64)
            .ok_or(Error::Unsupported("native text selection unavailable"))?;
        Ok(v as usize)
    };
    let start = read("selectionStart")?;
    let end = read("selectionEnd")?;
    let direction = match js_sys::Reflect::get(control, &"selectionDirection".into())
        .map_err(backend)?
        .as_string()
        .as_deref()
    {
        Some("backward") => SelectionDirection::Backward,
        Some("forward") => SelectionDirection::Forward,
        _ => SelectionDirection::None,
    };
    Ok(TextSelection {
        start,
        end,
        direction,
    })
}
pub(super) fn before(
    entry: &Entry,
    element: &Element,
    event: &web_sys::Event,
) -> Result<(), Error> {
    let Some(max) = budget(element) else {
        return Ok(());
    };
    let control = entry.control()?;
    let before = selection(&control).ok().filter(|s| s.validate(max).is_ok());
    let intent = match event
        .dyn_ref::<web_sys::InputEvent>()
        .map(|e| e.input_type())
        .as_deref()
    {
        Some("deleteContentBackward") => InputEditIntent::DeleteBackward,
        Some("deleteContentForward") => InputEditIntent::DeleteForward,
        Some(t) if t.starts_with("insert") => InputEditIntent::Insert,
        _ => InputEditIntent::Replace,
    };
    let mut editing = entry.editing.borrow_mut();
    editing.input_before = before.map(|s| (editing.revision, s, intent));
    Ok(())
}
pub(super) fn capture(
    control: &Element,
    max: usize,
    revision: u64,
    composing: bool,
    before: Option<(TextSelection, InputEditIntent)>,
) -> Result<InputEdit, Error> {
    let raw = js_sys::Reflect::get(control, &"value".into()).map_err(backend)?;
    if !raw.is_string() || js_sys::JsString::from(raw.clone()).length() as usize > max {
        return Ok(InputEdit::Rejected {
            revision,
            reason: InputEditRejection::TooLong,
        });
    }
    // Conversion is bounded by max UTF-16 units; reject excess UTF-8 bytes too.
    let raw = raw.as_string().unwrap_or_default();
    if raw.len() > max {
        return Ok(InputEdit::Rejected {
            revision,
            reason: InputEditRejection::TooLong,
        });
    }
    let value = glory_render::normalize_text(&raw, false).into_owned();
    let Ok(selection) = selection(control.as_ref()) else {
        return Ok(InputEdit::Rejected {
            revision,
            reason: InputEditRejection::SelectionUnavailable,
        });
    };
    if selection.validate(value.encode_utf16().count()).is_err() {
        return Ok(InputEdit::Rejected {
            revision,
            reason: InputEditRejection::SelectionUnavailable,
        });
    }
    let (before, intent) = before.map_or((None, InputEditIntent::Replace), |(s, i)| (Some(s), i));
    Ok(InputEdit::Value {
        value,
        revision,
        selection,
        before,
        intent,
        composing,
    })
}
pub(super) fn event(
    entry: &Entry,
    element: &Element,
    composing: bool,
) -> Result<Option<Event>, Error> {
    let Some(max) = budget(element) else {
        return Ok(None);
    };
    let (revision, before) = {
        let mut edit = entry.editing.borrow_mut();
        let revision = edit.revision;
        let before = edit
            .input_before
            .take()
            .filter(|(r, _, _)| *r == revision)
            .map(|(_, s, i)| (s, i));
        (revision, before)
    };
    let control = entry.control()?;
    Ok(Some(Event::InputEdit(capture(
        &control, max, revision, composing, before,
    )?)))
}
