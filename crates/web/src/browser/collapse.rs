//! One current intrinsic-height animation observer per live collapse node.
//! Native listeners/promises only queue captured semantic revisions.
use super::{
    backend,
    dom::{Dom, Entry},
};
use futures_util::future::{AbortHandle, Abortable};
use glory_render::{CollapseConfig, Error, Event, EventKind, Input, NodeId, memory::Memory};
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

pub(super) struct Observer {
    revision: u64,
    completed: bool,
    abort: Option<AbortHandle>,
}
impl Drop for Observer {
    fn drop(&mut self) {
        if let Some(abort) = self.abort.take() {
            abort.abort();
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Accepted {
    config: CollapseConfig,
    visible: bool,
}
#[derive(Default)]
pub(super) struct Collapses {
    pub supported: bool,
    live: BTreeMap<NodeId, Accepted>,
    pending: BTreeSet<NodeId>,
    ready: BTreeMap<NodeId, u64>,
}
fn visible(tree: &Memory, id: NodeId) -> bool {
    let Ok(node) = tree.node(id) else {
        return false;
    };
    node.placed
        && !node.suspended
        && node
            .parent
            .is_none_or(|parent| tree.active(parent) && !tree.inert(parent))
}
impl Collapses {
    pub fn update(
        &mut self,
        tree: &Memory,
        changes: &crate::semantic_changes::Changes<'_>,
    ) -> Vec<NodeId> {
        let mut changed = BTreeSet::new();
        for (id, semantics) in changes {
            if let Some(config) = semantics.and_then(|s| s.collapse) {
                let next = Accepted {
                    config,
                    visible: visible(tree, *id),
                };
                if self.live.get(id) != Some(&next) {
                    self.live.insert(*id, next);
                    self.pending.insert(*id);
                    self.ready.remove(id);
                    changed.insert(*id);
                }
            } else if self.live.remove(id).is_some() {
                self.pending.remove(id);
                self.ready.remove(id);
                changed.insert(*id);
            }
        }
        // Only current motion members are inspected, following their current
        // ancestor chain. There is no model/source scan or ancestry history.
        self.live.retain(|id, accepted| {
            if tree.node(*id).is_err() {
                self.pending.remove(id);
                self.ready.remove(id);
                return false;
            }
            let next = visible(tree, *id);
            if accepted.visible != next {
                accepted.visible = next;
                self.pending.insert(*id);
                changed.insert(*id);
            }
            true
        });
        changed.into_iter().collect()
    }
    pub fn retire(&mut self, id: NodeId) {
        self.live.remove(&id);
        self.pending.remove(&id);
        self.ready.remove(&id);
    }
    pub fn needs_frame(&self) -> bool {
        !self.pending.is_empty() || !self.ready.is_empty()
    }
}
impl Dom {
    pub(super) fn configure_collapse(&self) -> Result<(), Error> {
        let css = js_sys::Reflect::get(&js_sys::global(), &"CSS".into()).ok();
        let supports = css
            .as_ref()
            .and_then(|css| js_sys::Reflect::get(css, &"supports".into()).ok())
            .and_then(|value| value.dyn_into::<js_sys::Function>().ok());
        let supported = css.zip(supports).is_some_and(|(css, supports)| {
            [
                ("interpolate-size", "allow-keywords"),
                (
                    "transition-duration",
                    "attr(data-gly-collapse-duration type(<time>),0ms)",
                ),
            ]
            .into_iter()
            .all(|(property, value)| {
                supports
                    .call2(&css, &property.into(), &value.into())
                    .ok()
                    .and_then(|value| value.as_bool())
                    == Some(true)
            })
        }) && js_sys::Reflect::get(&self.root, &"getAnimations".into())
            .is_ok_and(|value| value.is_function());
        self.collapses.borrow_mut().supported = supported;
        if supported {
            self.root
                .remove_attribute("data-gly-collapse-static")
                .map_err(backend)?;
        } else {
            self.root
                .set_attribute("data-gly-collapse-static", "")
                .map_err(backend)?;
        }
        Ok(())
    }
    pub(super) fn admit_collapse(&self, tree: &Memory, id: NodeId) {
        let Ok(node) = tree.node(id) else { return };
        let Some(config) = node
            .descriptor
            .view()
            .semantics
            .as_ref()
            .and_then(|s| s.collapse)
        else {
            return;
        };
        self.collapses
            .borrow_mut()
            .live
            .entry(id)
            .or_insert(Accepted {
                config,
                visible: visible(tree, id),
            });
    }
    pub(super) fn queue_collapse(&self, id: NodeId, entry: &Entry, enabled: bool) {
        if enabled {
            self.collapses.borrow_mut().pending.insert(id);
            self.frame.input();
        } else {
            let previous = entry.collapse.borrow_mut().take();
            drop(previous);
            let mut state = self.collapses.borrow_mut();
            state.pending.remove(&id);
            state.ready.remove(&id);
        }
    }
    pub(super) fn update_collapses(
        &self,
        tree: &Memory,
        changes: &crate::semantic_changes::Changes<'_>,
    ) {
        let changed = self.collapses.borrow_mut().update(tree, changes);
        for id in changed {
            let Ok(entry) = self.entry(id) else { continue };
            let current = self.collapses.borrow().live.get(&id).copied();
            let mut previous = entry.collapse.borrow_mut().take();
            if previous.as_ref().is_some_and(|observer| {
                current.is_some_and(|current| observer.revision == current.config.revision)
            }) {
                // Preserve this revision's terminal bit. Hide/resume does not
                // republish an already completed configuration.
                if current.is_some_and(|current| !current.visible)
                    && let Some(abort) =
                        previous.as_mut().and_then(|observer| observer.abort.take())
                {
                    abort.abort();
                }
                *entry.collapse.borrow_mut() = previous;
            } else {
                drop(previous);
            }
        }
        if self.collapses.borrow().needs_frame() {
            self.frame.input();
        }
    }
    fn complete_collapse(&self, id: NodeId, entry: &Entry, revision: u64) {
        let abort = {
            let mut slot = entry.collapse.borrow_mut();
            let observer = slot.get_or_insert_with(|| {
                Box::new(Observer {
                    revision,
                    completed: false,
                    abort: None,
                })
            });
            if observer.revision != revision || observer.completed {
                return;
            }
            observer.completed = true;
            observer.abort.take()
        };
        if let Some(abort) = abort {
            abort.abort();
        }
        self.collapses.borrow_mut().ready.insert(id, revision);
    }
    pub(super) fn read_collapses(self: &Rc<Self>, tree: &Memory) -> Result<(), Error> {
        let pending = std::mem::take(&mut self.collapses.borrow_mut().pending);
        for id in pending {
            let Some(accepted) = self.collapses.borrow().live.get(&id).copied() else {
                continue;
            };
            let Ok(entry) = self.entry(id) else { continue };
            if !entry
                .interests
                .borrow()
                .contains(&EventKind::CollapseComplete)
            {
                continue;
            }
            let revision = accepted.config.revision;
            let (same, completed, watching) =
                entry
                    .collapse
                    .borrow()
                    .as_ref()
                    .map_or((false, false, false), |observer| {
                        (
                            observer.revision == revision,
                            observer.completed,
                            observer.abort.is_some(),
                        )
                    });
            if same && completed {
                continue;
            }
            if !accepted.visible {
                self.complete_collapse(id, &entry, revision);
                continue;
            }
            if same && watching {
                continue;
            }
            if !self.collapses.borrow().supported {
                self.complete_collapse(id, &entry, revision);
                continue;
            }
            let element = entry.element()?;
            // Reading getAnimations flushes current style. All such reads run
            // before any private layout writes; no per-frame Cage is involved.
            let function =
                js_sys::Reflect::get(&element, &"getAnimations".into()).map_err(backend)?;
            let animations = function
                .dyn_ref::<js_sys::Function>()
                .and_then(|function| function.call0(&element).ok())
                .filter(js_sys::Array::is_array)
                .map(|value| js_sys::Array::from(&value));
            let mut promise = None;
            if animations
                .as_ref()
                .is_some_and(|animations| animations.length() > 8)
            {
                return Err(Error::Unsupported(
                    "collapse host animation count exceeds eight",
                ));
            }
            if let Some(animations) = animations
                && animations.length() <= 8
            {
                for animation in animations.iter() {
                    if js_sys::Reflect::get(&animation, &"transitionProperty".into())
                        .ok()
                        .and_then(|value| value.as_string())
                        .as_deref()
                        == Some("height")
                    {
                        promise = js_sys::Reflect::get(&animation, &"finished".into())
                            .ok()
                            .and_then(|value| value.dyn_into::<js_sys::Promise>().ok());
                        break;
                    }
                }
            }
            let Some(promise) = promise else {
                // Duration zero, reduced motion, unsupported intrinsic/typed
                // attr interpolation, or an already settled host all terminate.
                self.complete_collapse(id, &entry, revision);
                continue;
            };
            let (abort, registration) = AbortHandle::new_pair();
            let previous = entry.collapse.borrow_mut().replace(Box::new(Observer {
                revision,
                completed: false,
                abort: Some(abort),
            }));
            drop(previous);
            let weak_dom = Rc::downgrade(self);
            let weak_entry = Rc::downgrade(&entry);
            wasm_bindgen_futures::spawn_local(async move {
                // Rejection of this still-current transition (including a
                // live Reduce change) is also terminal. Superseded/retired
                // observers revoke publication through Abortable.
                if Abortable::new(JsFuture::from(promise), registration)
                    .await
                    .is_err()
                {
                    return;
                }
                let (Some(dom), Some(entry)) = (weak_dom.upgrade(), weak_entry.upgrade()) else {
                    return;
                };
                if !dom
                    .collapses
                    .borrow()
                    .live
                    .get(&id)
                    .is_some_and(|current| current.config.revision == revision)
                    || !entry
                        .interests
                        .borrow()
                        .contains(&EventKind::CollapseComplete)
                {
                    return;
                }
                dom.complete_collapse(id, &entry, revision);
                dom.frame.input();
            });
        }
        let ready = std::mem::take(&mut self.collapses.borrow_mut().ready);
        for (id, revision) in ready {
            if !self
                .collapses
                .borrow()
                .live
                .get(&id)
                .is_some_and(|accepted| accepted.config.revision == revision)
                || !self.entry(id).is_ok_and(|entry| {
                    entry
                        .interests
                        .borrow()
                        .contains(&EventKind::CollapseComplete)
                })
                || tree.node(id).is_err()
            {
                continue;
            }
            self.inputs.borrow_mut().push(Input::Event {
                node: id,
                event: Event::CollapseComplete { revision },
            });
        }
        Ok(())
    }
}
