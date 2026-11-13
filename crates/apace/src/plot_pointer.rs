//! Native shell ingress for the explicit local plot channel. This module never
//! suppresses defaults, activates controls, invokes application code or changes
//! the engine's original Pointer path. Native engine tests are not device proof.
use super::*;
use apace_traits::events::{
    ApacePointerEvent, ApacePointerId, DomEventData, MouseEventButton, UiEvent,
};
use glory_render::{
    MAX_PLOT_POINTER_COORDINATE as LIMIT, PlotPointer, PlotPointerDevice as Device,
    PlotPointerPhase as Phase,
};
const MAX_POINTERS: usize = 32;
const MAX_PENDING: usize = 4096;
const ORDINARY_LIMIT: usize = MAX_PENDING - 2 * MAX_POINTERS;
#[derive(Clone, Copy)]
struct Sample {
    native: ApacePointerId,
    device: Device,
    phase: Phase,
    client: [f64; 2],
    main: bool,
    at: usize,
}
#[derive(Clone, Copy)]
struct Slot {
    native: ApacePointerId,
    id: i32,
    device: Device,
    capture: Option<NodeId>,
    hover: Option<NodeId>,
    client: [f64; 2],
    last: Option<PlotPointer>,
    bounds: Option<[f64; 4]>,
}
#[derive(Clone, Copy)]
struct Pending {
    at: usize,
    owner: NodeId,
    id: i32,
    device: Device,
    phase: Phase,
    client: [f64; 2],
    fallback: Option<PlotPointer>,
    observed_bounds: Option<[f64; 4]>,
}
#[derive(Default)]
pub(super) struct Pointers {
    interests: BTreeSet<NodeId>,
    slots: Vec<Slot>,
    pending: Vec<Pending>,
    next_finger: i32,
    ui: Option<(Sample, Option<NodeId>, bool)>,
}
impl Pointers {
    fn push(&mut self, p: Pending, cleanup: bool) -> Result<(), Error> {
        if p.phase == Phase::Move
            && let Some(old) = self.pending.last_mut()
            && old.at == p.at
            && old.owner == p.owner
            && old.id == p.id
            && old.phase == Phase::Move
        {
            *old = p;
            return Ok(());
        }
        if self.pending.len() >= if cleanup { MAX_PENDING } else { ORDINARY_LIMIT } {
            return Err(Error::InvalidEvent("native plot pointer pending budget"));
        }
        self.pending.push(p);
        Ok(())
    }
}
fn valid(v: f64) -> bool {
    v.is_finite() && v.abs() <= LIMIT
}
fn sample(pointer: &ApacePointerEvent, phase: Phase, at: usize) -> Result<Sample, Error> {
    let client = [
        f64::from(pointer.coords.client_x),
        f64::from(pointer.coords.client_y),
    ];
    if !client.iter().all(|v| valid(*v)) {
        return Err(Error::InvalidEvent(
            "invalid native fractional plot coordinate",
        ));
    }
    Ok(Sample {
        native: pointer.id,
        device: match pointer.id {
            ApacePointerId::Mouse => Device::Mouse,
            ApacePointerId::Pen => Device::Pen,
            ApacePointerId::Finger(_) => Device::Touch,
        },
        phase,
        client,
        main: pointer.button == MouseEventButton::Main,
        at,
    })
}
fn nearest(state: &State, chain: &[EngineId]) -> Option<NodeId> {
    for physical in chain {
        let Some(mut id) = state
            .dom
            .controls
            .get(physical)
            .or_else(|| state.dom.owners.get(physical))
            .copied()
        else {
            continue;
        };
        loop {
            let node = state.mapper.tree().node(id).ok()?;
            if state.plot_pointer.interests.contains(&id)
                && matches!(node.descriptor.view().props, glory_render::Props::View)
            {
                return Some(id);
            }
            let Some(parent) = node.parent else { break };
            id = parent;
        }
    }
    None
}
fn route(state: &mut State, s: Sample, owner: Option<NodeId>) -> Result<(), Error> {
    if s.phase == Phase::Down && !s.main {
        return Ok(());
    }
    let p = &mut state.plot_pointer;
    if p.pending.len() + 2 > ORDINARY_LIMIT
        && !matches!(s.phase, Phase::Up | Phase::Leave | Phase::Cancel)
    {
        return Err(Error::InvalidEvent("native plot pointer pending budget"));
    }
    let index = p.slots.iter().position(|x| x.native == s.native);
    if index.is_none()
        && (owner.is_none()
            || s.device == Device::Touch && s.phase != Phase::Down
            || matches!(s.phase, Phase::Up | Phase::Leave | Phase::Cancel))
    {
        return Ok(());
    }
    let mut slot = if let Some(i) = index {
        p.slots.remove(i)
    } else {
        if p.slots.len() >= MAX_POINTERS {
            return Err(Error::InvalidEvent("native plot pointer live budget"));
        }
        let id = match s.native {
            ApacePointerId::Mouse => 0,
            ApacePointerId::Pen => 1,
            ApacePointerId::Finger(_) => {
                let id = p.next_finger.max(2);
                p.next_finger = id
                    .checked_add(1)
                    .ok_or(Error::InvalidEvent("native plot pointer id exhausted"))?;
                id
            }
        };
        Slot {
            native: s.native,
            id,
            device: s.device,
            capture: None,
            hover: None,
            client: s.client,
            last: None,
            bounds: None,
        }
    };
    slot.client = s.client;
    let fallback = slot.last;
    let observed_bounds = slot.bounds;
    let previous = slot.capture.or(slot.hover);
    let raw = move |owner, phase| Pending {
        at: s.at,
        owner,
        id: slot.id,
        device: s.device,
        phase,
        client: s.client,
        fallback: if Some(owner) == previous {
            fallback
        } else {
            None
        },
        observed_bounds: if Some(owner) == previous {
            observed_bounds
        } else {
            None
        },
    };
    match s.phase {
        Phase::Down => {
            if let Some(old) = previous
                && (slot.capture.is_some() || Some(old) != owner)
            {
                p.push(raw(old, Phase::Cancel), true)?;
            }
            slot.capture = owner;
            slot.hover = owner;
            slot.last = None;
            slot.bounds = None;
            if let Some(owner) = owner {
                p.push(raw(owner, Phase::Down), false)?;
            }
        }
        Phase::Move => {
            let next = slot.capture.or(owner);
            if slot.capture.is_none()
                && slot.hover != next
                && let Some(old) = slot.hover
            {
                p.push(raw(old, Phase::Leave), true)?;
            }
            if let Some(owner) = next {
                p.push(raw(owner, Phase::Move), false)?;
                if slot.hover != next {
                    slot.last = None;
                    slot.bounds = None;
                }
                slot.hover = next;
            } else {
                slot.hover = None;
            }
        }
        Phase::Up => {
            let next = if s.device == Device::Touch {
                None
            } else {
                owner
            };
            if let Some(old) = slot.capture {
                p.push(raw(old, Phase::Up), true)?;
                if Some(old) != owner {
                    p.push(raw(old, Phase::Leave), true)?;
                }
            }
            slot.capture = None;
            if slot.hover != next {
                slot.last = None;
                slot.bounds = None;
            }
            slot.hover = next;
        }
        Phase::Leave => {
            // Logical capture survives viewport departure; the original native
            // Pointer path still receives its own unmodified Leave.
            if slot.capture.is_none() {
                if let Some(owner) = slot.hover {
                    p.push(raw(owner, Phase::Leave), true)?;
                }
                slot.hover = None;
            }
        }
        Phase::Cancel => {
            if let Some(owner) = previous {
                p.push(raw(owner, Phase::Cancel), true)?;
            }
            slot.capture = None;
            slot.hover = None;
        }
    }
    if slot.capture.is_some() || slot.hover.is_some() {
        p.slots.push(slot);
    }
    state.observation_pending = true;
    Ok(())
}
pub(super) fn begin_ui(state: &mut State, event: &UiEvent) {
    if state.plot_pointer.interests.is_empty() {
        return;
    }
    let at = state.inputs.len();
    if matches!(event, UiEvent::Wheel(_)) {
        cancel(state);
        return;
    }
    let event = match event {
        UiEvent::PointerMove(p) => Some((p, Phase::Move)),
        UiEvent::PointerDown(p) => Some((p, Phase::Down)),
        UiEvent::PointerUp(p) => Some((p, Phase::Up)),
        UiEvent::PointerCancel(p) => Some((p, Phase::Cancel)),
        UiEvent::PointerLeave(p) => Some((p, Phase::Leave)),
        _ => None,
    };
    if let Some((p, phase)) = event {
        match sample(p, phase, at) {
            Ok(raw) => state.plot_pointer.ui = Some((raw, None, false)),
            Err(e) => state.input_error = Some(e),
        }
    }
}
pub(super) fn dom(state: &mut State, chain: &[EngineId], event: &DomEventData) {
    if state.plot_pointer.interests.is_empty() {
        return;
    }
    if matches!(event, DomEventData::Scroll(_) | DomEventData::Wheel(_)) {
        cancel(state);
        return;
    }
    let event = match event {
        DomEventData::PointerMove(p) => Some((p, Phase::Move)),
        DomEventData::PointerDown(p) => Some((p, Phase::Down)),
        DomEventData::PointerUp(p) => Some((p, Phase::Up)),
        DomEventData::PointerCancel(p) => Some((p, Phase::Cancel)),
        DomEventData::PointerLeave(p) => Some((p, Phase::Leave)),
        _ => None,
    };
    let Some((pointer, phase)) = event else {
        return;
    };
    if let Some((sample, _, seen)) = state.plot_pointer.ui {
        if !seen && sample.native == pointer.id && sample.phase == phase {
            let target = nearest(state, chain);
            state.plot_pointer.ui = Some((sample, target, true));
        }
        return;
    }
    let raw = match sample(pointer, phase, state.inputs.len()) {
        Ok(s) => s,
        Err(e) => {
            state.input_error = Some(e);
            return;
        }
    };
    let owner = if phase == Phase::Leave {
        None
    } else {
        nearest(state, chain)
    };
    if let Err(e) = route(state, raw, owner) {
        state.input_error = Some(e);
    }
}
pub(super) fn finish_ui(state: &mut State) {
    if let Some((sample, owner, _)) = state.plot_pointer.ui.take()
        && let Err(e) = route(state, sample, owner)
    {
        state.input_error = Some(e);
    }
}
pub(super) fn cancel(state: &mut State) {
    let at = state.inputs.len();
    let slots = std::mem::take(&mut state.plot_pointer.slots);
    for slot in slots {
        state
            .plot_pointer
            .pending
            .retain(|p| p.id != slot.id || matches!(p.phase, Phase::Cancel | Phase::Leave));
        if let (Some(owner), Some(last)) = (slot.capture.or(slot.hover), slot.last)
            && let Err(e) = state.plot_pointer.push(
                Pending {
                    at,
                    owner,
                    id: slot.id,
                    device: slot.device,
                    phase: Phase::Cancel,
                    client: slot.client,
                    fallback: Some(last),
                    observed_bounds: slot.bounds,
                },
                true,
            )
        {
            state.input_error = Some(e);
        }
    }
    state.observation_pending = true;
}
pub(super) fn interest(state: &mut State, node: NodeId, enabled: bool) {
    if enabled {
        state.plot_pointer.interests.insert(node);
    } else {
        state.plot_pointer.interests.remove(&node);
        state
            .plot_pointer
            .slots
            .retain(|s| s.capture != Some(node) && s.hover != Some(node));
        state.plot_pointer.pending.retain(|p| p.owner != node);
    }
}
pub(super) fn sync(state: &mut State) {
    let tree = state.mapper.tree();
    state
        .plot_pointer
        .interests
        .retain(|id| tree.node(*id).is_ok());
    state.plot_pointer.slots.retain(|s| {
        s.capture
            .or(s.hover)
            .is_some_and(|id| tree.node(id).is_ok())
    });
    state
        .plot_pointer
        .pending
        .retain(|s| tree.node(s.owner).is_ok());
    if !state.plot_pointer.slots.is_empty() {
        state.observation_pending = true;
    }
}
/// beta.4 client bounds do not compose CSS transforms. Reject every computed
/// transform ancestor, including apparently simple scales/translations, rather
/// than pretending its untransformed Bounds are a visual CSS border box.
fn geometry(state: &State, id: NodeId) -> Option<[f64; 4]> {
    let tree = state.mapper.tree();
    if !state.plot_pointer.interests.contains(&id)
        || !tree.active(id)
        || !tree.enabled(id)
        || tree.inert(id)
        || !state.layers.allowed(tree, id)
    {
        return None;
    }
    let physical = state.dom.main(id).ok()?;
    if !state.dom.focus_visible(physical) {
        return None;
    }
    let mut cursor = Some(physical);
    while let Some(current) = cursor {
        let node = state.dom.doc.get_node(current)?;
        if node.primary_styles().is_some() {
            for property in ["transform", "rotate", "scale", "translate", "perspective"] {
                let value = state.dom.doc.resolved_style_value(current, property);
                if !value.is_empty() && value != "none" {
                    return None;
                }
            }
            if state
                .dom
                .doc
                .resolved_style_value(current, "opacity")
                .parse::<f64>()
                .ok()?
                <= 0.
            {
                return None;
            }
        }
        cursor = node.parent;
    }
    let rect = state.dom.doc.get_client_bounding_rect(physical)?;
    let scroll = *state.dom.doc.get_node(physical)?.scroll_offset();
    let offset = state
        .dom
        .translations
        .offset(tree, id, state.dom.time)
        .ok()?;
    let values = [
        rect.x + scroll.x + offset.x,
        rect.y + scroll.y + offset.y,
        rect.width,
        rect.height,
    ];
    if values.iter().all(|v| valid(*v)) && values[2] > 0. && values[3] > 0. {
        Some(values)
    } else {
        None
    }
}
pub(super) fn observe(state: &mut State) {
    let pending = std::mem::take(&mut state.plot_pointer.pending);
    let slots = state.plot_pointer.slots.clone();
    let mut boxes = BTreeMap::new();
    for id in pending
        .iter()
        .map(|r| r.owner)
        .chain(slots.iter().filter_map(|s| s.capture.or(s.hover)))
    {
        boxes.entry(id).or_insert_with(|| geometry(state, id));
    }
    let mut rejected = BTreeSet::new();
    let mut events = Vec::new();
    for slot in slots {
        if let Some(owner) = slot.capture.or(slot.hover) {
            let next = boxes.get(&owner).copied().flatten();
            if next.is_none() || slot.bounds.is_some_and(|old| Some(old) != next) {
                if let Some(last) = slot.last {
                    events.push((
                        0,
                        Input::Event {
                            node: owner,
                            event: Event::PlotPointer {
                                from: owner,
                                pointer: PlotPointer {
                                    phase: Phase::Cancel,
                                    viewport: Rect::default(),
                                    ..last
                                },
                            },
                        },
                    ));
                }
                state.plot_pointer.slots.retain(|s| s.id != slot.id);
                rejected.insert((owner, slot.id));
            }
        }
    }
    for raw in pending {
        if rejected.contains(&(raw.owner, raw.id)) {
            continue;
        }
        let current = boxes.get(&raw.owner).copied().flatten();
        if raw.observed_bounds.is_some_and(|old| Some(old) != current) {
            if let Some(last) = raw.fallback {
                events.push((
                    raw.at,
                    Input::Event {
                        node: raw.owner,
                        event: Event::PlotPointer {
                            from: raw.owner,
                            pointer: PlotPointer {
                                phase: Phase::Cancel,
                                viewport: Rect::default(),
                                ..last
                            },
                        },
                    },
                ));
            }
            state
                .plot_pointer
                .slots
                .retain(|s| s.id != raw.id || s.capture.or(s.hover) != Some(raw.owner));
            rejected.insert((raw.owner, raw.id));
            continue;
        }
        let point = if let Some([left, top, width, height]) = current {
            PlotPointer {
                pointer_id: raw.id,
                device: raw.device,
                phase: raw.phase,
                x: raw.client[0] - left,
                y: raw.client[1] - top,
                viewport: Rect {
                    x: 0.,
                    y: 0.,
                    width,
                    height,
                },
            }
        } else if let Some(last) = raw.fallback {
            PlotPointer {
                phase: Phase::Cancel,
                viewport: Rect::default(),
                ..last
            }
        } else {
            continue;
        };
        if point.validate().is_err() {
            continue;
        }
        if let Some(slot) = state
            .plot_pointer
            .slots
            .iter_mut()
            .find(|s| s.id == raw.id && s.capture.or(s.hover) == Some(raw.owner))
        {
            slot.last = Some(point);
            slot.bounds = boxes.get(&raw.owner).copied().flatten();
        }
        events.push((
            raw.at,
            Input::Event {
                node: raw.owner,
                event: Event::PlotPointer {
                    from: raw.owner,
                    pointer: point,
                },
            },
        ));
    }
    if events.is_empty() {
        return;
    }
    events.sort_by_key(|(at, _)| *at);
    let old = std::mem::take(&mut state.inputs);
    let mut merged = Vec::with_capacity(old.len() + events.len());
    let mut events = events.into_iter().peekable();
    let len = old.len();
    for (i, input) in old.into_iter().enumerate() {
        while events.peek().is_some_and(|(at, _)| *at <= i) {
            merged.push(events.next().unwrap().1);
        }
        merged.push(input);
    }
    for (at, input) in events {
        debug_assert!(at <= len);
        merged.push(input);
    }
    state.inputs = merged;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id() -> NodeId {
        glory_render::NodeIds::new(RuntimeId::new()).allocate()
    }
    fn raw(phase: Phase, at: usize, native: ApacePointerId) -> Sample {
        Sample {
            native,
            device: match native {
                ApacePointerId::Mouse => Device::Mouse,
                ApacePointerId::Pen => Device::Pen,
                ApacePointerId::Finger(_) => Device::Touch,
            },
            phase,
            client: [12.375, 9.125],
            main: true,
            at,
        }
    }
    #[test]
    fn logical_capture_ignores_leave_and_finishes_up_before_leave() {
        let renderer = Renderer::new(RuntimeId::new());
        let mut s = renderer.0.borrow_mut();
        let owner = id();
        route(
            &mut s,
            raw(Phase::Down, 0, ApacePointerId::Mouse),
            Some(owner),
        )
        .unwrap();
        route(&mut s, raw(Phase::Leave, 1, ApacePointerId::Mouse), None).unwrap();
        route(&mut s, raw(Phase::Move, 2, ApacePointerId::Mouse), None).unwrap();
        route(&mut s, raw(Phase::Up, 3, ApacePointerId::Mouse), None).unwrap();
        assert_eq!(
            s.plot_pointer
                .pending
                .iter()
                .map(|p| p.phase)
                .collect::<Vec<_>>(),
            [Phase::Down, Phase::Move, Phase::Up, Phase::Leave]
        );
        assert!(s.plot_pointer.slots.is_empty());
    }
    #[test]
    fn owner_switch_after_up_does_not_reuse_previous_box() {
        let renderer = Renderer::new(RuntimeId::new());
        let mut s = renderer.0.borrow_mut();
        let a = id();
        let b = id();
        route(&mut s, raw(Phase::Down, 0, ApacePointerId::Mouse), Some(a)).unwrap();
        s.plot_pointer.slots[0].bounds = Some([0., 0., 80., 60.]);
        s.plot_pointer.slots[0].last = Some(PlotPointer {
            pointer_id: 0,
            device: Device::Mouse,
            phase: Phase::Down,
            x: 12.,
            y: 9.,
            viewport: Rect {
                x: 0.,
                y: 0.,
                width: 80.,
                height: 60.,
            },
        });
        route(&mut s, raw(Phase::Up, 1, ApacePointerId::Mouse), Some(b)).unwrap();
        let slot = &s.plot_pointer.slots[0];
        assert_eq!(slot.hover, Some(b));
        assert!(slot.last.is_none());
        assert!(slot.bounds.is_none());
        assert_eq!(
            s.plot_pointer
                .pending
                .iter()
                .map(|p| (p.owner, p.phase))
                .collect::<Vec<_>>(),
            [(a, Phase::Down), (a, Phase::Up), (a, Phase::Leave)]
        );
    }
    #[test]
    fn live_touch_budget_is_bounded_and_overflow_preserves_existing_slots() {
        let renderer = Renderer::new(RuntimeId::new());
        let mut s = renderer.0.borrow_mut();
        let owner = id();
        for n in 0..MAX_POINTERS {
            route(
                &mut s,
                raw(Phase::Down, n, ApacePointerId::Finger(n as u64)),
                Some(owner),
            )
            .unwrap();
        }
        let old = s
            .plot_pointer
            .slots
            .iter()
            .map(|p| (p.native, p.id))
            .collect::<Vec<_>>();
        assert!(
            route(
                &mut s,
                raw(Phase::Down, 33, ApacePointerId::Finger(u64::MAX)),
                Some(owner)
            )
            .is_err()
        );
        assert_eq!(
            s.plot_pointer
                .slots
                .iter()
                .map(|p| (p.native, p.id))
                .collect::<Vec<_>>(),
            old
        );
        assert_eq!(s.plot_pointer.pending.len(), MAX_POINTERS);
    }
    #[test]
    fn adjacent_moves_coalesce_only_inside_original_input_barrier() {
        let renderer = Renderer::new(RuntimeId::new());
        let mut s = renderer.0.borrow_mut();
        let owner = id();
        route(
            &mut s,
            raw(Phase::Move, 0, ApacePointerId::Mouse),
            Some(owner),
        )
        .unwrap();
        route(
            &mut s,
            raw(Phase::Move, 0, ApacePointerId::Mouse),
            Some(owner),
        )
        .unwrap();
        route(
            &mut s,
            raw(Phase::Move, 1, ApacePointerId::Mouse),
            Some(owner),
        )
        .unwrap();
        assert_eq!(s.plot_pointer.pending.len(), 2);
        assert_eq!(
            s.plot_pointer
                .pending
                .iter()
                .map(|p| p.at)
                .collect::<Vec<_>>(),
            [0, 1]
        );
    }
}
