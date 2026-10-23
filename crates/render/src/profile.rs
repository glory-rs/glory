//! Opt-in, fixed-size inclusive phase measurements. No event/row/frame history.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Phase {
    Flush,
    Validate,
    Derive,
    Structure,
    Bind,
    Effect,
    Build,
    Window,
    Reconcile,
    Patch,
    Commit,
    Observe,
    Action,
    Resources,
    Layout,
    Paint,
    Draw,
    Task,
    Style,
    Damage,
    Construct,
    PostConstruct,
    BoxLayout,
    Transform,
    PaintTree,
    LayoutCleanup,
    Hover,
    Subdocuments,
    TextShape,
    TextBreak,
    TextAlign,
}
impl Phase {
    pub const ALL: [Self; 31] = [
        Self::Flush,
        Self::Validate,
        Self::Derive,
        Self::Structure,
        Self::Bind,
        Self::Effect,
        Self::Build,
        Self::Window,
        Self::Reconcile,
        Self::Patch,
        Self::Commit,
        Self::Observe,
        Self::Action,
        Self::Resources,
        Self::Layout,
        Self::Paint,
        Self::Draw,
        Self::Task,
        Self::Style,
        Self::Damage,
        Self::Construct,
        Self::PostConstruct,
        Self::BoxLayout,
        Self::Transform,
        Self::PaintTree,
        Self::LayoutCleanup,
        Self::Hover,
        Self::Subdocuments,
        Self::TextShape,
        Self::TextBreak,
        Self::TextAlign,
    ];
    pub const fn name(self) -> &'static str {
        match self {
            Self::Flush => "flush",
            Self::Validate => "validate",
            Self::Derive => "derive",
            Self::Structure => "structure",
            Self::Bind => "bind",
            Self::Effect => "effect",
            Self::Build => "build",
            Self::Window => "window",
            Self::Reconcile => "reconcile",
            Self::Patch => "patch",
            Self::Commit => "commit",
            Self::Observe => "observe",
            Self::Action => "action",
            Self::Resources => "resources",
            Self::Layout => "layout",
            Self::Paint => "paint",
            Self::Draw => "draw",
            Self::Task => "task",
            Self::Style => "style",
            Self::Damage => "damage",
            Self::Construct => "construct",
            Self::PostConstruct => "post_construct",
            Self::BoxLayout => "box_layout",
            Self::Transform => "transform",
            Self::PaintTree => "paint_tree",
            Self::LayoutCleanup => "layout_cleanup",
            Self::Hover => "hover",
            Self::Subdocuments => "subdocuments",
            Self::TextShape => "text_shape",
            Self::TextBreak => "text_break",
            Self::TextAlign => "text_align",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sample {
    /// Attempts, including operations ending in an error or unwind.
    pub calls: u64,
    /// Call-site work units: dependency edges, keys, mutations, or 1 per call.
    pub units: u64,
    /// Includes nested phases; never add phase totals to estimate frame time.
    pub elapsed: Duration,
    pub max: Duration,
    pub invalid_clock: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snapshot([Sample; Phase::ALL.len()]);
impl Snapshot {
    pub fn sample(&self, phase: Phase) -> Sample {
        self.0[phase as usize]
    }
    pub fn iter(&self) -> impl Iterator<Item = (Phase, Sample)> + '_ {
        Phase::ALL
            .into_iter()
            .map(|phase| (phase, self.sample(phase)))
    }
}
struct Inner {
    clock: Box<dyn Fn() -> Duration>,
    epoch: Cell<u64>,
    samples: RefCell<Snapshot>,
}
/// Clones share one local collector. Construct a separate Profile per app or
/// experiment. Clock must be lightweight/monotonic and must not reenter UI or
/// graph work. The collector never calls it while borrowing its own samples.
/// Attaching None disables instrumentation and all clock reads at call sites.
#[derive(Clone)]
pub struct Profile(Rc<Inner>);
impl Profile {
    pub fn new(clock: impl Fn() -> Duration + 'static) -> Self {
        Self(Rc::new(Inner {
            clock: Box::new(clock),
            epoch: Cell::new(0),
            samples: RefCell::new(Snapshot::default()),
        }))
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn native() -> Self {
        let start = std::time::Instant::now();
        Self::new(move || start.elapsed())
    }
    pub fn snapshot(&self) -> Snapshot {
        *self.0.samples.borrow()
    }
    /// Reset counters and retire all in-flight spans. Refuse epoch exhaustion
    /// rather than charging an ancient span to a new measurement interval.
    pub fn reset(&self) -> bool {
        let Some(epoch) = self.0.epoch.get().checked_add(1) else {
            return false;
        };
        self.0.epoch.set(epoch);
        *self.0.samples.borrow_mut() = Snapshot::default();
        true
    }
    pub fn span(&self, phase: Phase) -> Span {
        let start = (self.0.clock)();
        Span {
            profile: self.clone(),
            epoch: self.0.epoch.get(),
            phase,
            start,
            units: 1,
        }
    }
    /// Capture this interval without reading its clock. Native engines may
    /// measure on worker threads with their own monotonic clock and import
    /// fixed aggregates on the UI thread after joining that work. Reset retires
    /// these recordings just like spans; the collector remains local.
    pub fn recording(&self) -> Recording {
        Recording {
            profile: self.clone(),
            epoch: self.0.epoch.get(),
        }
    }
}
#[derive(Clone)]
pub struct Recording {
    profile: Profile,
    epoch: u64,
}
impl Recording {
    /// Merge a validated external aggregate, not a timestamp or a phase total
    /// derived by subtracting unrelated phases. Reject retired intervals and
    /// impossible counter shapes without changing samples or reading a clock.
    pub fn record(&self, phase: Phase, sample: Sample) -> bool {
        if self.epoch != self.profile.0.epoch.get()
            || sample.max > sample.elapsed
            || sample.invalid_clock > sample.calls
            || sample.calls == 0 && sample != Sample::default()
        {
            return false;
        }
        let mut snapshot = self.profile.0.samples.borrow_mut();
        let destination = &mut snapshot.0[phase as usize];
        destination.calls = destination.calls.saturating_add(sample.calls);
        destination.units = destination.units.saturating_add(sample.units);
        destination.elapsed = destination.elapsed.saturating_add(sample.elapsed);
        destination.max = destination.max.max(sample.max);
        destination.invalid_clock = destination
            .invalid_clock
            .saturating_add(sample.invalid_clock);
        true
    }
}
#[must_use]
pub struct Span {
    profile: Profile,
    epoch: u64,
    phase: Phase,
    start: Duration,
    units: u64,
}
impl Span {
    pub fn set_work(&mut self, units: u64) {
        self.units = units;
    }
    pub fn work(mut self, units: u64) -> Self {
        self.units = units;
        self
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if self.epoch != self.profile.0.epoch.get() {
            return;
        }
        let end = (self.profile.0.clock)();
        if self.epoch != self.profile.0.epoch.get() {
            return;
        }
        let mut snapshot = self.profile.0.samples.borrow_mut();
        let sample = &mut snapshot.0[self.phase as usize];
        sample.calls = sample.calls.saturating_add(1);
        sample.units = sample.units.saturating_add(self.units);
        if let Some(elapsed) = end.checked_sub(self.start) {
            sample.elapsed = sample.elapsed.saturating_add(elapsed);
            sample.max = sample.max.max(elapsed);
        } else {
            sample.invalid_clock = sample.invalid_clock.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clock() -> (Profile, Rc<Cell<u64>>) {
        let time = Rc::new(Cell::new(0));
        let value = time.clone();
        (
            Profile::new(move || Duration::from_nanos(value.get())),
            time,
        )
    }
    #[test]
    fn nested_totals_are_inclusive_and_reset_retires_live_spans() {
        let (profile, time) = clock();
        let outer = profile.span(Phase::Flush);
        time.set(10);
        let inner = profile.span(Phase::Bind).work(3);
        time.set(15);
        drop(inner);
        time.set(20);
        drop(outer);
        assert_eq!(
            profile.snapshot().sample(Phase::Flush).elapsed,
            Duration::from_nanos(20)
        );
        assert_eq!(profile.snapshot().sample(Phase::Bind).units, 3);
        assert_eq!(
            profile.snapshot().sample(Phase::Bind).elapsed,
            Duration::from_nanos(5)
        );
        let old = profile.span(Phase::Draw);
        assert!(profile.reset());
        time.set(50);
        drop(old);
        assert_eq!(profile.snapshot(), Snapshot::default());
    }
    #[test]
    fn regressing_clock_is_reported_and_unwind_releases_span() {
        let (profile, time) = clock();
        time.set(10);
        let span = profile.span(Phase::Patch);
        time.set(0);
        drop(span);
        let sample = profile.snapshot().sample(Phase::Patch);
        assert_eq!((sample.calls, sample.invalid_clock), (1, 1));
        assert_eq!(sample.elapsed, Duration::ZERO);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _span = profile.span(Phase::Build);
            panic!("fixture")
        }));
        assert_eq!(profile.snapshot().sample(Phase::Build).calls, 1);
    }
    #[test]
    fn external_aggregates_merge_without_clock_reads_and_reset_retires_recordings() {
        let reads = Rc::new(Cell::new(0));
        let called = reads.clone();
        let profile = Profile::new(move || {
            called.set(called.get() + 1);
            Duration::ZERO
        });
        let recording = profile.recording();
        let sample = Sample {
            calls: 2,
            units: 7,
            elapsed: Duration::from_nanos(10),
            max: Duration::from_nanos(6),
            invalid_clock: 0,
        };
        assert!(recording.record(Phase::TextShape, sample));
        assert!(recording.record(Phase::TextShape, sample));
        let value = profile.snapshot().sample(Phase::TextShape);
        assert_eq!(
            (value.calls, value.units, value.elapsed, value.max),
            (4, 14, Duration::from_nanos(20), Duration::from_nanos(6))
        );
        assert!(profile.reset());
        assert!(!recording.record(Phase::TextShape, sample));
        assert_eq!(profile.snapshot(), Snapshot::default());
        assert_eq!(reads.get(), 0);
    }
    #[test]
    fn impossible_external_samples_do_not_change_the_collector() {
        let (profile, _) = clock();
        let recording = profile.recording();
        for sample in [
            Sample {
                units: 1,
                ..Sample::default()
            },
            Sample {
                calls: 1,
                max: Duration::from_nanos(2),
                elapsed: Duration::from_nanos(1),
                ..Sample::default()
            },
            Sample {
                calls: 1,
                invalid_clock: 2,
                ..Sample::default()
            },
        ] {
            assert!(!recording.record(Phase::Style, sample));
            assert_eq!(profile.snapshot(), Snapshot::default());
        }
        assert!(recording.record(Phase::Style, Sample::default()));
    }
}
