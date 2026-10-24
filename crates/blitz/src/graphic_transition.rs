use super::*;
use glory_render::{GraphicTransition, PathCommand};
use std::cell::{Cell, RefCell};

/// Current source geometry only, released on completion/cancellation. The
/// target remains in the widget's current GraphicProps, never a path history.
pub(crate) struct Transition {
    pub clock: Rc<Cell<f64>>,
    pub allowed: Cell<bool>,
    pub running: Cell<bool>,
    pub duration: u16,
    start: f64,
    from: RefCell<Option<Vec<PathCommand>>>,
}
impl Transition {
    pub fn new(clock: Rc<Cell<f64>>, duration: u16, from: Option<Vec<PathCommand>>) -> Self {
        Self {
            start: clock.get(),
            clock,
            allowed: Cell::new(false),
            running: Cell::new(false),
            duration,
            from: RefCell::new(from),
        }
    }
    fn progress(&self) -> f64 {
        if self.duration == 0 {
            return 1.0;
        }
        ((self.clock.get() - self.start) / (f64::from(self.duration) / 1000.0)).clamp(0.0, 1.0)
    }
    pub fn pending(&self) -> bool {
        self.from.borrow().is_some() && self.progress() < 1.0
    }
    pub fn cancel(&self) -> bool {
        self.running.set(false);
        self.from.borrow_mut().take().is_some()
    }
    pub fn finish(&self) -> bool {
        self.progress() >= 1.0 && self.cancel()
    }
    pub fn command(&self, index: usize, target: PathCommand) -> PathCommand {
        self.from
            .borrow()
            .as_ref()
            .and_then(|from| from.get(index))
            .map_or(target, |from| {
                GraphicTransition::command(*from, target, self.progress())
                    .expect("validated compatible current geometry")
            })
    }
    pub fn sample(&self, target: &[PathCommand]) -> Vec<PathCommand> {
        target
            .iter()
            .enumerate()
            .map(|(index, to)| self.command(index, *to))
            .collect()
    }
}
