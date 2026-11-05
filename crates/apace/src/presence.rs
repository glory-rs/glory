//! Current opacity presentation, driven by the document clock, without UI callbacks.
use super::*;
use glory_render::{MotionPreference, Patch, PresenceConfig, PresenceEffect, PresenceTiming};

#[derive(Clone, Copy)]
struct Phase {
    from: f64,
    to: f64,
    start: f64,
    end: f64,
    timing: PresenceTiming,
}
fn easing(value: f64, timing: PresenceTiming) -> f64 {
    let (x1, y1, x2, y2) = match timing {
        PresenceTiming::Linear => return value,
        PresenceTiming::Ease => (0.25, 0.1, 0.25, 1.),
        PresenceTiming::EaseIn => (0.42, 0., 1., 1.),
        PresenceTiming::EaseOut => (0., 0., 0.58, 1.),
        PresenceTiming::EaseInOut => (0.42, 0., 0.58, 1.),
    };
    let bezier = |t: f64, a: f64, b: f64| {
        3. * (1. - t).powi(2) * t * a + 3. * (1. - t) * t * t * b + t * t * t
    };
    let (mut low, mut high) = (0., 1.);
    for _ in 0..24 {
        let mid = (low + high) / 2.;
        if bezier(mid, x1, x2) < value {
            low = mid;
        } else {
            high = mid;
        }
    }
    bezier((low + high) / 2., y1, y2)
}
impl Phase {
    fn sample(self, time: f64) -> f64 {
        self.from
            + (self.to - self.from)
                * easing(
                    ((time - self.start) / (self.end - self.start)).clamp(0., 1.),
                    self.timing,
                )
    }
}
struct Current {
    config: PresenceConfig,
    visible: bool,
    base: String,
    opacity: f64,
    phase: Option<Phase>,
    terminal: bool,
    delivered: bool,
}
#[derive(Default)]
pub(super) struct Presences {
    live: BTreeMap<NodeId, Current>,
}
impl Presences {
    #[cfg(feature = "paint")]
    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }
    pub fn running(&self) -> bool {
        self.live.values().any(|v| v.phase.is_some())
    }
    pub fn ready(&self, subscriptions: &BTreeSet<(NodeId, EventKind)>) -> bool {
        self.live.iter().any(|(id, v)| {
            v.terminal
                && !v.delivered
                && subscriptions.contains(&(*id, EventKind::PresenceComplete))
        })
    }
}
pub(super) fn check(
    node: glory_render::DescriptorView<'_>,
    patch: Option<&Patch>,
) -> Result<(), Error> {
    let semantics = match patch {
        None => node.semantics.as_deref(),
        Some(Patch::Semantics(glory_render::Change::Set(value))) => value.as_deref(),
        _ => None,
    };
    if semantics
        .and_then(|s| s.presence.as_ref())
        .is_some_and(|p| p.duration_millis > 0 && p.effect.style(p.distance).has_transform())
    {
        return Err(Error::Unsupported(
            "native presence supports fade/none; transform effects require the browser host",
        ));
    }
    Ok(())
}
impl Dom {
    pub(super) fn sync_presences(
        &mut self,
        tree: &glory_render::memory::Memory,
        mutations: &[Mutation],
    ) {
        let declared = |id| {
            tree.node(id).ok().is_some_and(|n| {
                n.descriptor
                    .view()
                    .semantics
                    .as_ref()
                    .is_some_and(|s| s.presence.is_some())
            })
        };
        let mut touched = BTreeSet::new();
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
        touched.extend(self.presences.live.keys().copied());
        let size = self.doc.viewport().window_size;
        for id in touched {
            let Ok(node) = tree.node(id) else {
                self.presences.live.remove(&id);
                continue;
            };
            let Some(config) = node
                .descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|s| s.presence.as_deref())
                .copied()
            else {
                self.presences.live.remove(&id);
                continue;
            };
            let visible = size.0 > 0
                && size.1 > 0
                && node.placed
                && !node.suspended
                && node.parent.is_none_or(|p| tree.active(p) && !tree.inert(p));
            let base = glory_html::style(node.descriptor.view().style);
            let mut current = self.presences.live.remove(&id);
            let initial = current.is_none();
            let entry = current.get_or_insert(Current {
                config,
                visible,
                base: String::new(),
                opacity: if config.open && !config.appear {
                    1.
                } else {
                    f64::from(config.effect.style(config.distance).opacity) / 1000.
                },
                phase: None,
                terminal: false,
                delivered: false,
            });
            let changed = entry.config != config;
            let reduce = self.translations.preference == MotionPreference::Reduce;
            if initial || changed || entry.visible != visible || reduce && entry.phase.is_some() {
                entry.opacity = entry.phase.map_or(entry.opacity, |p| p.sample(self.time));
                let animate = visible
                    && (initial && config.appear && config.open
                        || !initial && entry.visible && changed)
                    && !reduce
                    && config.duration_millis > 0
                    && config.effect != PresenceEffect::None;
                entry.phase = if animate {
                    let start = self.time + f64::from(config.delay_millis) / 1000.;
                    Some(Phase {
                        from: entry.opacity,
                        to: if config.open {
                            1.
                        } else {
                            f64::from(config.effect.style(config.distance).opacity) / 1000.
                        },
                        start,
                        end: start + f64::from(config.duration_millis) / 1000.,
                        timing: config.timing,
                    })
                } else {
                    None
                };
                if initial || changed {
                    entry.terminal = false;
                    entry.delivered = false;
                }
                if !animate {
                    entry.opacity = if config.open {
                        1.
                    } else {
                        f64::from(config.effect.style(config.distance).opacity) / 1000.
                    };
                    entry.terminal = true;
                }
            }
            entry.config = config;
            entry.visible = visible;
            entry.base = base;
            self.presences
                .live
                .insert(id, current.expect("current presence"));
        }
        resolve(self);
    }
}
fn write(dom: &mut Dom, id: NodeId, entry: &Current) {
    let display = if !entry.config.open && entry.phase.is_none() {
        "none"
    } else {
        "block"
    };
    let style = format!(
        "{}opacity:{}!important;display:{display}!important;transform:none!important;transition:none!important;",
        entry.base, entry.opacity
    );
    let Ok(main) = dom.main(id) else { return };
    if dom
        .doc
        .get_node(main)
        .and_then(|n| n.element_data())
        .and_then(|e| e.attr(attr("style").local))
        != Some(style.as_str())
    {
        dom.doc.mutate().set_attribute(main, attr("style"), &style);
        dom.invalidate();
    }
}
pub(super) fn resolve(dom: &mut Dom) {
    let mut state = std::mem::take(&mut dom.presences);
    for (id, entry) in &mut state.live {
        if let Some(phase) = entry.phase {
            entry.opacity = phase.sample(dom.time);
            if dom.time >= phase.end {
                entry.opacity = phase.to;
                entry.phase = None;
                entry.terminal = true;
            }
        }
        write(dom, *id, entry);
    }
    // Resolve current CSS visibility, including authored display:none ancestors,
    // before deciding whether retained pixels can continue. Only current motion
    // members and their physical ancestor chains are inspected.
    if state.running() && dom.dirty {
        dom.layout();
    }
    for (id, entry) in &mut state.live {
        if entry.phase.is_some() && dom.main(*id).ok().is_none_or(|id| !dom.focus_visible(id)) {
            entry.phase = None;
            entry.terminal = true;
            entry.opacity = if entry.config.open {
                1.
            } else {
                f64::from(entry.config.effect.style(entry.config.distance).opacity) / 1000.
            };
            write(dom, *id, entry);
        }
    }
    dom.presences = state;
}
pub(super) fn collect(state: &mut State) {
    for (id, entry) in &mut state.dom.presences.live {
        if entry.terminal
            && !entry.delivered
            && state
                .subscriptions
                .contains(&(*id, EventKind::PresenceComplete))
            && state.mapper.tree().node(*id).is_ok()
        {
            entry.delivered = true;
            state.inputs.push(Input::Event {
                node: *id,
                event: Event::PresenceComplete {
                    revision: entry.config.revision,
                },
            });
        }
    }
}
