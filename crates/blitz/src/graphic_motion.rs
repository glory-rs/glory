//! Only current live animated graphic declarations enter this host table.
use super::*;
use glory_render::{GraphicAnimation, MotionPreference};
use std::cell::Cell;
#[path = "graphic_transition.rs"]
mod transition;
pub(super) use transition::Transition;

pub(super) struct Animation {
    pub clock: Rc<Cell<f64>>,
    pub allowed: Cell<bool>,
    pub running: Cell<bool>,
}
pub(super) struct Motion {
    pub clock: Rc<Cell<f64>>,
    pub preference: MotionPreference,
    pub candidates: HashMap<NodeId, Rc<Animation>>,
    pub transitions: HashMap<NodeId, Rc<Transition>>,
}
impl Motion {
    pub fn new(preference: MotionPreference) -> Self {
        Self {
            clock: Rc::new(Cell::new(0.)),
            preference,
            candidates: HashMap::new(),
            transitions: HashMap::new(),
        }
    }
    pub fn remove(&mut self, id: NodeId) {
        if let Some(previous) = self.transitions.remove(&id) {
            previous.allowed.set(false);
            previous.cancel();
        }
        if let Some(previous) = self.candidates.remove(&id) {
            previous.allowed.set(false);
            previous.running.set(false);
        }
    }
    pub fn install(
        &mut self,
        id: NodeId,
        animation: Option<GraphicAnimation>,
    ) -> Option<Rc<Animation>> {
        if let Some(previous) = self.candidates.remove(&id) {
            previous.allowed.set(false);
            previous.running.set(false);
        }
        animation.map(|_| {
            let state = Rc::new(Animation {
                clock: self.clock.clone(),
                allowed: Cell::new(false),
                running: Cell::new(false),
            });
            self.candidates.insert(id, state.clone());
            state
        })
    }
    pub fn install_transition(
        &mut self,
        id: NodeId,
        config: Option<glory_render::GraphicTransition>,
        commands: &[glory_render::PathCommand],
        old_commands: Option<&[glory_render::PathCommand]>,
    ) -> Option<Rc<Transition>> {
        let previous = self.transitions.remove(&id);
        let Some(config) = config else {
            if let Some(old) = previous {
                old.cancel();
            }
            return None;
        };
        if let Some(old) = &previous
            && old.duration == config.duration_millis
            && (old_commands == Some(commands) || old_commands.is_none())
        {
            self.transitions.insert(id, old.clone());
            return previous;
        }
        let from = old_commands
            .filter(|old| {
                *old != commands
                    && config.duration_millis > 0
                    && self.preference == MotionPreference::NoPreference
            })
            .map(|old| {
                previous
                    .as_ref()
                    .map_or_else(|| old.to_vec(), |state| state.sample(old))
            });
        if let Some(old) = previous {
            old.allowed.set(false);
            old.cancel();
        }
        let state = Rc::new(Transition::new(
            self.clock.clone(),
            config.duration_millis,
            from,
        ));
        self.transitions.insert(id, state.clone());
        Some(state)
    }
    pub fn finish_transitions(&self) -> bool {
        let mut changed = false;
        for state in self.transitions.values() {
            changed |= state.finish();
        }
        changed
    }
    pub fn permissions(&self, tree: &glory_render::memory::Memory) {
        for (id, state) in &self.transitions {
            let mut cursor = Some(*id);
            let mut allowed = true;
            while let Some(id) = cursor {
                let Ok(node) = tree.node(id) else {
                    allowed = false;
                    break;
                };
                let d = node.descriptor.view();
                if !node.placed
                    || node.suspended
                    || d.hidden()
                    || d.style.display == Some(glory_render::style::Display::None)
                {
                    allowed = false;
                    break;
                }
                cursor = node.parent;
            }
            state.allowed.set(allowed);
            if !allowed {
                state.cancel();
            }
        }
        for (id, state) in &self.candidates {
            let mut cursor = Some(*id);
            let mut allowed = true;
            while let Some(id) = cursor {
                let Ok(node) = tree.node(id) else {
                    allowed = false;
                    break;
                };
                let descriptor = node.descriptor.view();
                if !node.placed
                    || node.suspended
                    || descriptor.hidden()
                    || descriptor.style.display == Some(glory_render::style::Display::None)
                {
                    allowed = false;
                    break;
                }
                cursor = node.parent;
            }
            state.allowed.set(allowed);
            if !allowed {
                state.running.set(false);
            }
        }
    }
}
impl Dom {
    pub(super) fn refresh_graphic_motion(&self) {
        let viewport = self.doc.viewport();
        let visible_viewport = viewport.window_size.0 > 0 && viewport.window_size.1 > 0;
        for (id, state) in &self.motion.transitions {
            let allowed = state.allowed.get()
                && visible_viewport
                && self.motion.preference == MotionPreference::NoPreference
                && self
                    .ranges
                    .get(id)
                    .and_then(|range| range.parts.get(&Part::Main))
                    .is_some_and(|main| {
                        self.focus_visible(*main)
                            && self.doc.get_node(*main).is_some_and(|node| {
                                node.final_layout().size.width > 0.
                                    && node.final_layout().size.height > 0.
                            })
                    });
            if !allowed {
                state.cancel();
            }
            state.running.set(allowed && state.pending());
        }
        for (id, state) in &self.motion.candidates {
            let running = state.allowed.get()
                && visible_viewport
                && self.motion.preference == MotionPreference::NoPreference
                && self
                    .ranges
                    .get(id)
                    .and_then(|range| range.parts.get(&Part::Main))
                    .is_some_and(|main| {
                        self.focus_visible(*main)
                            && self.doc.get_node(*main).is_some_and(|node| {
                                node.final_layout().size.width > 0.
                                    && node.final_layout().size.height > 0.
                            })
                    });
            state.running.set(running);
        }
    }
}
impl Renderer {
    /// Explicit native host preference. This never queries the OS implicitly.
    pub fn set_motion_preference(&self, preference: MotionPreference) {
        let wake = {
            let mut state = self.0.borrow_mut();
            if state.dom.motion.preference == preference {
                return;
            }
            state.dom.motion.preference = preference;
            state.dom.resolve();
            if state.dom.motion.candidates.is_empty() && state.dom.motion.transitions.is_empty() {
                None
            } else {
                state.dom.invalidate_pixels();
                Some(state.dom.resource_wake.clone())
            }
        };
        // User wakers may synchronously inspect the renderer; release its borrow.
        if let Some(wake) = wake {
            wake.wake();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use glory_render::style::{Color, Length, Style};
    use glory_render::{
        GraphicAnimation, GraphicPoint as Point, GraphicProps, GraphicStroke, PathCommand as Path,
        Semantics, StrokeCap,
    };
    use glory_ui::{App, graphic, view};
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        task::{Wake, Waker},
    };
    fn drawing() -> GraphicProps {
        GraphicProps {
            width: 24.,
            height: 24.,
            commands: vec![
                Path::Move(Point::new(12., 12.)),
                Path::Line(Point::new(22., 12.)),
            ],
            stroke: Some(GraphicStroke {
                color: Color(0, 255, 0, 255),
                width: 4.,
                cap: StrokeCap::Butt,
            }),
            animation: Some(GraphicAnimation::Rotate {
                period_millis: 1000,
            }),
            ..Default::default()
        }
    }
    fn app() -> (App, Renderer) {
        let mut renderer = None;
        let app = App::new(|runtime| {
            let host = Renderer::with_config(
                runtime,
                Config {
                    motion: glory_render::MotionPreference::NoPreference,
                    ..Default::default()
                },
            );
            host.set_viewport(40, 40, 1.).unwrap();
            renderer = Some(host.clone());
            host
        });
        (app, renderer.unwrap())
    }
    #[cfg(feature = "raster")]
    fn pixel(raster: &Raster, x: usize, y: usize) -> &[u8] {
        let width = raster.frame().unwrap().width as usize;
        &raster.pixels()[(y * width + x) * 4..(y * width + x) * 4 + 4]
    }
    #[cfg(feature = "raster")]
    #[test]
    fn real_rotation_clock_pixels_preference_and_retirement_have_no_reactive_frame_writes() {
        let (app, renderer) = app();
        app.mount(
            graphic()
                .drawing(drawing())
                .style(Style::new().width(Length::Px(24.)).height(Length::Px(24.))),
        )
        .unwrap();
        let id = app.tree().roots()[0];
        let physical = renderer.engine_id(id, Part::Main).unwrap();
        let mut raster = Raster::new();
        raster.draw(&renderer).unwrap();
        assert_eq!(pixel(&raster, 20, 12), [0, 255, 0, 255]);
        assert_eq!(pixel(&raster, 12, 20), [0, 0, 0, 0]);
        assert!(renderer.inspect(|doc| doc.is_animating()));
        let writes = app.runtime_stats().source_writes;
        let revision = renderer.stats().visual_revision;
        renderer.advance_time(0.25).unwrap();
        assert!(renderer.stats().visual_revision > revision);
        assert!(raster.draw(&renderer).unwrap().rendered);
        assert_eq!(pixel(&raster, 12, 20), [0, 255, 0, 255]);
        assert_eq!(pixel(&raster, 20, 12), [0, 0, 0, 0]);
        assert_eq!(app.runtime_stats().source_writes, writes);
        assert_eq!(renderer.engine_id(id, Part::Main).unwrap(), physical);
        renderer.set_motion_preference(MotionPreference::Reduce);
        raster.draw(&renderer).unwrap();
        assert_eq!(pixel(&raster, 20, 12), [0, 255, 0, 255]);
        assert!(!renderer.inspect(|doc| doc.is_animating()));
        let revision = renderer.stats().visual_revision;
        renderer.advance_time(0.5).unwrap();
        assert_eq!(renderer.stats().visual_revision, revision);
        assert!(!raster.draw(&renderer).unwrap().rendered);
        renderer.set_motion_preference(MotionPreference::NoPreference);
        raster.draw(&renderer).unwrap();
        assert!(renderer.inspect(|doc| doc.is_animating()));
        app.clear().unwrap();
        assert!(renderer.0.borrow().dom.motion.candidates.is_empty());
        assert!(!renderer.inspect(|doc| doc.is_animating()));
        let revision = renderer.stats().visual_revision;
        renderer.advance_time(0.75).unwrap();
        assert_eq!(renderer.stats().visual_revision, revision);
    }
    #[cfg(feature = "raster")]
    #[test]
    fn animated_pixels_preserve_ancestor_clip_opacity_and_resume_the_same_engine_node() {
        use glory_render::style::Overflow;
        let (app, renderer) = app();
        app.mount(
            view()
                .style(
                    Style::new()
                        .width(Length::Px(18.))
                        .height(Length::Px(24.))
                        .overflow_x(Overflow::Hidden)
                        .overflow_y(Overflow::Hidden)
                        .opacity(0.5),
                )
                .fill(
                    graphic()
                        .drawing(drawing())
                        .style(Style::new().width(Length::Px(24.)).height(Length::Px(24.))),
                ),
        )
        .unwrap();
        let root = app.tree().roots()[0];
        let child = app.tree().node(root).unwrap().children[0];
        let physical = renderer.engine_id(child, Part::Main).unwrap();
        let mut raster = Raster::new();
        raster.draw(&renderer).unwrap();
        assert_eq!(pixel(&raster, 20, 12), [0, 0, 0, 0]);
        assert_eq!(pixel(&raster, 16, 12), [0, 128, 0, 128]);
        renderer.advance_time(0.25).unwrap();
        raster.draw(&renderer).unwrap();
        assert_eq!(pixel(&raster, 12, 20), [0, 128, 0, 128]);
        assert_eq!(pixel(&raster, 16, 12), [0, 0, 0, 0]);
        let mut host = renderer.clone();
        host.apply(&[Mutation::Suspend { id: root }]).unwrap();
        raster.draw(&renderer).unwrap();
        assert_eq!(pixel(&raster, 12, 20), [0, 0, 0, 0]);
        host.apply(&[Mutation::Resume {
            id: root,
            parent: None,
            before: None,
        }])
        .unwrap();
        raster.draw(&renderer).unwrap();
        assert_eq!(pixel(&raster, 12, 20), [0, 128, 0, 128]);
        assert_eq!(renderer.engine_id(child, Part::Main).unwrap(), physical);
        assert_eq!(renderer.0.borrow().dom.motion.candidates.len(), 1);
        app.clear().unwrap();
        assert!(renderer.0.borrow().dom.motion.candidates.is_empty());
    }
    #[test]
    fn hide_and_suspend_stop_current_membership_and_clock_wakes_are_coalesced() {
        struct Counter(AtomicUsize);
        impl Wake for Counter {
            fn wake(self: Arc<Self>) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
            fn wake_by_ref(self: &Arc<Self>) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let (app, renderer) = app();
        let hidden = app.scope().cage(false);
        app.mount(
            view()
                .semantics(hidden.map(|hidden| {
                    Some(Box::new(Semantics {
                        hidden: *hidden,
                        ..Default::default()
                    }))
                }))
                .fill(
                    graphic()
                        .drawing(drawing())
                        .style(Style::new().width(Length::Px(24.)).height(Length::Px(24.))),
                ),
        )
        .unwrap();
        app.pump().unwrap();
        renderer.0.borrow_mut().dom.resolve();
        assert!(renderer.inspect(|doc| doc.is_animating()));
        let counter = Arc::new(Counter(AtomicUsize::new(0)));
        let waker = Waker::from(counter.clone());
        renderer.observe_resources(&waker);
        renderer.advance_time(0.1).unwrap();
        renderer.advance_time(0.2).unwrap();
        assert_eq!(counter.0.load(Ordering::SeqCst), 1);
        renderer.observe_resources(&waker);
        renderer.advance_time(0.3).unwrap();
        assert_eq!(counter.0.load(Ordering::SeqCst), 2);
        hidden.revise(|value| *value = true);
        app.flush().unwrap();
        assert!(!renderer.inspect(|doc| doc.is_animating()));
        let revision = renderer.stats().visual_revision;
        renderer.advance_time(0.4).unwrap();
        assert_eq!(renderer.stats().visual_revision, revision);
        hidden.revise(|value| *value = false);
        app.flush().unwrap();
        renderer.0.borrow_mut().dom.resolve();
        assert!(renderer.inspect(|doc| doc.is_animating()));
        let root = app.tree().roots()[0];
        let mut host = renderer.clone();
        host.apply(&[Mutation::Suspend { id: root }]).unwrap();
        assert!(!renderer.inspect(|doc| doc.is_animating()));
        host.apply(&[Mutation::Resume {
            id: root,
            parent: None,
            before: None,
        }])
        .unwrap();
        renderer.0.borrow_mut().dom.resolve();
        assert!(renderer.inspect(|doc| doc.is_animating()));
        app.clear().unwrap();
        assert!(renderer.0.borrow().dom.motion.candidates.is_empty());
    }
}
