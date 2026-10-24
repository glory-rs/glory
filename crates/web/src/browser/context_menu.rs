//! Delegated context admission. Native callbacks only enqueue one finite request.
use super::super::{backend, dom::Dom};
use glory_render::{ContextPoint, ContextSource, Event, EventKind, KeyboardInput, NodeId};
use wasm_bindgen::JsCast;
use web_sys::{Element, KeyboardEvent};

pub(super) fn receive(
    dom: &Dom,
    name: &str,
    event: &web_sys::Event,
    from: NodeId,
    element: &Element,
) -> Result<bool, glory_render::Error> {
    if !matches!(name, "contextmenu" | "keydown")
        || event.default_prevented()
        || dom.entry(from)?.editing.borrow().composing
    {
        return Ok(false);
    }
    let source = if name == "keydown" {
        let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
            return Ok(false);
        };
        if !(KeyboardInput {
            key: &key.key(),
            control: key.ctrl_key(),
            meta: key.meta_key(),
            alt: key.alt_key(),
            shift: key.shift_key(),
            repeat: key.repeat(),
            composing: key.is_composing() || dom.entry(from)?.editing.borrow().composing,
            editable: false,
        })
        .context_menu()
        {
            return Ok(false);
        }
        let Some(active) = dom.document.active_element() else {
            return Ok(false);
        };
        if super::target(dom, &active).map(|(id, _)| id) != Some(from) {
            return Ok(false);
        }
        ContextSource::Keyboard
    } else {
        let pointer = js_sys::Reflect::get(event, &"pointerType".into())
            .map_err(backend)?
            .as_string();
        if pointer.as_deref() == Some("touch") {
            return Ok(false);
        }
        ContextSource::Pointer
    };
    let editor = element.closest("input:not([type=checkbox]):not([type=radio]),textarea,select,[contenteditable=true],[role=textbox]").map_err(backend)?.is_some();
    let mut cursor = Some(from);
    while let Some(id) = cursor {
        let entry = dom.entry(id)?;
        if let Ok(region) = entry.element()
            && let Some(policy) = region.get_attribute("data-gly-context")
        {
            // The nearest declared region is a boundary, even without a listener.
            if !entry.interests.borrow().contains(&EventKind::ContextMenu)
                || (editor && policy != "override-editors")
                || !region.is_connected()
                || region
                    .closest("[hidden],[inert],[aria-disabled=true],fieldset[disabled]")
                    .map_err(backend)?
                    .is_some()
            {
                return Ok(false);
            }
            let point = if source == ContextSource::Keyboard {
                let active = dom
                    .document
                    .active_element()
                    .ok_or(glory_render::Error::InvalidEvent("context focus retired"))?;
                let bounds = active.get_bounding_client_rect();
                ContextPoint {
                    x: bounds.left(),
                    y: bounds.bottom(),
                }
            } else {
                let number = |key: &str| {
                    js_sys::Reflect::get(event, &key.into())
                        .map_err(backend)?
                        .as_f64()
                        .ok_or(glory_render::Error::InvalidEvent(
                            "context event requires CSS pixel coordinates",
                        ))
                };
                ContextPoint {
                    x: number("clientX")?,
                    y: number("clientY")?,
                }
            };
            point.validate()?;
            event.prevent_default();
            dom.queue(
                id,
                Event::ContextMenu {
                    from,
                    point,
                    source,
                },
            );
            return Ok(true);
        }
        cursor = entry.parent.get();
    }
    Ok(false)
}
