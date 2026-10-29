//! Current native pointer gesture. No business callback or per-frame state.
use super::*;
use apace_traits::events::{ApacePointerId, DomEventData, MouseEventButton};
use glory_render::{DragPhase, Orientation, Props, SwipeCapture};

pub(super) struct Session {
    pub node: NodeId,
    pointer: ApacePointerId,
    start: (f64, f64),
    latest: (f64, f64),
    capture: SwipeCapture,
    started: bool,
}
fn queue(state: &mut State, current: &Session, phase: DragPhase, point: (f64, f64)) {
    let event = Event::Swipe {
        from: current.node,
        phase,
        x: point.0,
        y: point.1,
        revision: current.capture.revision,
    };
    if phase == DragPhase::Move
        && let Some(Input::Event {
            node,
            event:
                previous @ Event::Swipe {
                    phase: DragPhase::Move,
                    ..
                },
        }) = state.inputs.last_mut()
        && *node == current.node
    {
        *previous = event;
        return;
    }
    state.inputs.push(Input::Event {
        node: current.node,
        event,
    });
}
pub(super) fn cancel(state: &mut State) {
    if let Some(current) = state.swipe.take()
        && current.started
        && state.mapper.tree().node(current.node).is_ok()
    {
        queue(state, &current, DragPhase::Cancel, current.latest);
    }
}
fn live(state: &State, current: &Session) -> bool {
    let tree = state.mapper.tree();
    if !tree.active(current.node)
        || !tree.enabled(current.node)
        || tree.inert(current.node)
        || !state
            .subscriptions
            .contains(&(current.node, EventKind::Swipe))
    {
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
    while let Some(id) = cursor {
        let Ok(node) = tree.node(id) else {
            return false;
        };
        let view = node.descriptor.view();
        if view.hidden() || view.style.display == Some(glory_render::style::Display::None) {
            return false;
        }
        cursor = node.parent;
    }
    true
}
pub(super) fn update(state: &mut State) {
    if state
        .swipe
        .as_ref()
        .is_some_and(|current| !live(state, current))
    {
        cancel(state);
    }
}
pub(super) fn input(state: &mut State, target: EngineId, event: &DomEventData) -> bool {
    if matches!(event, DomEventData::Wheel(_)) {
        cancel(state);
        return false;
    }
    let (pointer, phase) = match event {
        DomEventData::PointerDown(pointer) => (pointer, 0),
        DomEventData::PointerMove(pointer) => (pointer, 1),
        DomEventData::PointerUp(pointer) => (pointer, 2),
        DomEventData::PointerCancel(pointer) => (pointer, 3),
        _ => return false,
    };
    if !pointer.is_primary {
        cancel(state);
        return false;
    }
    let point = (f64::from(pointer.client_x()), f64::from(pointer.client_y()));
    if phase == 0 {
        cancel(state);
        if pointer.button != MouseEventButton::Main {
            return false;
        }
        let mut cursor = state.dom.owner(target);
        let mut observer = None;
        while let Some(id) = cursor {
            let Ok(node) = state.mapper.tree().node(id) else {
                return false;
            };
            let view = node.descriptor.view();
            if matches!(
                view.props,
                Props::Input(_)
                    | Props::Button(_)
                    | Props::Link(_)
                    | Props::Check(_)
                    | Props::Select(_)
                    | Props::File(_)
            ) || view.semantics.as_ref().is_some_and(|s| {
                s.slider.is_some() || s.splitter.is_some() || s.drag_capture.is_some()
            }) {
                return false;
            }
            if observer.is_none() && state.subscriptions.contains(&(id, EventKind::Swipe)) {
                observer = view
                    .semantics
                    .as_ref()
                    .and_then(|s| s.swipe_capture)
                    .map(|capture| (id, capture));
            }
            cursor = node.parent;
        }
        if let Some((node, capture)) = observer {
            let current = Session {
                node,
                pointer: pointer.id,
                start: point,
                latest: point,
                capture,
                started: false,
            };
            if live(state, &current) {
                state.swipe = Some(current);
                state.suppress_click = false;
            }
        }
        return false;
    }
    let Some(mut current) = state.swipe.take() else {
        return false;
    };
    if current.pointer != pointer.id {
        state.swipe = Some(current);
        return false;
    }
    if phase == 3 || !live(state, &current) {
        let consumed = current.started;
        state.swipe = Some(current);
        cancel(state);
        return consumed;
    }
    if phase == 1 {
        let dx = (point.0 - current.start.0).abs();
        let dy = (point.1 - current.start.1).abs();
        if !current.started && dx.hypot(dy) >= 6. {
            let (primary, cross) = match current.capture.axis {
                Orientation::Horizontal => (dx, dy),
                Orientation::Vertical => (dy, dx),
            };
            if cross > primary {
                return false;
            }
            if primary >= 6. {
                current.started = true;
                queue(state, &current, DragPhase::Start, current.start);
            }
        }
        current.latest = point;
        if current.started {
            queue(state, &current, DragPhase::Move, point);
        }
        let consumed = current.started;
        state.swipe = Some(current);
        return consumed;
    }
    if phase == 2 {
        if current.started {
            state.suppress_click = true;
            queue(state, &current, DragPhase::Drop, point);
        }
        return current.started;
    }
    state.swipe = Some(current);
    false
}
