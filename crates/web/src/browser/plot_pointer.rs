//! Explicit, queued fractional viewport observations. No native capture or DOM
//! mutation: the original Pointer/Drag listeners retain their own semantics.
use super::dom::Dom;
use glory_render::{
    Error, Event, EventKind, Input, MAX_PLOT_POINTER_COORDINATE as LIMIT, NodeId, PlotPointer,
    PlotPointerDevice as Device, PlotPointerPhase as Phase, Rect,
};
use std::collections::{BTreeMap, BTreeSet};
use wasm_bindgen::JsCast;
use web_sys::{Element, Node};

const MAX_POINTERS: usize = 32;
const MAX_PENDING: usize = 4096;
const ORDINARY_LIMIT: usize = MAX_PENDING - 2 * MAX_POINTERS;
#[derive(Clone, Copy)]
struct Slot {
    pointer: i32,
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
    pointer: i32,
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
}
impl Pointers {
    fn push(&mut self, raw: Pending, cleanup: bool) -> Result<(), Error> {
        if raw.phase == Phase::Move
            && let Some(last) = self.pending.last_mut()
            && last.at == raw.at
            && last.owner == raw.owner
            && last.pointer == raw.pointer
            && last.phase == Phase::Move
        {
            *last = raw;
            return Ok(());
        }
        let limit = if cleanup { MAX_PENDING } else { ORDINARY_LIMIT };
        if self.pending.len() >= limit {
            return Err(Error::InvalidEvent("plot pointer pending budget"));
        }
        self.pending.push(raw);
        Ok(())
    }
    /// Pointer IDs may be reused after a device's active contact finishes.
    /// Retire the old device lease before admitting a different device with
    /// that ID; never reinterpret its pending points as the incoming device.
    fn switch_device(&mut self, pointer: i32, device: Device, at: usize) -> Result<(), Error> {
        let Some(index) = self
            .slots
            .iter()
            .position(|s| s.pointer == pointer && s.device != device)
        else {
            return Ok(());
        };
        let slot = self.slots.remove(index);
        self.pending
            .retain(|p| p.pointer != pointer || matches!(p.phase, Phase::Cancel | Phase::Leave));
        if let (Some(owner), Some(last)) = (slot.capture.or(slot.hover), slot.last) {
            self.push(
                Pending {
                    at,
                    owner,
                    pointer,
                    device: slot.device,
                    phase: Phase::Cancel,
                    client: slot.client,
                    fallback: Some(last),
                    observed_bounds: slot.bounds,
                },
                true,
            )?;
        }
        Ok(())
    }
    fn cancel(&mut self, at: usize) -> Result<(), Error> {
        if self.pending.len() + self.slots.len() > MAX_PENDING {
            return Err(Error::InvalidEvent("plot pointer cleanup budget"));
        }
        for slot in std::mem::take(&mut self.slots) {
            // A resize/scroll retirement must not publish queued samples against
            // a different observed viewport. Only previously published leases
            // need a terminal event.
            self.pending.retain(|p| {
                p.pointer != slot.pointer || matches!(p.phase, Phase::Cancel | Phase::Leave)
            });
            if let (Some(owner), Some(last)) = (slot.capture.or(slot.hover), slot.last) {
                self.push(
                    Pending {
                        at,
                        owner,
                        pointer: slot.pointer,
                        device: slot.device,
                        phase: if slot.capture.is_some() {
                            Phase::Cancel
                        } else {
                            Phase::Leave
                        },
                        client: slot.client,
                        fallback: Some(last),
                        observed_bounds: slot.bounds,
                    },
                    true,
                )?;
            }
        }
        Ok(())
    }
}
fn device_key(device: Device) -> u8 {
    match device {
        Device::Mouse => 0,
        Device::Pen => 1,
        Device::Touch => 2,
    }
}
fn finite(v: f64) -> bool {
    v.is_finite() && v.abs() <= LIMIT
}
fn number(pointer: &web_sys::PointerEvent, name: &str) -> Result<f64, Error> {
    let value = js_sys::Reflect::get(pointer.as_ref(), &name.into())
        .map_err(super::backend)?
        .as_f64()
        .ok_or(Error::InvalidEvent("missing fractional pointer coordinate"))?;
    if !finite(value) {
        return Err(Error::InvalidEvent("invalid fractional pointer coordinate"));
    }
    Ok(value)
}
fn nearest(dom: &Dom, element: Option<Element>) -> Option<NodeId> {
    let mut cursor = element?;
    if cursor.closest("[data-gly-app]").ok().flatten()? != dom.root {
        return None;
    }
    loop {
        let found = cursor.closest("[data-gly-node]").ok().flatten()?;
        let id = dom.id_for_token(&found.get_attribute("data-gly-node")?)?;
        if let Ok(entry) = dom.entry(id)
            && entry.element().ok().as_ref() == Some(&found)
            && entry.interests.borrow().contains(&EventKind::PlotPointer)
        {
            return Some(id);
        }
        cursor = found.parent_element()?;
        if cursor.closest("[data-gly-app]").ok().flatten().as_ref() != Some(&dom.root) {
            return None;
        }
    }
}
fn event_element(target: Option<web_sys::EventTarget>) -> Option<Element> {
    let n = target?.dyn_into::<Node>().ok()?;
    n.dyn_ref::<Element>()
        .cloned()
        .or_else(|| n.parent_element())
}
fn is_window_target(event: &web_sys::Event, window: &web_sys::Window) -> bool {
    let expected: &web_sys::EventTarget = window.as_ref();
    event.target().as_ref() == Some(expected)
}
pub(super) fn receive(dom: &Dom, name: &str, event: &web_sys::Event) -> Result<(), Error> {
    if dom.staging.get() || !dom.live.get() || dom.plot_pointer.borrow().interests.is_empty() {
        return Ok(());
    }
    // Window's capture listener also observes non-bubbling descendant blur.
    // Losing focus inside this still-active document must not discard a plot
    // Down that was just queued before the element's native focus change.
    if name == "blur"
        && !dom
            .document
            .default_view()
            .is_some_and(|window| is_window_target(event, &window))
    {
        return Ok(());
    }
    let at = dom.inputs.borrow().len();
    if matches!(name, "blur" | "scroll" | "resize") {
        dom.plot_pointer.borrow_mut().cancel(at)?;
        dom.frame.input();
        return Ok(());
    }
    if !matches!(
        name,
        "pointerdown"
            | "pointermove"
            | "pointerup"
            | "pointercancel"
            | "pointerout"
            | "pointerover"
            | "lostpointercapture"
    ) {
        return Ok(());
    }
    let Some(pointer) = event.dyn_ref::<web_sys::PointerEvent>() else {
        return Ok(());
    };
    let device = match pointer.pointer_type().as_str() {
        "mouse" => Device::Mouse,
        "pen" => Device::Pen,
        "touch" => Device::Touch,
        _ => return Ok(()),
    };
    let client = [number(pointer, "clientX")?, number(pointer, "clientY")?];
    if name == "pointerdown" && pointer.button() != 0 {
        return Ok(());
    }
    let owner = nearest(dom, event_element(event.target()));
    let related = nearest(dom, event_element(pointer.related_target()));
    let id = pointer.pointer_id();
    let mut p = dom.plot_pointer.borrow_mut();
    p.switch_device(id, device, at)?;
    if p.pending.len() + 2 > ORDINARY_LIMIT
        && !matches!(
            name,
            "pointerup" | "pointercancel" | "pointerout" | "lostpointercapture"
        )
    {
        return Err(Error::InvalidEvent("plot pointer pending budget"));
    }
    let index = p.slots.iter().position(|s| s.pointer == id);
    if index.is_none() && !matches!(name, "pointerdown" | "pointermove" | "pointerover") {
        return Ok(());
    }
    let mut slot = if let Some(i) = index {
        p.slots.remove(i)
    } else {
        if owner.is_none() || device == Device::Touch && name != "pointerdown" {
            return Ok(());
        }
        if p.slots.len() >= MAX_POINTERS {
            return Err(Error::InvalidEvent("plot pointer live budget"));
        }
        Slot {
            pointer: id,
            device,
            capture: None,
            hover: None,
            client,
            last: None,
            bounds: None,
        }
    };
    slot.client = client;
    let fallback = slot.last;
    let observed_bounds = slot.bounds;
    let fallback_owner = slot.capture.or(slot.hover);
    let raw = move |owner, phase| Pending {
        at,
        owner,
        pointer: id,
        device,
        phase,
        client,
        fallback: if Some(owner) == fallback_owner {
            fallback
        } else {
            None
        },
        observed_bounds: if Some(owner) == fallback_owner {
            observed_bounds
        } else {
            None
        },
    };
    match name {
        "pointerdown" => {
            if let Some(old) = slot.capture.or(slot.hover)
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
        "pointerup" => {
            let next = if device == Device::Touch { None } else { owner };
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
        "pointercancel" | "lostpointercapture" => {
            if let Some(owner) = slot.capture.or(slot.hover) {
                p.push(raw(owner, Phase::Cancel), true)?;
            }
            slot.capture = None;
            slot.hover = None;
        }
        "pointerout" => {
            if slot.capture.is_none()
                && let Some(old) = slot.hover
                && Some(old) != related
            {
                p.push(raw(old, Phase::Leave), true)?;
                slot.hover = None;
            }
        }
        "pointermove" | "pointerover" => {
            let next = slot.capture.or(owner);
            if slot.capture.is_none()
                && slot.hover != next
                && let Some(old) = slot.hover
            {
                p.push(raw(old, Phase::Leave), true)?;
            }
            if let Some(owner) = next {
                p.push(raw(owner, Phase::Move), false)?;
                if slot.hover != Some(owner) {
                    slot.last = None;
                    slot.bounds = None;
                }
                slot.hover = Some(owner);
            } else {
                slot.hover = None;
            }
        }
        _ => {}
    }
    if slot.capture.is_some() || slot.hover.is_some() {
        p.slots.push(slot);
    }
    drop(p);
    dom.frame.input();
    Ok(())
}
/// CSSOM's actual two-dimensional matrix, not an inferred AABB inverse.
fn axis_matrix(value: &str) -> bool {
    if value == "none" {
        return true;
    }
    let Some(body) = value
        .strip_prefix("matrix(")
        .and_then(|v| v.strip_suffix(')'))
    else {
        return false;
    };
    let values = body
        .split(',')
        .map(|v| v.trim().parse::<f64>())
        .collect::<Result<Vec<_>, _>>();
    values.is_ok_and(|v| {
        v.len() == 6
            && v.iter().all(|v| v.is_finite())
            && v[0] > 0.
            && v[3] > 0.
            && v[1] == 0.
            && v[2] == 0.
    })
}
fn geometry(dom: &Dom, id: NodeId) -> Option<[f64; 4]> {
    let entry = dom.entry(id).ok()?;
    if !entry.interests.borrow().contains(&EventKind::PlotPointer) {
        return None;
    }
    let element = entry.element().ok()?;
    if !element.is_connected()
        || !dom.root.contains(Some(&element))
        || !dom.document_layers.allows(Some(element.as_ref()))
    {
        return None;
    }
    let window = dom.document.default_view()?;
    let mut cursor = Some(element.clone());
    while let Some(node) = cursor {
        if node.has_attribute("inert")
            || node.has_attribute("data-gly-disabled")
            || node.get_attribute("aria-disabled").as_deref() == Some("true")
        {
            return None;
        }
        let css = window.get_computed_style(&node).ok()??;
        if css.get_property_value("display").ok()? == "none"
            || css.get_property_value("visibility").ok()? != "visible"
            || css
                .get_property_value("opacity")
                .ok()?
                .parse::<f64>()
                .ok()?
                <= 0.
        {
            return None;
        }
        if !axis_matrix(&css.get_property_value("transform").ok()?)
            || css.get_property_value("rotate").ok()? != "none"
            || css.get_property_value("perspective").ok()? != "none"
            || css.get_property_value("transform-style").ok()? != "flat"
        {
            return None;
        }
        let scale = css.get_property_value("scale").ok()?;
        if scale != "none" {
            let values = scale.split_whitespace().collect::<Vec<_>>();
            if values.is_empty()
                || values.len() > 2
                || !values.iter().all(|v| {
                    v.trim_end_matches('%')
                        .parse::<f64>()
                        .is_ok_and(|v| v.is_finite() && v > 0.)
                })
            {
                return None;
            }
        }
        let translate = css.get_property_value("translate").ok()?;
        if translate != "none" && translate.split_whitespace().count() > 2 {
            return None;
        }
        cursor = node.parent_element();
    }
    let b = element.get_bounding_client_rect();
    let rect = [b.left(), b.top(), b.width(), b.height()];
    if rect.iter().all(|v| finite(*v)) && rect[2] > 0. && rect[3] > 0. {
        Some(rect)
    } else {
        None
    }
}
pub(super) fn interest(dom: &Dom, id: NodeId, enabled: bool) {
    let mut p = dom.plot_pointer.borrow_mut();
    if enabled {
        p.interests.insert(id);
    } else {
        p.interests.remove(&id);
        p.slots
            .retain(|s| s.capture != Some(id) && s.hover != Some(id));
        p.pending.retain(|s| s.owner != id);
    }
}
pub(super) fn changed(dom: &Dom) {
    if !dom.plot_pointer.borrow().slots.is_empty() {
        dom.frame.input();
    }
}
pub(super) fn retire(dom: &Dom, subtree: &[NodeId]) {
    let mut p = dom.plot_pointer.borrow_mut();
    p.interests.retain(|id| !subtree.contains(id));
    p.slots.retain(|s| {
        !s.capture
            .or(s.hover)
            .is_some_and(|id| subtree.contains(&id))
    });
    p.pending.retain(|s| !subtree.contains(&s.owner));
}
/// Disposal removes old inputs too. Rebase insertion offsets before that retain;
/// relative ordering of every surviving old input and every raw point is kept.
pub(super) fn retaining_inputs(
    dom: &Dom,
    live: &glory_render::NodeMap<std::rc::Rc<super::dom::Entry>>,
) {
    let mut p = dom.plot_pointer.borrow_mut();
    if p.pending.is_empty() {
        return;
    }
    let old = dom.inputs.borrow();
    let mut prefix = Vec::with_capacity(old.len() + 1);
    prefix.push(0);
    for input in old.iter() {
        let keep = !matches!(input,Input::Event{node,..}if !live.contains_key(node));
        prefix.push(prefix.last().copied().unwrap() + usize::from(keep));
    }
    for raw in &mut p.pending {
        raw.at = prefix[raw.at.min(old.len())];
    }
}
pub(super) fn observe(dom: &Dom) {
    let pending = std::mem::take(&mut dom.plot_pointer.borrow_mut().pending);
    let slots = dom.plot_pointer.borrow().slots.clone();
    let mut boxes = BTreeMap::new();
    for id in pending
        .iter()
        .map(|p| p.owner)
        .chain(slots.iter().filter_map(|p| p.capture.or(p.hover)))
    {
        boxes.entry(id).or_insert_with(|| geometry(dom, id));
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
                dom.plot_pointer
                    .borrow_mut()
                    .slots
                    .retain(|s| s.pointer != slot.pointer);
                rejected.insert((owner, slot.pointer, device_key(slot.device)));
            }
        }
    }
    for raw in pending {
        if rejected.contains(&(raw.owner, raw.pointer, device_key(raw.device))) {
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
            if !matches!(raw.phase, Phase::Cancel | Phase::Leave) {
                dom.plot_pointer.borrow_mut().slots.retain(|s| {
                    s.pointer != raw.pointer
                        || s.device != raw.device
                        || s.capture.or(s.hover) != Some(raw.owner)
                });
                rejected.insert((raw.owner, raw.pointer, device_key(raw.device)));
            }
            continue;
        }
        let point = if let Some([left, top, width, height]) = current {
            PlotPointer {
                pointer_id: raw.pointer,
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
        } else {
            if let Some(last) = raw.fallback {
                PlotPointer {
                    phase: Phase::Cancel,
                    viewport: Rect::default(),
                    ..last
                }
            } else {
                continue;
            }
        };
        if point.validate().is_err() {
            continue;
        }
        if let Some(slot) = dom.plot_pointer.borrow_mut().slots.iter_mut().find(|s| {
            s.pointer == raw.pointer
                && s.device == raw.device
                && s.capture.or(s.hover) == Some(raw.owner)
                && !matches!(raw.phase, Phase::Cancel | Phase::Leave)
        }) {
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
    let old = std::mem::take(&mut *dom.inputs.borrow_mut());
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
    *dom.inputs.borrow_mut() = merged;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn css_axis_admission_is_explicit() {
        for s in [
            "none",
            "matrix(1,0,0,1,12.25,-3.5)",
            "matrix(2,0,0,0.5,0,0)",
        ] {
            assert!(axis_matrix(s));
        }
        for s in [
            "matrix(1,0.1,0,1,0,0)",
            "matrix(-1,0,0,1,0,0)",
            "matrix3d(1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)",
            "matrix(NaN,0,0,1,0,0)",
        ] {
            assert!(!axis_matrix(s));
        }
    }
    fn node() -> NodeId {
        glory_render::NodeIds::new(glory_render::RuntimeId::new()).allocate()
    }
    fn pending(owner: NodeId, at: usize, phase: Phase) -> Pending {
        Pending {
            at,
            owner,
            pointer: 17,
            device: Device::Mouse,
            phase,
            client: [12.375, 9.125],
            fallback: None,
            observed_bounds: None,
        }
    }
    #[test]
    fn adjacent_move_coalescing_never_crosses_old_input_or_phase_barriers() {
        let owner = node();
        let mut p = Pointers::default();
        p.push(pending(owner, 0, Phase::Move), false).unwrap();
        let mut last = pending(owner, 0, Phase::Move);
        last.client[0] = 15.125;
        p.push(last, false).unwrap();
        assert_eq!(p.pending.len(), 1);
        assert_eq!(p.pending[0].client[0], 15.125);
        p.push(pending(owner, 1, Phase::Move), false).unwrap();
        p.push(pending(owner, 1, Phase::Down), false).unwrap();
        p.push(pending(owner, 1, Phase::Move), false).unwrap();
        assert_eq!(p.pending.len(), 4);
    }
    #[test]
    fn pending_budget_keeps_reserved_terminal_capacity() {
        let owner = node();
        let mut p = Pointers::default();
        for at in 0..ORDINARY_LIMIT {
            p.push(pending(owner, at, Phase::Down), false).unwrap();
        }
        assert!(p.push(pending(owner, 9999, Phase::Down), false).is_err());
        assert_eq!(p.pending.len(), ORDINARY_LIMIT);
        for at in ORDINARY_LIMIT..MAX_PENDING {
            p.push(pending(owner, at, Phase::Cancel), true).unwrap();
        }
        assert!(
            p.push(pending(owner, MAX_PENDING, Phase::Cancel), true)
                .is_err()
        );
        assert_eq!(p.pending.len(), MAX_PENDING);
    }
    #[test]
    fn cancelling_unobserved_lease_cannot_replay_stale_down() {
        let owner = node();
        let mut p = Pointers::default();
        p.pending.push(pending(owner, 0, Phase::Down));
        p.slots.push(Slot {
            pointer: 17,
            device: Device::Mouse,
            capture: Some(owner),
            hover: Some(owner),
            client: [12.375, 9.125],
            last: None,
            bounds: None,
        });
        p.cancel(0).unwrap();
        assert!(p.pending.is_empty());
        assert!(p.slots.is_empty());
    }
    #[test]
    fn reused_pointer_id_retires_previous_device_without_retyping_cleanup() {
        let owner = node();
        let mut p = Pointers::default();
        let old = PlotPointer {
            pointer_id: 17,
            device: Device::Mouse,
            phase: Phase::Move,
            x: 12.375,
            y: 9.125,
            viewport: Rect {
                x: 0.,
                y: 0.,
                width: 100.,
                height: 80.,
            },
        };
        p.slots.push(Slot {
            pointer: 17,
            device: Device::Mouse,
            capture: None,
            hover: Some(owner),
            client: [12.375, 9.125],
            last: Some(old),
            bounds: Some([0., 0., 100., 80.]),
        });
        p.pending.push(pending(owner, 0, Phase::Move));
        p.switch_device(17, Device::Touch, 1).unwrap();
        assert!(p.slots.is_empty());
        assert_eq!(p.pending.len(), 1);
        assert_eq!(p.pending[0].device, Device::Mouse);
        assert_eq!(p.pending[0].phase, Phase::Cancel);
        assert_eq!(p.pending[0].fallback.unwrap().device, Device::Mouse);
        p.slots.push(Slot {
            pointer: 17,
            device: Device::Touch,
            capture: Some(owner),
            hover: Some(owner),
            client: [12.375, 9.125],
            last: Some(PlotPointer {
                device: Device::Touch,
                phase: Phase::Down,
                ..old
            }),
            bounds: Some([0., 0., 100., 80.]),
        });
        p.cancel(2).unwrap();
        assert_eq!(p.pending.last().unwrap().device, Device::Touch);
        assert_eq!(p.pending.last().unwrap().phase, Phase::Cancel);
    }
    #[test]
    fn device_reuse_before_first_observation_drops_unpublished_old_samples() {
        let owner = node();
        let mut p = Pointers::default();
        p.slots.push(Slot {
            pointer: 17,
            device: Device::Mouse,
            capture: Some(owner),
            hover: Some(owner),
            client: [12.375, 9.125],
            last: None,
            bounds: None,
        });
        p.pending.push(pending(owner, 0, Phase::Down));
        p.switch_device(17, Device::Touch, 1).unwrap();
        assert!(p.slots.is_empty());
        assert!(p.pending.is_empty());
    }
    /// Prepared for a DOM-backed WASM test runner. Synthetic dispatch here
    /// checks target identity only; real focus/pointer ordering is a separate
    /// browser-host regression, not a claim made by this unit test.
    #[cfg(target_arch = "wasm32")]
    #[test]
    fn browser_blur_target_identity_does_not_classify_element_as_window() {
        let window = web_sys::window().expect("DOM-backed WASM test runner");
        let document = window.document().unwrap();
        let body = document.body().unwrap();
        let element = document.create_element("input").unwrap();
        body.append_child(&element).unwrap();
        let undelivered = web_sys::Event::new("blur").unwrap();
        assert!(!is_window_target(&undelivered, &window));
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let recorded = seen.clone();
        let observed_window = window.clone();
        let listener =
            wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(move |event| {
                recorded
                    .borrow_mut()
                    .push(is_window_target(&event, &observed_window));
            });
        window
            .add_event_listener_with_callback_and_bool(
                "blur",
                listener.as_ref().unchecked_ref(),
                true,
            )
            .unwrap();
        element
            .dispatch_event(&web_sys::Event::new("blur").unwrap())
            .unwrap();
        window
            .dispatch_event(&web_sys::Event::new("blur").unwrap())
            .unwrap();
        window
            .remove_event_listener_with_callback_and_bool(
                "blur",
                listener.as_ref().unchecked_ref(),
                true,
            )
            .unwrap();
        body.remove_child(&element).unwrap();
        assert_eq!(*seen.borrow(), [false, true]);
    }
}
