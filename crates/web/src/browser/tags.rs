//! Host observations for an explicitly opted-in free tag editor.
use super::super::dom::{Dom, Entry};
use super::backend;
use glory_render::{
    Error, Event, EventKind, KeyboardInput, MAX_TAG_INPUT_BYTES, MAX_TAG_PASTE_BYTES, NodeId,
    TagKey, TagSeparators, TagsCommand,
};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, KeyboardEvent, Node};

fn policy(element: &Element) -> Option<TagSeparators> {
    match element.get_attribute("data-gly-tags").as_deref() {
        Some("enter") => Some(TagSeparators::Enter),
        Some("enter-comma") => Some(TagSeparators::EnterComma),
        Some("enter-comma-semicolon") => Some(TagSeparators::EnterCommaSemicolon),
        _ => None,
    }
}
fn text(value: JsValue, limit: usize) -> Result<String, Error> {
    // Inspect the JS string's UTF-16 length before copying into a Rust String.
    // Even a non-ASCII conversion is then bounded to at most 3 * limit bytes.
    let value = js_sys::JsString::from(value);
    if value.length() as usize > limit {
        return Err(Error::InvalidEvent(
            "tags host text exceeds its finite budget",
        ));
    }
    let value = value
        .as_string()
        .ok_or(Error::InvalidEvent("tags host text is not a string"))?;
    if value.len() > limit {
        return Err(Error::InvalidEvent(
            "tags host UTF-8 text exceeds its finite budget",
        ));
    }
    Ok(value)
}
fn value(entry: &Entry) -> Result<String, Error> {
    text(
        js_sys::Reflect::get(entry.control()?.as_ref(), &"value".into()).map_err(backend)?,
        MAX_TAG_INPUT_BYTES,
    )
}
fn available(dom: &Dom, entry: &Entry, element: &Element) -> Result<bool, Error> {
    Ok(entry.interests.borrow().contains(&EventKind::Tags)
        && !entry.editing.borrow().composing
        && !element.has_attribute("disabled")
        && !element.has_attribute("readonly")
        && dom.document.active_element().as_ref() == Some(element))
}
pub(super) fn key(
    dom: &Dom,
    id: NodeId,
    entry: &Entry,
    element: &Element,
    node: &Node,
    event: &web_sys::Event,
    key: &KeyboardEvent,
) -> Result<bool, Error> {
    let Some(policy) = policy(element) else {
        return Ok(false);
    };
    if *node != Node::from(entry.control()?) || !available(dom, entry, element)? {
        return Ok(false);
    }
    let raw = js_sys::Reflect::get(entry.control()?.as_ref(), &"value".into()).map_err(backend)?;
    let empty = js_sys::JsString::from(raw.clone()).length() == 0;
    let Some(command) = policy.key(
        KeyboardInput {
            key: &key.key(),
            control: key.ctrl_key(),
            meta: key.meta_key(),
            alt: key.alt_key(),
            shift: key.shift_key(),
            repeat: key.repeat(),
            composing: key.is_composing(),
            editable: true,
        },
        empty,
    ) else {
        return Ok(false);
    };
    event.prevent_default();
    let command = match command {
        TagKey::Commit => TagsCommand::Commit {
            value: text(raw, MAX_TAG_INPUT_BYTES)?,
        },
        TagKey::RemoveLast => TagsCommand::RemoveLast,
    };
    dom.queue(
        id,
        Event::Tags {
            from: id,
            revision: entry.editing.borrow().revision,
            command,
        },
    );
    Ok(true)
}
pub(super) fn paste(
    dom: &Dom,
    id: NodeId,
    entry: &Entry,
    element: &Element,
    node: &Node,
    event: &web_sys::Event,
) -> Result<(), Error> {
    let Some(policy) = policy(element) else {
        return Ok(());
    };
    if event.default_prevented()
        || *node != Node::from(entry.control()?)
        || !available(dom, entry, element)?
    {
        return Ok(());
    }
    let clipboard =
        js_sys::Reflect::get(event.as_ref(), &"clipboardData".into()).map_err(backend)?;
    if clipboard.is_null() || clipboard.is_undefined() {
        return Ok(());
    }
    let read = js_sys::Reflect::get(&clipboard, &"getData".into())
        .map_err(backend)?
        .dyn_into::<js_sys::Function>()
        .map_err(backend)?;
    let raw = read
        .call1(&clipboard, &"text/plain".into())
        .map_err(backend)?;
    // Reject oversized clipboard text before UTF-8 copying or native insertion.
    if js_sys::JsString::from(raw.clone()).length() as usize > MAX_TAG_PASTE_BYTES {
        event.prevent_default();
        return Err(Error::InvalidEvent(
            "tags clipboard exceeds its finite budget",
        ));
    }
    let pasted = match text(raw, MAX_TAG_PASTE_BYTES) {
        Ok(text) => text,
        Err(error) => {
            event.prevent_default();
            return Err(error);
        }
    };
    if !pasted.chars().any(|character| policy.contains(character)) {
        return Ok(());
    }
    event.prevent_default();
    let control = entry.control()?;
    let selection = |name: &str| -> Result<u32, Error> {
        let value = js_sys::Reflect::get(control.as_ref(), &name.into())
            .map_err(backend)?
            .as_f64()
            .ok_or(Error::InvalidEvent(
                "tags paste requires an editor selection",
            ))?;
        if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > u32::MAX as f64 {
            return Err(Error::InvalidEvent("invalid tags paste selection"));
        }
        Ok(value as u32)
    };
    let command = TagsCommand::Paste {
        value: value(entry)?,
        text: pasted,
        start: selection("selectionStart")?,
        end: selection("selectionEnd")?,
    };
    command.validate()?;
    dom.queue(
        id,
        Event::Tags {
            from: id,
            revision: entry.editing.borrow().revision,
            command,
        },
    );
    Ok(())
}
