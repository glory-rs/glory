use std::cell::{Cell, RefCell};
use std::rc::Rc;

use glory_core::{BindingId, Owner, Runtime};

fn observe(owner: &Owner, phase: usize, callback: impl FnMut() + 'static) -> BindingId {
    match phase {
        0 => owner.structural(callback),
        1 => owner.binding(callback),
        _ => owner.effect(callback),
    }
}
fn bound(runtime: &Runtime) {
    let stats = runtime.stats();
    assert!(stats.queue_entries <= stats.queued * 2 + 64);
    if stats.queued == 0 {
        assert_eq!(stats.queue_entries, 0);
    }
}

#[test]
fn small_retirements_do_not_scan_unrelated_ready_work_or_delay_capture_destruction() {
    struct Capture(Rc<Runtime>, Rc<Cell<usize>>, Rc<Vec<u8>>);
    impl Drop for Capture {
        fn drop(&mut self) {
            bound(&self.0); // No graph borrow may survive into this destructor.
            assert_eq!(self.2.len(), 1_024);
            self.1.set(self.1.get() + 1);
        }
    }
    let runtime = Rc::new(Runtime::new());
    let global = runtime.owner();
    let called = Rc::new(Cell::new(0));
    for _ in 0..4_096 {
        let out = called.clone();
        global.binding(move || out.set(out.get() + 1));
    }
    let drops = Rc::new(Cell::new(0));
    for _ in 0..16 {
        let row = runtime.owner();
        let payload = Rc::new(vec![1_u8; 1_024]);
        let weak = Rc::downgrade(&payload);
        let capture = Capture(runtime.clone(), drops.clone(), payload);
        row.binding(move || {
            std::hint::black_box(&capture);
            panic!("retired observer");
        });
        row.cage(7);
        row.dispose();
        assert!(weak.upgrade().is_none());
        bound(&runtime);
    }
    assert_eq!(drops.get(), 16);
    let stats = runtime.stats();
    assert_eq!(
        (stats.nodes, stats.queued, stats.queue_entries),
        (4_096, 4_096, 4_112)
    );
    assert_eq!(
        (
            stats.queue_compactions,
            stats.queue_compaction_visits,
            called.get()
        ),
        (0, 0, 0)
    );
    runtime.flush().unwrap();
    assert_eq!(called.get(), 4_096);
    bound(&runtime);
}

#[test]
fn retired_generations_cannot_execute_reused_slots_or_change_phase_and_fifo_order() {
    let runtime = Runtime::new();
    let global = runtime.owner();
    let trace = Rc::new(RefCell::new(Vec::new()));
    for phase in [2, 1, 0] {
        let out = trace.clone();
        observe(&global, phase, move || out.borrow_mut().push((phase, 0)));
    }
    let retired = runtime.owner();
    for phase in 0..3 {
        observe(&retired, phase, || panic!("old generation must never run"));
    }
    retired.dispose();
    assert!(runtime.stats().queue_entries > runtime.stats().queued);
    let replacement = runtime.owner();
    for phase in [2, 0, 1] {
        let out = trace.clone();
        observe(&replacement, phase, move || {
            out.borrow_mut().push((phase, 1))
        });
    }
    assert_eq!(
        runtime.stats().node_slots,
        6,
        "reuse all three retired slots"
    );
    runtime.flush().unwrap();
    assert_eq!(
        &*trace.borrow(),
        &[(0, 0), (0, 1), (1, 0), (1, 1), (2, 0), (2, 1)]
    );
    bound(&runtime);
}

#[test]
fn long_retirement_history_compacts_without_retaining_rows_or_reordering_live_work() {
    let runtime = Runtime::new();
    let global = runtime.owner();
    let trace = Rc::new(RefCell::new(Vec::new()));
    for index in 0..8 {
        for phase in [2, 1, 0] {
            let out = trace.clone();
            observe(&global, phase, move || {
                out.borrow_mut().push((phase, index))
            });
        }
    }
    for _ in 0..5_000 {
        let row = runtime.owner();
        for phase in 0..3 {
            observe(&row, phase, || panic!("retired row"));
        }
        row.dispose();
        bound(&runtime);
        assert_eq!((runtime.stats().nodes, runtime.stats().queued), (24, 24));
        assert!(runtime.stats().node_slots <= 27 && runtime.stats().queue_capacity <= 512);
    }
    let stats = runtime.stats();
    assert!(stats.queue_compactions > 0 && stats.queue_compaction_visits < 100_000);
    runtime.flush().unwrap();
    let expected: Vec<_> = (0..3)
        .flat_map(|phase| (0..8).map(move |index| (phase, index)))
        .collect();
    assert_eq!(&*trace.borrow(), &expected);
    bound(&runtime);
}

#[test]
fn draining_live_work_maintains_the_space_bound_before_entering_callbacks() {
    let runtime = Rc::new(Runtime::new());
    let global = runtime.owner();
    let trace = Rc::new(RefCell::new(Vec::new()));
    for index in 0..100 {
        let runtime = runtime.clone();
        let out = trace.clone();
        global.binding(move || {
            bound(&runtime);
            out.borrow_mut().push(index);
        });
    }
    for _ in 0..125 {
        let row = runtime.owner();
        row.effect(|| panic!("dead effect queued after live bindings"));
        row.dispose();
    }
    assert_eq!(
        (runtime.stats().queued, runtime.stats().queue_entries),
        (100, 225)
    );
    assert_eq!(runtime.stats().queue_compactions, 0);
    runtime.flush().unwrap();
    assert_eq!(&*trace.borrow(), &(0..100).collect::<Vec<_>>());
    assert!(runtime.stats().queue_compactions > 0);
    bound(&runtime);
}

#[test]
fn an_effect_queued_again_during_its_callback_can_retire_without_being_resurrected() {
    let runtime = Runtime::new();
    let global = runtime.owner();
    let value = global.cage(0);
    let trace = Rc::new(RefCell::new(Vec::new()));
    let out = trace.clone();
    global.binding(move || out.borrow_mut().push(value.get()));
    let row = Rc::new(runtime.owner());
    let weak = Rc::downgrade(&row);
    let called = Rc::new(Cell::new(0));
    let runs = called.clone();
    row.effect(move || {
        value.get();
        runs.set(runs.get() + 1);
        value.revise(|v| *v += 1);
        weak.upgrade().unwrap().dispose();
    });
    runtime.flush().unwrap();
    assert_eq!((called.get(), value.get()), (1, 1));
    assert_eq!(&*trace.borrow(), &[0, 1]);
    bound(&runtime);
    value.revise(|v| *v = 2);
    runtime.flush().unwrap();
    assert_eq!(called.get(), 1);
    assert_eq!(&*trace.borrow(), &[0, 1, 2]);
    bound(&runtime);
}
