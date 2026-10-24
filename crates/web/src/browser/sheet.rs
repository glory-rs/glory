//! One current pointer gesture, finite accepted sheet declarations, and one
//! post-turn restoration. Native listeners capture raw coordinates only.
use std::collections::BTreeMap;

use glory_render::{Error, Event, LayerPlacement, Mutation, NodeId, SheetConfig};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, PointerEvent};

use super::{backend, dom::Dom};

pub(super) fn cancel_outside_document_modal(dom: &Dom) -> Result<(), Error> {
    let current = dom
        .sheets
        .borrow()
        .gesture
        .as_ref()
        .map(|gesture| (gesture.id, gesture.pointer));
    if let Some((id, pointer)) = current
        && let Ok(element) = dom.entry(id).and_then(|entry| entry.element())
        && !dom.document_layers.allows(Some(element.as_ref()))
    {
        if let Some(gesture) = dom.sheets.borrow_mut().gesture.as_mut() {
            gesture.cancelled = true;
        }
        if let Some(grip) = element
            .query_selector("[data-gly-sheet-grip]")
            .map_err(backend)?
        {
            let _ = grip.release_pointer_capture(pointer);
        }
        dom.frame.input();
    }
    Ok(())
}

#[derive(Clone, PartialEq)]
struct Accepted {
    config: SheetConfig,
    edge: LayerPlacement,
    enabled: bool,
}
struct Gesture {
    id: NodeId,
    revision: u64,
    pointer: i32,
    start: (f64, f64),
    point: (f64, f64),
    baseline: Option<f64>,
    started: bool,
    released: bool,
    cancelled: bool,
}
#[derive(Clone, Copy, PartialEq)]
struct Viewport {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
#[derive(Default)]
pub(super) struct Sheets {
    live: BTreeMap<NodeId, Accepted>,
    gesture: Option<Gesture>,
    viewport: Option<Viewport>,
    restore: Option<NodeId>,
    dirty: bool,
}
impl Sheets {
    pub fn needs_frame(&self) -> bool {
        self.dirty || self.gesture.is_some()
    }
    pub fn update(&mut self, tree: &glory_render::memory::Memory, mutations: &[Mutation]) {
        if mutations.iter().any(|mutation| matches!(mutation, Mutation::Patch { id, patch: glory_render::Patch::Style(_) } if self.live.contains_key(id))) {
            // A mapper Style edit replaces the attribute. Reinstall only the
            // current private sheet declarations, without retaining old CSS.
            self.dirty = true;
        }
        for id in mutations.iter().flat_map(|mutation| match mutation {
            Mutation::Create { id, .. } | Mutation::Patch { id, .. } => std::slice::from_ref(id),
            Mutation::CreatePlan { ids, .. } => ids.as_slice(),
            _ => &[],
        }) {
            let Ok(node) = tree.node(*id) else { continue };
            if let Some(layer) = node
                .descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|s| s.layer.as_ref())
                && let Some(config) = &layer.sheet
            {
                let next = Accepted {
                    config: config.clone(),
                    edge: layer.placement,
                    enabled: false,
                };
                if self
                    .live
                    .get(id)
                    .is_none_or(|old| old.config != next.config || old.edge != next.edge)
                {
                    self.dirty = true;
                    self.live.insert(*id, next);
                }
            }
        }
        self.live.retain(|id, state| {
            if tree.node(*id).is_err() {
                return false;
            }
            let mut cursor = Some(*id);
            let mut hidden = false;
            while let Some(id) = cursor {
                let Ok(node) = tree.node(id) else { break };
                hidden |= node
                    .descriptor
                    .view()
                    .semantics
                    .as_ref()
                    .is_some_and(|s| s.hidden);
                cursor = node.parent;
            }
            let enabled = tree.active(*id) && !tree.inert(*id) && !hidden;
            self.dirty |= state.enabled != enabled;
            state.enabled = enabled;
            true
        });
        if self.gesture.as_ref().is_some_and(|g| {
            self.live
                .get(&g.id)
                .is_none_or(|s| !s.enabled || s.config.revision != g.revision)
        }) {
            self.gesture = None;
            self.dirty = true;
        }
        if self.restore.is_some_and(|id| !self.live.contains_key(&id)) {
            self.restore = None;
        }
    }
}

/// Accepted program configuration gets a CSS expression without reading host
/// layout. Explicit Bounds barriers therefore see the new extent even before
/// the next RAF; that frame reads/clamps the actual visual viewport.
pub(super) fn sync_accepted(dom: &Dom) -> Result<(), Error> {
    let state = dom.sheets.borrow();
    if !state.dirty {
        return Ok(());
    }
    for (id, accepted) in &state.live {
        if state.gesture.as_ref().is_some_and(|g| g.id == *id) {
            continue;
        }
        let Ok(element) = dom.entry(*id).and_then(|entry| entry.element()) else {
            continue;
        };
        let element = element
            .dyn_into::<HtmlElement>()
            .map_err(|_| Error::Unsupported("sheet element"))?;
        let viewport = if horizontal(accepted.edge) {
            "var(--gly-sheet-width,100vw)"
        } else {
            "var(--gly-sheet-height,100dvh)"
        };
        element
            .style()
            .set_property(
                "--gly-sheet-extent",
                &format!(
                    "calc(min({viewport},{}px) * {})",
                    accepted.config.max_extent, accepted.config.fractions[accepted.config.current]
                ),
            )
            .map_err(backend)?;
        element
            .style()
            .set_property(
                "--gly-sheet-settle",
                &format!("{}ms", accepted.config.settle_millis),
            )
            .map_err(backend)?;
    }
    Ok(())
}

pub(super) fn receive(dom: &Dom, name: &str, event: &web_sys::Event) -> Result<bool, Error> {
    if name == "blur"
        && !event
            .target()
            .is_some_and(|target| target.is_instance_of::<web_sys::Window>())
    {
        // Window's capture listener also sees ordinary descendant focus loss.
        // Clicking a passive grip must not cancel its newly captured gesture.
        return Ok(false);
    }
    if name == "resize" || name == "blur" {
        let mut state = dom.sheets.borrow_mut();
        if let Some(gesture) = state.gesture.as_mut() {
            gesture.cancelled = true;
        }
        state.dirty = true;
        drop(state);
        dom.frame.input();
        return Ok(false);
    }
    let Some(pointer) = event.dyn_ref::<PointerEvent>() else {
        return Ok(false);
    };
    if !pointer.is_primary() {
        return Ok(false);
    }
    let point = (f64::from(pointer.client_x()), f64::from(pointer.client_y()));
    if name == "pointerdown" {
        if pointer.button() != 0 {
            return Ok(false);
        }
        let Some(target) = event.target().and_then(|e| e.dyn_into::<Element>().ok()) else {
            return Ok(false);
        };
        let Some(grip) = target.closest("[data-gly-sheet-grip]").map_err(backend)? else {
            return Ok(false);
        };
        if target
            .closest("input,textarea,select,button,a,[contenteditable=true]")
            .map_err(backend)?
            .is_some()
            || grip.closest("[hidden],[inert]").map_err(backend)?.is_some()
        {
            return Ok(false);
        }
        let Some(layer) = grip.closest("[data-gly-sheet]").map_err(backend)? else {
            return Ok(false);
        };
        let Some((id, _)) = super::events::target(dom, &layer) else {
            return Ok(false);
        };
        let mut state = dom.sheets.borrow_mut();
        let Some(accepted) = state.live.get(&id).filter(|s| s.enabled) else {
            return Ok(false);
        };
        let revision = accepted.config.revision;
        state.dirty |= state.gesture.is_some();
        state.gesture = Some(Gesture {
            id,
            revision,
            pointer: pointer.pointer_id(),
            start: point,
            point,
            baseline: None,
            started: false,
            released: false,
            cancelled: false,
        });
        drop(state);
        dom.suppress_click.set(false);
        let _ = grip.set_pointer_capture(pointer.pointer_id());
        dom.frame.input();
        return Ok(true);
    }
    let mut state = dom.sheets.borrow_mut();
    let Some(gesture) = state
        .gesture
        .as_mut()
        .filter(|g| g.pointer == pointer.pointer_id())
    else {
        return Ok(false);
    };
    if gesture.released {
        return Ok(true);
    }
    match name {
        "pointermove" => {
            gesture.point = point;
            gesture.started |= (point.0 - gesture.start.0).hypot(point.1 - gesture.start.1) >= 6.0;
            if gesture.started {
                event.prevent_default();
            }
        }
        "pointerup" => {
            gesture.point = point;
            gesture.released = true;
            if gesture.started {
                event.prevent_default();
                dom.suppress_click.set(true);
            }
        }
        "pointercancel" | "lostpointercapture" => gesture.cancelled = true,
        _ => return Ok(false),
    }
    drop(state);
    dom.frame.input();
    Ok(true)
}

fn viewport(dom: &Dom) -> Result<Viewport, Error> {
    let window = dom
        .document
        .default_view()
        .ok_or(Error::Unsupported("sheet viewport"))?;
    let value = if let Some(v) = window.visual_viewport() {
        Viewport {
            x: v.offset_left(),
            y: v.offset_top(),
            width: v.width(),
            height: v.height(),
        }
    } else {
        Viewport {
            x: 0.0,
            y: 0.0,
            width: window
                .inner_width()
                .map_err(backend)?
                .as_f64()
                .unwrap_or(0.0),
            height: window
                .inner_height()
                .map_err(backend)?
                .as_f64()
                .unwrap_or(0.0),
        }
    };
    if [value.x, value.y, value.width, value.height]
        .iter()
        .any(|v| !v.is_finite())
        || value.width < 0.0
        || value.height < 0.0
    {
        return Err(Error::InvalidEvent("nonfinite sheet viewport"));
    }
    Ok(value)
}
fn horizontal(edge: LayerPlacement) -> bool {
    matches!(edge, LayerPlacement::Left | LayerPlacement::Right)
}
fn axis(v: Viewport, edge: LayerPlacement) -> f64 {
    if horizontal(edge) { v.width } else { v.height }
}
pub(super) struct Write {
    id: NodeId,
    viewport: Viewport,
    extent: f64,
    millis: u16,
}
/// Called once at the start of the frame read phase. This performs every sheet
/// layout/visualViewport read before any sheet private CSS write.
pub(super) fn read(dom: &Dom) -> Result<Vec<Write>, Error> {
    let mut state = dom.sheets.borrow_mut();
    if !state.dirty && state.gesture.is_none() {
        return Ok(Vec::new());
    }
    let view = viewport(dom)?;
    let resized = state.viewport.is_some_and(|old| old != view);
    state.viewport = Some(view);
    let mut writes = Vec::new();
    if state.dirty || resized {
        for (id, accepted) in &state.live {
            writes.push(Write {
                id: *id,
                viewport: view,
                extent: accepted
                    .config
                    .extent(axis(view, accepted.edge), accepted.config.current)?,
                millis: if resized {
                    0
                } else {
                    accepted.config.settle_millis
                },
            });
        }
    }
    state.dirty = false;
    let Some(mut gesture) = state.gesture.take() else {
        return Ok(writes);
    };
    let Some(accepted) = state
        .live
        .get(&gesture.id)
        .cloned()
        .filter(|s| s.enabled && s.config.revision == gesture.revision)
    else {
        return Ok(writes);
    };
    if resized {
        gesture.cancelled = true;
    }
    let available = axis(view, accepted.edge);
    let baseline = if let Some(baseline) = gesture.baseline {
        baseline
    } else {
        let bounds = dom.entry(gesture.id)?.element()?.get_bounding_client_rect();
        let measured = if horizontal(accepted.edge) {
            bounds.width()
        } else {
            bounds.height()
        };
        let baseline = if measured.is_finite() {
            measured.clamp(0.0, available.min(accepted.config.max_extent))
        } else {
            accepted.config.extent(available, accepted.config.current)?
        };
        gesture.baseline = Some(baseline);
        baseline
    };
    if gesture.cancelled {
        writes.push(Write {
            id: gesture.id,
            viewport: view,
            extent: accepted.config.extent(available, accepted.config.current)?,
            millis: 0,
        });
        return Ok(writes);
    }
    if gesture.started {
        let delta = match accepted.edge {
            LayerPlacement::Left => gesture.point.0 - gesture.start.0,
            LayerPlacement::Right => gesture.start.0 - gesture.point.0,
            LayerPlacement::Top => gesture.point.1 - gesture.start.1,
            LayerPlacement::Bottom => gesture.start.1 - gesture.point.1,
            _ => unreachable!("validated sheet edge"),
        };
        let extent = (baseline + delta).clamp(0.0, available.min(accepted.config.max_extent));
        writes.push(Write {
            id: gesture.id,
            viewport: view,
            extent,
            millis: 0,
        });
        if gesture.released {
            let command = accepted.config.nearest(available, extent)?;
            state.restore = Some(gesture.id);
            // The current live declaration owns the captured revision. Business
            // handlers run later, after the renderer's read phase has ended.
            dom.queue(
                gesture.id,
                Event::Sheet {
                    from: gesture.id,
                    revision: gesture.revision,
                    command,
                },
            );
        }
    }
    if !gesture.released {
        state.gesture = Some(gesture);
    }
    Ok(writes)
}
pub(super) fn write(dom: &Dom, writes: Vec<Write>) -> Result<(), Error> {
    for value in writes {
        let Ok(element) = dom.entry(value.id).and_then(|entry| entry.element()) else {
            continue;
        };
        let element = element
            .dyn_into::<HtmlElement>()
            .map_err(|_| Error::Unsupported("sheet element"))?;
        let style = element.style();
        for (key, number) in [
            ("--gly-sheet-extent", value.extent),
            ("--gly-sheet-x", value.viewport.x),
            ("--gly-sheet-y", value.viewport.y),
            ("--gly-sheet-width", value.viewport.width),
            ("--gly-sheet-height", value.viewport.height),
        ] {
            style
                .set_property(key, &format!("{number}px"))
                .map_err(backend)?;
        }
        style
            .set_property("--gly-sheet-settle", &format!("{}ms", value.millis))
            .map_err(backend)?;
    }
    Ok(())
}
/// Runs after the complete RAF host turn, including when controlled callbacks
/// accept no mutation. The declaration registry now contains accepted state.
pub(super) fn finish_turn(dom: &Dom) -> Result<(), Error> {
    let value = {
        let mut state = dom.sheets.borrow_mut();
        let Some(id) = state.restore.take() else {
            return Ok(());
        };
        state
            .live
            .get(&id)
            .zip(state.viewport)
            .map(|(accepted, viewport)| {
                accepted
                    .config
                    .extent(axis(viewport, accepted.edge), accepted.config.current)
                    .map(|extent| Write {
                        id,
                        viewport,
                        extent,
                        millis: accepted.config.settle_millis,
                    })
            })
            .transpose()?
    };
    write(dom, value.into_iter().collect())
}
