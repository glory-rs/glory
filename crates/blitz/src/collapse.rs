//! Current native intrinsic-height presentation. Stylo interpolates bounded
//! numeric constraints; the component still owns open state and body lifetime.
use super::*;
use glory_render::{CollapseConfig, MotionPreference};

#[derive(Clone, Copy)]
struct Phase {
    from: f64,
    to: f64,
    start: f64,
    end: f64,
}
impl Phase {
    fn sample(self, time: f64) -> f64 {
        let progress = ((time - self.start) / (self.end - self.start)).clamp(0., 1.);
        self.from + (self.to - self.from) * progress
    }
}
struct Current {
    config: CollapseConfig,
    visible: bool,
    base: String,
    style: String,
    origin: f64,
    phase: Option<Phase>,
    pending: bool,
    animate: bool,
    terminal: bool,
    delivered: bool,
}
#[derive(Default)]
pub(super) struct Collapses {
    live: BTreeMap<NodeId, Current>,
    pub natural_dirty: bool,
}
impl Collapses {
    #[cfg(feature = "paint")]
    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }
    pub fn running(&self) -> bool {
        self.live.values().any(|entry| entry.phase.is_some())
    }
    pub fn ready(&self, subscriptions: &BTreeSet<(NodeId, EventKind)>) -> bool {
        self.live.iter().any(|(id, entry)| {
            entry.terminal
                && !entry.delivered
                && subscriptions.contains(&(*id, EventKind::CollapseComplete))
        })
    }
}
fn visible(tree: &glory_render::memory::Memory, id: NodeId, viewport: bool) -> bool {
    let Ok(node) = tree.node(id) else {
        return false;
    };
    // The closing node itself becomes hidden/inert immediately. Its retained
    // pixels may finish shrinking; an unavailable ancestor terminates motion.
    viewport
        && node.placed
        && !node.suspended
        && node.descriptor.view().style.display != Some(glory_render::style::Display::None)
        && node
            .parent
            .is_none_or(|parent| tree.active(parent) && !tree.inert(parent))
}
impl Dom {
    pub(super) fn sync_collapses(
        &mut self,
        tree: &glory_render::memory::Memory,
        mutations: &[Mutation],
    ) {
        let mut touched = BTreeSet::new();
        let mut retired = false;
        let declared = |id| {
            tree.node(id).ok().is_some_and(|node| {
                node.descriptor
                    .view()
                    .semantics
                    .as_ref()
                    .is_some_and(|semantics| semantics.collapse.is_some())
            })
        };
        for mutation in mutations {
            match mutation {
                Mutation::Create { id, .. } | Mutation::Patch { id, .. } => {
                    if declared(*id) {
                        touched.insert(*id);
                    }
                }
                Mutation::CreatePlan { ids, .. } => {
                    touched.extend(ids.iter().copied().filter(|id| declared(*id)))
                }
                _ => (),
            }
        }
        touched.extend(self.collapses.live.keys().copied());
        if touched.is_empty() {
            return;
        }
        self.collapses.natural_dirty |= !mutations.is_empty();
        let viewport = self.doc.viewport().window_size;
        let viewport = viewport.0 > 0 && viewport.1 > 0;
        for id in touched {
            let node = tree.node(id).ok();
            let config = node
                .as_ref()
                .and_then(|node| node.descriptor.view().semantics.as_ref()?.collapse);
            let Some(config) = config else {
                retired |= self.collapses.live.remove(&id).is_some();
                continue;
            };
            let visible = visible(tree, id, viewport);
            let base = glory_html::style(node.expect("current collapse").descriptor.view().style);
            let height = self
                .main(id)
                .ok()
                .and_then(|id| self.doc.get_node(id))
                .map_or(0., |node| f64::from(node.final_layout().size.height));
            if let Some(entry) = self.collapses.live.get_mut(&id) {
                let changed = entry.config != config;
                if changed
                    || entry.visible != visible
                    || self.translations.preference == MotionPreference::Reduce
                        && entry.phase.is_some()
                {
                    // A hidden ancestor can leave a stale physical box behind.
                    // A settled closed target has zero extent regardless of it.
                    entry.origin = entry
                        .phase
                        .map_or(if entry.config.open { height } else { 0. }, |phase| {
                            phase.sample(self.time)
                        });
                    entry.animate = visible
                        && entry.visible
                        && self.translations.preference == MotionPreference::NoPreference
                        && config.duration_millis > 0;
                    entry.config = config;
                    entry.visible = visible;
                    entry.pending = true;
                    if changed {
                        entry.terminal = false;
                        entry.delivered = false;
                    }
                }
                entry.base = base;
            } else {
                self.collapses.live.insert(
                    id,
                    Current {
                        config,
                        visible,
                        base,
                        style: String::new(),
                        origin: height,
                        phase: None,
                        pending: true,
                        animate: false,
                        terminal: false,
                        delivered: false,
                    },
                );
            }
        }
        if retired
            || self
                .collapses
                .live
                .values()
                .any(|entry| entry.pending || entry.phase.is_some() && self.collapses.natural_dirty)
        {
            self.resolve();
        }
    }
}
fn constraints(height: Option<f64>, millis: f64) -> String {
    let extent = height.map_or_else(
        || "min-height:0!important;max-height:none!important;".to_owned(),
        |height| format!("min-height:{height}px!important;max-height:{height}px!important;"),
    );
    format!(
        "{extent}transition:{}!important;",
        if millis > 0. {
            format!("min-height {millis}ms linear,max-height {millis}ms linear")
        } else {
            "none".into()
        }
    )
}
fn write(dom: &mut Dom, id: NodeId, entry: &Current) -> bool {
    let Ok(main) = dom.main(id) else { return false };
    let style = format!("{}{}", entry.base, entry.style);
    if dom
        .doc
        .get_node(main)
        .and_then(|node| node.element_data())
        .and_then(|element| element.attr(attr("style").local))
        == Some(style.as_str())
    {
        return false;
    }
    dom.doc.mutate().set_attribute(main, attr("style"), &style);
    dom.invalidate();
    true
}
struct Work {
    id: NodeId,
    from: f64,
    end: f64,
    measure: bool,
}
/// Called from resolve after foreign resource delivery, before final layout.
/// Internal measurement passes never publish events or invoke application code.
pub(super) fn resolve(dom: &mut Dom) {
    if dom.collapses.live.is_empty() {
        return;
    }
    let mut state = std::mem::take(&mut dom.collapses);
    let mut work = Vec::new();
    let mut measure = false;
    for (id, entry) in &mut state.live {
        if !entry.pending && entry.phase.is_some_and(|phase| dom.time >= phase.end) {
            entry.phase = None;
            entry.terminal = true;
            entry.style = constraints(None, 0.);
        }
        let retarget = entry.phase.is_some() && entry.config.open && state.natural_dirty;
        if entry.pending || retarget {
            let animate = if entry.pending { entry.animate } else { true };
            let from = if entry.pending {
                entry.origin
            } else {
                entry
                    .phase
                    .expect("retarget current phase")
                    .sample(dom.time)
            };
            let end = if entry.pending {
                dom.time + f64::from(entry.config.duration_millis) / 1000.
            } else {
                entry.phase.expect("retarget current phase").end
            };
            entry.phase = None;
            if !animate || !entry.visible || end <= dom.time || entry.terminal {
                entry.terminal = true;
                entry.style = constraints(None, 0.);
            } else {
                let natural = entry.config.open;
                entry.style = constraints(if natural { None } else { Some(from) }, 0.);
                work.push(Work {
                    id: *id,
                    from,
                    end,
                    measure: natural,
                });
                measure |= natural;
            }
            entry.pending = false;
            entry.animate = false;
        }
        write(dom, *id, entry);
    }
    if (measure || state.natural_dirty && state.running()) && dom.dirty {
        dom.layout();
    }
    for (id, entry) in &mut state.live {
        if entry.phase.is_some() && dom.main(*id).ok().is_none_or(|id| !dom.focus_visible(id)) {
            entry.phase = None;
            entry.terminal = true;
            entry.style = constraints(None, 0.);
            write(dom, *id, entry);
        }
    }
    for item in &work {
        let entry = state.live.get_mut(&item.id).expect("current work");
        let physical = dom.main(item.id).ok();
        let target = if item.measure {
            physical
                .and_then(|id| dom.doc.get_node(id))
                .map_or(0., |node| f64::from(node.final_layout().size.height))
        } else {
            0.
        };
        if !target.is_finite()
            || !item.from.is_finite()
            || item.from == target
            || physical.is_none_or(|id| !dom.focus_visible(id))
        {
            entry.terminal = true;
            entry.style = constraints(None, 0.);
        } else {
            entry.phase = Some(Phase {
                from: item.from,
                to: target,
                start: dom.time,
                end: item.end,
            });
            entry.style = constraints(Some(item.from), 0.);
        }
        write(dom, item.id, entry);
    }
    // Establish every current origin together, then start native numeric
    // transitions at the accepted clock. Opening constraints are released on
    // completion so later intrinsic content changes remain ordinary layout.
    if !work.is_empty() && dom.dirty {
        dom.layout();
    }
    for item in work {
        let entry = state.live.get_mut(&item.id).expect("current work");
        if let Some(phase) = entry.phase {
            entry.style = constraints(Some(phase.to), (phase.end - dom.time) * 1000.);
            write(dom, item.id, entry);
        }
    }
    state.natural_dirty = false;
    dom.collapses = state;
}
pub(super) fn collect(state: &mut State) {
    for (id, entry) in &mut state.dom.collapses.live {
        if entry.terminal
            && !entry.delivered
            && state
                .subscriptions
                .contains(&(*id, EventKind::CollapseComplete))
            && state.mapper.tree().node(*id).is_ok()
        {
            entry.delivered = true;
            state.inputs.push(Input::Event {
                node: *id,
                event: Event::CollapseComplete {
                    revision: entry.config.revision,
                },
            });
        }
    }
}
