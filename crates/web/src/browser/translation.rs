//! One current WAAPI object per live inert translation. All computed-style
//! reads precede animation writes; callbacks only request a host observation.
use super::{backend, dom::Dom};
use glory_render::{Error, Mutation, NodeId, TranslationKind, TranslationMotion, memory::Memory};
use js_sys::{Array, Object, Reflect};
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Element, Event, MediaQueryList};

struct Animation(JsValue);
impl Drop for Animation {
    fn drop(&mut self) {
        let _ = call(&self.0, "cancel");
    }
}
fn call(object: &JsValue, name: &str) -> Result<(), Error> {
    Reflect::get(object, &name.into())
        .map_err(backend)?
        .dyn_into::<js_sys::Function>()
        .map_err(backend)?
        .call0(object)
        .map_err(backend)?;
    Ok(())
}
struct Current {
    next: TranslationMotion,
    accepted: Option<(TranslationMotion, bool)>,
    animation: Option<Animation>,
}
#[derive(Default)]
pub(super) struct Translations {
    live: BTreeMap<NodeId, Current>,
    pending: bool,
    preference: Option<(MediaQueryList, Closure<dyn FnMut(Event)>)>,
}
impl Drop for Translations {
    fn drop(&mut self) {
        if let Some((query, listener)) = &self.preference {
            let _ = query
                .remove_event_listener_with_callback("change", listener.as_ref().unchecked_ref());
        }
    }
}
impl Translations {
    pub fn retire(&mut self, id: NodeId) {
        self.live.remove(&id);
    }
}
fn allowed(tree: &Memory, id: NodeId) -> bool {
    let mut cursor = Some(id);
    while let Some(id) = cursor {
        let Ok(node) = tree.node(id) else {
            return false;
        };
        let d = node.descriptor.view();
        if !node.placed
            || node.suspended
            || d.hidden()
            || d.style.display == Some(glory_render::style::Display::None)
        {
            return false;
        }
        cursor = node.parent;
    }
    true
}
fn transform(x: f64, y: f64) -> String {
    format!("translate({x}px,{y}px)")
}
fn animation(
    element: &Element,
    from: &str,
    to: &str,
    millis: u32,
    repeat: bool,
    paused: bool,
) -> Result<Animation, Error> {
    let frames = Array::new();
    for value in [from, to] {
        let frame = Object::new();
        Reflect::set(&frame, &"transform".into(), &value.into()).map_err(backend)?;
        frames.push(&frame);
    }
    let options = Object::new();
    Reflect::set(&options, &"duration".into(), &millis.into()).map_err(backend)?;
    Reflect::set(
        &options,
        &"iterations".into(),
        &if repeat {
            f64::INFINITY.into()
        } else {
            1f64.into()
        },
    )
    .map_err(backend)?;
    Reflect::set(&options, &"fill".into(), &"both".into()).map_err(backend)?;
    Reflect::set(&options, &"easing".into(), &"linear".into()).map_err(backend)?;
    let value = Reflect::get(element, &"animate".into())
        .map_err(backend)?
        .dyn_into::<js_sys::Function>()
        .map_err(backend)?
        .call2(element, &frames, &options)
        .map_err(backend)?;
    let value = Animation(value);
    if paused {
        call(&value.0, "pause")?;
    }
    Ok(value)
}
impl Dom {
    pub(super) fn update_translations(self: &Rc<Self>, tree: &Memory, mutations: &[Mutation]) {
        let mut state = self.translations.borrow_mut();
        let mut touched: BTreeSet<_> = state.live.keys().copied().collect();
        for mutation in mutations {
            match mutation {
                Mutation::Create { id, .. }
                | Mutation::Patch {
                    id,
                    patch: glory_render::Patch::Semantics(_),
                } => {
                    touched.insert(*id);
                }
                Mutation::CreatePlan { ids, .. } => touched.extend(ids.iter().copied()),
                _ => {}
            }
        }
        for id in touched {
            let next = tree.node(id).ok().and_then(|node| {
                node.descriptor
                    .view()
                    .semantics
                    .as_deref()
                    .and_then(|s| s.translation)
            });
            if let Some(next) = next {
                if let Some(current) = state.live.get_mut(&id) {
                    current.next = next;
                } else {
                    state.live.insert(
                        id,
                        Current {
                            next,
                            accepted: None,
                            animation: None,
                        },
                    );
                }
            } else {
                state.live.remove(&id);
            }
        }
        state.pending = !state.live.is_empty();
        if state.live.is_empty() {
            let retired = std::mem::take(&mut *state);
            drop(state);
            drop(retired);
            return;
        }
        if state.pending {
            self.frame.input();
        }
    }
    pub(super) fn read_translations(self: &Rc<Self>, tree: &Memory) -> Result<(), Error> {
        if self.staging.get() || !self.live.get() {
            return Ok(());
        }
        if self.translations.borrow().live.is_empty() {
            return Ok(());
        }
        if self.translations.borrow().preference.is_none() {
            let window = web_sys::window().ok_or(Error::Unsupported("browser Window"))?;
            let query = window
                .match_media("(prefers-reduced-motion: reduce)")
                .map_err(backend)?
                .ok_or(Error::Unsupported("browser reduced-motion query"))?;
            let weak = Rc::downgrade(self);
            let callback = Closure::new(move |_: Event| {
                if let Some(dom) = weak.upgrade() {
                    dom.translations.borrow_mut().pending = true;
                    dom.frame.input();
                }
            });
            query
                .add_event_listener_with_callback("change", callback.as_ref().unchecked_ref())
                .map_err(backend)?;
            self.translations.borrow_mut().preference = Some((query, callback));
        }
        let reads = {
            let state = self.translations.borrow();
            if !state.pending {
                return Ok(());
            }
            let reduce = state
                .preference
                .as_ref()
                .expect("configured query")
                .0
                .matches();
            let window = web_sys::window().ok_or(Error::Unsupported("browser Window"))?;
            let mut reads = Vec::new();
            for (id, current) in &state.live {
                let enabled = !reduce && allowed(tree, *id);
                if current.accepted == Some((current.next, enabled)) {
                    continue;
                }
                let element = self.entry(*id)?.element()?;
                let computed = window
                    .get_computed_style(&element)
                    .map_err(backend)?
                    .ok_or(Error::Unsupported("browser computed transform"))?
                    .get_property_value("transform")
                    .map_err(backend)?;
                // Mapper accepted the new resting inline transform before this
                // read. A prior static declaration therefore needs its accepted
                // resting value; only an existing animation still covers that
                // inline target with the actual preceding physical position.
                let from = if current.animation.is_none() {
                    current.accepted.map_or(computed, |(old, _)| {
                        let offset = old.resting_offset();
                        transform(offset.x, offset.y)
                    })
                } else {
                    computed
                };
                let completed = current
                    .animation
                    .as_ref()
                    .and_then(|a| Reflect::get(&a.0, &"currentTime".into()).ok())
                    .and_then(|v| v.as_f64())
                    .is_some_and(|elapsed| match current.next.kind {
                        TranslationKind::To {
                            duration_millis, ..
                        } => elapsed >= f64::from(duration_millis),
                        _ => false,
                    });
                reads.push((*id, element, enabled, from, completed));
            }
            reads
        };
        let mut state = self.translations.borrow_mut();
        state.pending = false;
        for (id, element, enabled, from, completed) in reads {
            let preload = js_sys::Symbol::for_("glory.preload.translation");
            let previous_stream = Reflect::get(&element, &preload).map_err(backend)?;
            if !previous_stream.is_undefined() && !previous_stream.is_null() {
                call(&previous_stream, "cancel")?;
                Reflect::delete_property(element.unchecked_ref::<Object>(), &preload)
                    .map_err(backend)?;
            }
            let Some(current) = state.live.get_mut(&id) else {
                continue;
            };
            let previous = current.accepted;
            current.accepted = Some((current.next, enabled));
            if !enabled {
                current.animation = None;
                continue;
            }
            let changed = previous
                .is_none_or(|(old, was_enabled)| old.kind != current.next.kind || !was_enabled);
            if !changed {
                if let Some(animation) = &current.animation {
                    if current.next.paused {
                        call(&animation.0, "pause")?;
                    } else if !completed {
                        call(&animation.0, "play")?;
                    }
                }
                continue;
            }
            let next = match current.next.kind {
                TranslationKind::Loop {
                    from,
                    to,
                    period_millis,
                } if from != to => Some((
                    transform(from.x, from.y),
                    transform(to.x, to.y),
                    period_millis,
                    true,
                )),
                TranslationKind::To {
                    target,
                    duration_millis,
                } if duration_millis > 0 && previous.is_some_and(|(_, enabled)| enabled) => Some((
                    from,
                    transform(target.x, target.y),
                    u32::from(duration_millis),
                    false,
                )),
                _ => None,
            };
            current.animation = None;
            if let Some((from, to, millis, repeat)) = next {
                current.animation = Some(animation(
                    &element,
                    &from,
                    &to,
                    millis,
                    repeat,
                    current.next.paused,
                )?);
            }
        }
        Ok(())
    }
}
