//! Pointer capture is host-owned; only finite queued drag events escape.
use super::super::{
    backend,
    dom::{Dom, DragSession},
};
use super::target;
use glory_render::{DragPhase, Event, EventKind, NodeId};
use wasm_bindgen::JsCast;
use web_sys::{Element, PointerEvent};

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
    let x = f64::from(pointer.client_x());
    let y = f64::from(pointer.client_y());
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
            let slider = dom.entry(from)?.element()?.has_attribute("data-gly-slider");
            if slider {
                let control = dom.entry(from)?.element()?;
                if control.get_attribute("aria-disabled").as_deref() == Some("true")
                    || control.get_attribute("aria-readonly").as_deref() == Some("true")
                {
                    return Ok(false);
                }
                event.prevent_default();
                super::super::dom::focus(&control)?;
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
                started: slider,
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
    if matches!(name, "pointercancel" | "lostpointercapture") {
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
            let captured = dom
                .entry(drag.from)
                .ok()
                .and_then(|entry| entry.element().ok())
                .is_some_and(|element| {
                    element.has_attribute("data-gly-splitter")
                        || element.has_attribute("data-gly-slider")
                });
            let (to, phase) = (if captured {
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
