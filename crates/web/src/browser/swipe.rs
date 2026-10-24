//! One current axis-intent controller; listeners perform no layout/application work.
use super::{backend, dom::Dom};
use glory_render::{
    DragPhase, Error, Event, EventKind, NodeId, Orientation, SwipeCapture, memory::Memory,
};
use wasm_bindgen::JsCast;
use web_sys::{Element, PointerEvent};
#[derive(Clone, Copy)]
pub(super) struct Session {
    pub node: NodeId,
    pointer: i32,
    start: (f64, f64),
    latest: (f64, f64),
    pub capture: SwipeCapture,
    started: bool,
}
fn event(dom: &Dom, session: Session, phase: DragPhase, point: (f64, f64)) {
    dom.queue(
        session.node,
        Event::Swipe {
            from: session.node,
            phase,
            x: point.0,
            y: point.1,
            revision: session.capture.revision,
        },
    );
}
pub(super) fn cancel(dom: &Dom) {
    let current = dom.swipe.borrow_mut().take();
    if let Some(current) = current {
        if let Ok(element) = dom.entry(current.node).and_then(|entry| entry.element()) {
            let _ = element.release_pointer_capture(current.pointer);
        }
        if current.started {
            event(dom, current, DragPhase::Cancel, current.latest);
        }
    }
}
fn valid(tree: &Memory, current: Session) -> bool {
    if !tree.active(current.node) || !tree.enabled(current.node) || tree.inert(current.node) {
        return false;
    }
    let Ok(node) = tree.node(current.node) else {
        return false;
    };
    if node
        .descriptor
        .view()
        .semantics
        .as_ref()
        .and_then(|s| s.swipe_capture)
        != Some(current.capture)
    {
        return false;
    }
    let mut cursor = Some(current.node);
    while let Some(node) = cursor {
        let Ok(node) = tree.node(node) else {
            return false;
        };
        if node.descriptor.view().hidden()
            || node.descriptor.view().style.display == Some(glory_render::style::Display::None)
        {
            return false;
        }
        cursor = node.parent;
    }
    true
}
pub(super) fn update(dom: &Dom, tree: &Memory) {
    let current = *dom.swipe.borrow();
    if current.is_some_and(|current| !valid(tree, current) || !live(dom, current)) {
        cancel(dom);
    }
}
fn live(dom: &Dom, current: Session) -> bool {
    if !dom
        .entry(current.node)
        .is_ok_and(|entry| entry.interests.borrow().contains(&EventKind::Swipe))
    {
        return false;
    }
    dom.entry(current.node).and_then(|entry|entry.element()).is_ok_and(|element|element.is_connected()&&element.get_attribute("data-gly-swipe-axis").as_deref()==Some(current.capture.axis_name())&&element.get_attribute("data-gly-swipe-revision").is_some_and(|revision|revision==current.capture.revision.to_string())&&element.closest("[hidden],[data-gly-hidden],[inert],[aria-disabled=true],fieldset[disabled]").ok().flatten().is_none())
}
fn interested(dom: &Dom, mut node: NodeId) -> Option<(NodeId, SwipeCapture)> {
    loop {
        let entry = dom.entry(node).ok()?;
        if entry.interests.borrow().contains(&EventKind::Swipe) {
            let element = entry.element().ok()?;
            let axis = match element.get_attribute("data-gly-swipe-axis")?.as_str() {
                "horizontal" => Orientation::Horizontal,
                "vertical" => Orientation::Vertical,
                _ => return None,
            };
            let revision = element
                .get_attribute("data-gly-swipe-revision")?
                .parse()
                .ok()?;
            return Some((node, SwipeCapture { axis, revision }));
        }
        node = entry.parent.get()?;
    }
}
pub(super) fn receive(dom: &Dom, name: &str, input: &web_sys::Event) -> Result<bool, Error> {
    if name == "blur"
        && input
            .target()
            .is_some_and(|target| target.is_instance_of::<web_sys::Window>())
    {
        cancel(dom);
        return Ok(false);
    }
    if matches!(name, "resize" | "scroll") {
        cancel(dom);
        return Ok(false);
    }
    let Some(pointer) = input.dyn_ref::<PointerEvent>() else {
        return Ok(false);
    };
    if name == "pointerdown" && !pointer.is_primary() {
        cancel(dom);
        return Ok(false);
    }
    if !pointer.is_primary() {
        return Ok(false);
    }
    let point = (f64::from(pointer.client_x()), f64::from(pointer.client_y()));
    if name == "pointerdown" {
        cancel(dom);
        if pointer.button() != 0 {
            return Ok(false);
        }
        let Some(element) = input
            .target()
            .and_then(|target| target.dyn_into::<Element>().ok())
        else {
            return Ok(false);
        };
        if element.closest("input,textarea,select,button,a,[contenteditable=true],[data-gly-slider],[data-gly-splitter],[data-gly-drag-capture],[data-gly-sheet-grip]").map_err(backend)?.is_some(){return Ok(false);}
        let Some((origin, _)) = super::events::target(dom, &element) else {
            return Ok(false);
        };
        let Some((node, capture)) = interested(dom, origin) else {
            return Ok(false);
        };
        let current = Session {
            node,
            pointer: pointer.pointer_id(),
            start: point,
            latest: point,
            capture,
            started: false,
        };
        if !live(dom, current) {
            return Ok(false);
        }
        *dom.swipe.borrow_mut() = Some(current);
        dom.suppress_click.set(false);
        return Ok(false);
    }
    let Some(mut current) = dom.swipe.borrow_mut().take() else {
        return Ok(false);
    };
    if current.pointer != pointer.pointer_id() {
        *dom.swipe.borrow_mut() = Some(current);
        return Ok(false);
    }
    if matches!(name, "pointercancel" | "lostpointercapture") || !live(dom, current) {
        let consumed = current.started;
        *dom.swipe.borrow_mut() = Some(current);
        cancel(dom);
        return Ok(consumed);
    }
    if name == "pointermove" {
        let dx = (point.0 - current.start.0).abs();
        let dy = (point.1 - current.start.1).abs();
        if !current.started && dx.hypot(dy) >= 6. {
            let (primary, cross) = match current.capture.axis {
                Orientation::Horizontal => (dx, dy),
                Orientation::Vertical => (dy, dx),
            };
            if cross > primary {
                return Ok(false);
            }
            if primary < 6. {
                *dom.swipe.borrow_mut() = Some(current);
                return Ok(false);
            }
            let element = dom.entry(current.node)?.element()?;
            element
                .set_pointer_capture(current.pointer)
                .map_err(backend)?;
            current.started = true;
            event(dom, current, DragPhase::Start, current.start);
        }
        current.latest = point;
        if current.started {
            input.prevent_default();
            event(dom, current, DragPhase::Move, point);
        }
        let consumed = current.started;
        *dom.swipe.borrow_mut() = Some(current);
        return Ok(consumed);
    }
    if name == "pointerup" {
        if let Ok(element) = dom.entry(current.node).and_then(|entry| entry.element()) {
            let _ = element.release_pointer_capture(current.pointer);
        }
        if current.started {
            input.prevent_default();
            dom.suppress_click.set(true);
            event(dom, current, DragPhase::Drop, point);
        }
        return Ok(current.started);
    }
    *dom.swipe.borrow_mut() = Some(current);
    Ok(false)
}
