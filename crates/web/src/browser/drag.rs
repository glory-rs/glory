//! Pointer capture is host-owned; only finite queued drag events escape.
use super::super::{
    backend,
    dom::{Dom, DragSession},
};
use super::target;
use glory_render::{DragPhase, Event, EventKind, NodeId};
use wasm_bindgen::{JsCast, prelude::wasm_bindgen};
use web_sys::{Element, PointerEvent};

// web-sys's stable MouseEvent accessors truncate these CSS pixel coordinates.
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = PointerEvent, typescript_type = "PointerEvent")]
    type PointerCoordinates;

    #[wasm_bindgen(method, getter, structural, js_name = clientX)]
    fn client_x(this: &PointerCoordinates) -> f64;

    #[wasm_bindgen(method, getter, structural, js_name = clientY)]
    fn client_y(this: &PointerCoordinates) -> f64;
}

fn interested(dom: &Dom, mut id: NodeId) -> Option<NodeId> {
    loop {
        let entry = dom.entry(id).ok()?;
        if entry.interests.borrow().contains(&EventKind::Drag) {
            return Some(id);
        }
        id = entry.parent.get()?;
    }
}
fn hit(dom: &Dom, x: f64, y: f64) -> Option<NodeId> {
    let element = dom.document.element_from_point(x as f32, y as f32)?;
    let (id, _) = target(dom, &element)?;
    interested(dom, id)
}
pub(super) fn cancel(dom: &Dom) {
    let drag = dom.drag.borrow_mut().take();
    if let Some(drag) = &drag
        && let Ok(control) = dom.entry(drag.from).and_then(|entry| entry.element())
    {
        let _ = control.release_pointer_capture(drag.pointer);
    }
    if let Some(drag) = drag
        && drag.started
    {
        dom.queue(
            drag.from,
            Event::Drag {
                from: drag.from,
                phase: DragPhase::Cancel,
                x: drag.x,
                y: drag.y,
            },
        );
    }
}
pub(super) fn receive(
    dom: &Dom,
    name: &str,
    event: &web_sys::Event,
) -> Result<bool, glory_render::Error> {
    if name == "blur" {
        if event
            .target()
            .is_some_and(|target| target.is_instance_of::<web_sys::Window>())
        {
            cancel(dom);
        }
        return Ok(false);
    }
    let Some(pointer) = event.dyn_ref::<PointerEvent>() else {
        return Ok(false);
    };
    if !pointer.is_primary() {
        return Ok(false);
    }
    let coordinates = pointer.unchecked_ref::<PointerCoordinates>();
    let x = coordinates.client_x();
    let y = coordinates.client_y();
    if name == "pointerdown" {
        dom.suppress_click.set(false);
        cancel(dom);
        if pointer.button() != 0 {
            return Ok(false);
        }
        let Some(element) = event
            .target()
            .and_then(|target| target.dyn_into::<Element>().ok())
        else {
            return Ok(false);
        };
        if element
            .closest("input,textarea,select,button,a,[contenteditable=true],[role=textbox]")
            .map_err(backend)?
            .is_some()
        {
            return Ok(false);
        }
        if let Some((id, _)) = target(dom, &element)
            && let Some(from) = interested(dom, id)
        {
            let control = dom.entry(from)?.element()?;
            let slider = control.has_attribute("data-gly-slider");
            let splitter = control.has_attribute("data-gly-splitter");
            let capture = control.get_attribute("data-gly-drag-capture");
            let focus_on_press = slider || capture.as_deref() == Some("press-focus");
            let immediate = splitter || focus_on_press || capture.as_deref() == Some("press");
            if immediate {
                if control.closest("[hidden],[data-gly-hidden],[inert],[aria-disabled=true],fieldset[disabled]").map_err(backend)?.is_some()
                    || control.get_attribute("aria-readonly").as_deref() == Some("true")
                {
                    return Ok(false);
                }
                // The separator's default pointer focus preserves the browser's
                // input modality. Scripted focus here can show a keyboard ring.
                if !splitter {
                    event.prevent_default();
                }
                if focus_on_press {
                    super::super::dom::focus(&control)?;
                }
                dom.queue(
                    from,
                    Event::Drag {
                        from,
                        phase: DragPhase::Start,
                        x,
                        y,
                    },
                );
            }
            *dom.drag.borrow_mut() = Some(DragSession {
                from,
                pointer: pointer.pointer_id(),
                x,
                y,
                started: immediate,
                captured: immediate,
            });
            let _ = element.set_pointer_capture(pointer.pointer_id());
        }
        return Ok(dom.drag.borrow().as_ref().is_some_and(|drag| drag.started));
    }
    let Some(mut drag) = dom.drag.borrow_mut().take() else {
        return Ok(false);
    };
    if drag.pointer != pointer.pointer_id() {
        *dom.drag.borrow_mut() = Some(drag);
        return Ok(false);
    }
    let live = dom
        .entry(drag.from)
        .ok()
        .and_then(|entry| entry.element().ok());
    let unavailable = if let Some(control) = live.as_ref() {
        !control.is_connected()
            || control
                .closest(
                    "[hidden],[data-gly-hidden],[inert],[aria-disabled=true],fieldset[disabled]",
                )
                .map_err(backend)?
                .is_some()
            || drag.captured
                && !(control.has_attribute("data-gly-slider")
                    || control.has_attribute("data-gly-splitter")
                    || control.has_attribute("data-gly-drag-capture"))
    } else {
        true
    };
    if matches!(name, "pointercancel" | "lostpointercapture") || unavailable {
        if drag.started {
            dom.queue(
                drag.from,
                Event::Drag {
                    from: drag.from,
                    phase: DragPhase::Cancel,
                    x,
                    y,
                },
            );
        }
        return Ok(drag.started);
    }
    if name == "pointermove" {
        if !drag.started && (x - drag.x).hypot(y - drag.y) >= 6.0 {
            drag.started = true;
            dom.queue(
                drag.from,
                Event::Drag {
                    from: drag.from,
                    phase: DragPhase::Start,
                    x: drag.x,
                    y: drag.y,
                },
            );
        }
        if drag.started {
            dom.queue(
                drag.from,
                Event::Drag {
                    from: drag.from,
                    phase: DragPhase::Move,
                    x,
                    y,
                },
            );
            event.prevent_default();
        }
        if drag.started {
            drag.x = x;
            drag.y = y;
        }
        let consumed = drag.started;
        *dom.drag.borrow_mut() = Some(drag);
        return Ok(consumed);
    }
    if name == "pointerup" {
        if drag.started {
            event.prevent_default();
            dom.suppress_click.set(true);
            let (to, phase) = (if drag.captured {
                Some(drag.from)
            } else {
                hit(dom, x, y)
            })
            .map_or((drag.from, DragPhase::Cancel), |target| {
                (target, DragPhase::Drop)
            });
            dom.queue(
                to,
                Event::Drag {
                    from: drag.from,
                    phase,
                    x,
                    y,
                },
            );
        }
        return Ok(drag.started);
    }
    *dom.drag.borrow_mut() = Some(drag);
    Ok(false)
}
