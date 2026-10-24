//! Current finite translations. Stylo owns interpolation/painting; the pure
//! phase supplies equivalent query offsets for beta.4's untransformed Bounds.
use super::*;
use glory_render::{MotionPreference, TranslationKind, TranslationOffset, TranslationPhase};

struct Current {
    phase: TranslationPhase,
    serial: u128,
    base: String,
    applied: String,
}
pub(super) struct Translations {
    live: BTreeMap<NodeId, Current>,
    sequence: u128,
    sheet: EngineId,
    css: String,
    pub(super) preference: MotionPreference,
}
impl Translations {
    #[cfg(feature = "paint")]
    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }
    pub fn new(sheet: EngineId, preference: MotionPreference) -> Self {
        Self {
            live: BTreeMap::new(),
            sequence: 0,
            sheet,
            css: String::new(),
            preference,
        }
    }
    fn serial(&mut self) -> Result<u128, Error> {
        self.sequence = self.sequence.checked_add(1).ok_or(Error::InvalidProps(
            "native translation generation exhausted",
        ))?;
        Ok(self.sequence)
    }
    pub fn offset(
        &self,
        tree: &glory_render::memory::Memory,
        id: NodeId,
        time: f64,
    ) -> Result<TranslationOffset, Error> {
        let mut offset = TranslationOffset::ZERO;
        let mut cursor = Some(id);
        while let Some(id) = cursor {
            if let Some(current) = self.live.get(&id) {
                let own = current.phase.offset(time)?;
                offset.x += own.x;
                offset.y += own.y;
            }
            cursor = tree.node(id)?.parent;
        }
        if !offset.x.is_finite() || !offset.y.is_finite() {
            return Err(Error::InvalidProps("nested native translation overflow"));
        }
        Ok(offset)
    }
}
fn allowed(
    tree: &glory_render::memory::Memory,
    id: NodeId,
    viewport: bool,
    preference: MotionPreference,
) -> bool {
    if !viewport || preference == MotionPreference::Reduce {
        return false;
    }
    let mut cursor = Some(id);
    while let Some(id) = cursor {
        let Ok(node) = tree.node(id) else {
            return false;
        };
        let descriptor = node.descriptor.view();
        if !node.placed
            || node.suspended
            || descriptor.hidden()
            || descriptor.style.display == Some(glory_render::style::Display::None)
        {
            return false;
        }
        cursor = node.parent;
    }
    true
}
impl Dom {
    pub(super) fn sync_translations(
        &mut self,
        tree: &glory_render::memory::Memory,
        mutations: &[Mutation],
    ) -> Result<(), Error> {
        let mut physical_change = false;
        let mut touched = BTreeSet::new();
        for mutation in mutations {
            match mutation {
                Mutation::Create { id, .. } | Mutation::Patch { id, .. } => {
                    touched.insert(*id);
                }
                Mutation::CreatePlan { ids, .. } => touched.extend(ids.iter().copied()),
                _ => {}
            }
        }
        touched.extend(self.translations.live.keys().copied());
        let viewport = self.doc.viewport();
        let viewport = viewport.window_size.0 > 0 && viewport.window_size.1 > 0;
        for id in touched {
            let node = tree.node(id).ok();
            let motion = node.as_ref().and_then(|node| {
                node.descriptor
                    .view()
                    .semantics
                    .as_deref()
                    .and_then(|s| s.translation)
            });
            let Some(config) = motion else {
                if self.translations.live.remove(&id).is_some()
                    && let Some(node) = node
                {
                    let base = glory_html::style(node.descriptor.view().style);
                    let main = self.main(id)?;
                    // Cancel fill-mode animation while its rule is still live,
                    // before restoring ownership to the ordinary stylesheet.
                    self.doc.mutate().set_attribute(
                        main,
                        attr("style"),
                        &format!("{base}animation:none;transform:none;"),
                    );
                    self.invalidate();
                    self.resolve();
                    self.doc.mutate().set_attribute(main, attr("style"), &base);
                    self.invalidate();
                    physical_change = true;
                }
                continue;
            };
            let active = allowed(tree, id, viewport, self.translations.preference);
            let changed = self.translations.live.get(&id).is_none_or(|old| {
                old.phase.config().kind != config.kind
                    || old.phase.admitted() != active
                        && !matches!(config.kind, TranslationKind::To { .. })
            });
            let serial = if changed {
                Some(self.translations.serial()?)
            } else {
                None
            };
            let base = glory_html::style(
                node.expect("motion has a live node")
                    .descriptor
                    .view()
                    .style,
            );
            if let Some(old) = self.translations.live.get_mut(&id) {
                old.phase.configure(config, self.time, active)?;
                old.base = base;
                if let Some(serial) = serial {
                    old.serial = serial;
                }
            } else {
                self.translations.live.insert(
                    id,
                    Current {
                        phase: TranslationPhase::new(config, self.time, active)?,
                        serial: serial.expect("new phase"),
                        base,
                        applied: String::new(),
                    },
                );
            }
        }
        let mut css = String::new();
        let mut writes = Vec::new();
        for (id, current) in &mut self.translations.live {
            let config = current.phase.config();
            let static_offset = config.resting_offset();
            let mut style = format!(
                "{}transform:translate({}px,{}px);",
                current.base, static_offset.x, static_offset.y
            );
            if current.phase.admitted() {
                let (from, to, millis, repeat) = match config.kind {
                    TranslationKind::Loop {
                        from,
                        to,
                        period_millis,
                    } => (from, to, period_millis, "infinite"),
                    TranslationKind::To {
                        target,
                        duration_millis,
                    } => (
                        current.phase.origin(),
                        target,
                        u32::from(duration_millis),
                        "1",
                    ),
                };
                if millis > 0 && from != to {
                    let name = format!("gly-translation-{}", current.serial);
                    css.push_str(&format!("@keyframes {name}{{from{{transform:translate({}px,{}px)}}to{{transform:translate({}px,{}px)}}}}",from.x,from.y,to.x,to.y));
                    style.push_str(&format!(
                        "animation:{name} {millis}ms linear 0ms {repeat} both {};",
                        if config.paused { "paused" } else { "running" }
                    ));
                }
            }
            if style != current.applied {
                current.applied = style.clone();
                writes.push((*id, style));
            }
        }
        if css != self.translations.css {
            self.doc
                .mutate()
                .set_node_text(self.translations.sheet, &css);
            self.translations.css = css;
            self.invalidate();
            physical_change = true;
        }
        for (id, style) in writes {
            let main = self.main(id)?;
            self.doc.mutate().set_attribute(main, attr("style"), &style);
            self.invalidate();
            physical_change = true;
        }
        // Establish native CSS animation start at this accepted host timestamp,
        // independently from the time of the next query or paint.
        if physical_change {
            self.resolve();
        }
        Ok(())
    }
}
