use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use glory_core::{AccessError, Bond, Cage, Host, HostError, LocalFuture, Runtime, untracked};
use glory_render::profile::{Phase, Profile};

#[test]
fn instance_entry_checks_generation_and_runtime_before_business_code_without_retaining_ownership() {
    let runtime = Runtime::new();
    let global = runtime.owner();
    let source = global.cage((1usize,));
    let instance = runtime.scope();
    let old = instance.id();
    let field = runtime
        .enter(old, || {
            source.field("value", |value| &value.0, |value| &mut value.0)
        })
        .unwrap();
    drop(instance);
    assert!(!field.is_alive());
    let replacement = runtime.scope();
    assert_ne!(old, replacement.id());
    let called = Cell::new(false);
    assert_eq!(
        runtime.enter(old, || called.set(true)),
        Err(AccessError::Disposed)
    );
    let other = Runtime::new();
    let foreign = other.scope();
    assert_eq!(
        runtime.enter(foreign.id(), || called.set(true)),
        Err(AccessError::WrongRuntime)
    );
    assert!(!called.get());
    let next = runtime
        .enter(replacement.id(), || {
            source.field("value", |value| &value.0, |value| &mut value.0)
        })
        .unwrap();
    assert_eq!(next.get(), 1);
    drop(replacement);
    assert!(!next.is_alive());
    assert!(source.is_alive());
}

#[test]
fn nested_runtime_entries_restore_the_previous_owner_after_foreign_context_and_panic() {
    let a = Runtime::new();
    let b = Runtime::new();
    let a_root = a.owner();
    let b_root = b.owner();
    let a_value = a_root.cage((1usize,));
    let b_value = b_root.cage((2usize,));
    let a_scope = a.scope();
    let b_scope = b.scope();
    let (owned_a, fallback_a, owned_b, restored_a, fallback_b) = a
        .enter(a_scope.id(), || {
            let owned_a = a_value.field("value", |v| &v.0, |v| &mut v.0);
            let (fallback_a, owned_b) = b
                .enter(b_scope.id(), || {
                    (
                        a_value.field("value", |v| &v.0, |v| &mut v.0),
                        b_value.field("value", |v| &v.0, |v| &mut v.0),
                    )
                })
                .unwrap();
            let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                b.enter(b_scope.id(), || panic!("nested owner unwind"))
                    .unwrap();
            }));
            assert!(failed.is_err());
            let restored_a = a_value.field("value", |v| &v.0, |v| &mut v.0);
            let fallback_b = b_value.map(|v| v.0);
            (owned_a, fallback_a, owned_b, restored_a, fallback_b)
        })
        .unwrap();
    drop(a_scope);
    drop(b_scope);
    assert!(!owned_a.is_alive() && !restored_a.is_alive() && !owned_b.is_alive());
    assert_eq!(fallback_a.get(), 1);
    assert_eq!(fallback_b.get(), 2);
    drop(a_root);
    drop(b_root);
    assert!(!fallback_a.is_alive() && !fallback_b.is_alive());
}

#[test]
fn effect_and_structural_callback_creations_follow_the_registered_owner_not_the_flush_caller() {
    for structural in [false, true] {
        let runtime = Runtime::new();
        let global = runtime.owner();
        let source = global.cage((0usize,));
        let trigger = global.cage(0usize);
        let row = runtime.owner();
        let caller = runtime.owner();
        let saved = Rc::new(Cell::new(None));
        let out = saved.clone();
        let callback = move || {
            let tick = trigger.get();
            let field = source.field("value", |value| &value.0, |value| &mut value.0);
            let derived = if let Some((_, derived)) = out.get() {
                derived
            } else {
                field.map(|value| *value)
            };
            out.set(Some((field, derived)));
            assert_eq!(field.get(), 0);
            assert_eq!(derived.get(), 0);
            std::hint::black_box(tick);
        };
        if structural {
            row.structural(callback);
        } else {
            row.effect(callback);
        }
        for _ in 0..1000 {
            caller.enter(|| runtime.flush()).unwrap();
            trigger.revise(|value| *value += 1);
        }
        let (field, derived) = saved.get().unwrap();
        assert_eq!(runtime.stats().nodes, 5);
        drop(row);
        assert!(!field.is_alive() && !derived.is_alive());
        assert_eq!((runtime.stats().nodes, runtime.stats().fields), (2, 0));
        assert!(source.is_alive() && trigger.is_alive());
    }
}

#[test]
fn short_lived_effects_do_not_leave_visited_map_key_paths_in_the_source_owner() {
    let runtime = Runtime::new();
    let global = runtime.owner();
    let rows = global.cage(std::collections::HashMap::<usize, usize>::new());
    for index in 0..1000 {
        let owner = runtime.owner();
        let saved = Rc::new(Cell::new(None));
        let out = saved.clone();
        owner.effect(move || out.set(Some(rows.entry(index))));
        runtime.flush().unwrap();
        let field = saved.get().unwrap();
        assert_eq!(field.try_get(), Err(AccessError::MissingField));
        drop(owner);
        assert!(!field.is_alive());
        let stats = runtime.stats();
        assert_eq!(
            (
                stats.nodes,
                stats.fields,
                stats.entry_keys,
                stats.entry_buckets
            ),
            (1, 0, 0, 0)
        );
        assert!(stats.node_slots <= 3 && stats.field_slots <= 1);
    }
}

#[test]
fn dependency_value_retirement_cancels_an_effect_before_its_captured_callback_runs() {
    struct Value {
        _owner: Option<glory_core::Owner>,
    }
    let runtime = Runtime::new();
    let global = runtime.owner();
    let trigger = global.cage(0usize);
    let child = runtime.owner();
    let marker = child.cage(());
    let holder = Rc::new(RefCell::new(None));
    let resource = holder.clone();
    let dependency = global.bond(move || {
        trigger.get();
        Value {
            _owner: resource.borrow_mut().take(),
        }
    });
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    child.effect(move || {
        dependency.with(|_| ());
        observed.set(observed.get() + 1);
    });
    *holder.borrow_mut() = Some(child);
    runtime.flush().unwrap();
    assert!(marker.is_alive());
    assert_eq!(calls.get(), 1);
    trigger.revise(|value| *value += 1);
    runtime.flush().unwrap();
    assert!(!marker.is_alive());
    assert_eq!(
        calls.get(),
        1,
        "retired effect must not execute the old callback snapshot"
    );
}

#[test]
fn profile_separates_equality_validation_from_binding_and_can_stop_clock_reads() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let reads = Rc::new(Cell::new(0u64));
    let clock = reads.clone();
    let profile = Profile::new(move || {
        let next = clock.get() + 1;
        clock.set(next);
        Duration::from_nanos(next)
    });
    assert_eq!(reads.get(), 0);
    runtime.set_profile(Some(profile.clone()));
    let source = owner.cage(1);
    let parity = owner.bond(move || source.get() % 2).with_partial_eq();
    let seen = Rc::new(Cell::new(0));
    let result = seen.clone();
    owner.binding(move || {
        parity.get();
        result.set(result.get() + 1)
    });
    runtime.flush().unwrap();
    assert_eq!(seen.get(), 1);
    assert!(profile.reset());
    source.revise(|v| *v = 3);
    runtime.flush().unwrap();
    let snapshot = profile.snapshot();
    assert_eq!(snapshot.sample(Phase::Validate).calls, 2);
    assert_eq!(snapshot.sample(Phase::Derive).calls, 1);
    assert_eq!(snapshot.sample(Phase::Bind).calls, 0);
    assert_eq!(seen.get(), 1);
    runtime.set_profile(None);
    let before = reads.get();
    source.revise(|v| *v = 4);
    runtime.flush().unwrap();
    assert_eq!(reads.get(), before);
    assert_eq!(profile.snapshot(), snapshot);
    assert_eq!(seen.get(), 2);
}

#[test]
fn direct_dependencies_gate_consumers_on_derived_equality() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let count = owner.cage(1);
    let parity = owner.bond(move || count.get() % 2).with_partial_eq();
    let parity_seen = Rc::new(RefCell::new(Vec::new()));
    let count_seen = Rc::new(RefCell::new(Vec::new()));
    let seen = parity_seen.clone();
    owner.binding(move || seen.borrow_mut().push(parity.get()));
    let seen = count_seen.clone();
    owner.binding(move || seen.borrow_mut().push(count.get()));
    runtime.flush().unwrap();
    assert_eq!(runtime.stats().edges, 3); // source -> bond -> binding, plus source -> binding
    count.revise(|value| *value = 3);
    runtime.flush().unwrap();
    assert_eq!(&*parity_seen.borrow(), &[1]);
    assert_eq!(&*count_seen.borrow(), &[1, 3]);
    assert_eq!(runtime.stats().derived_runs, 2);
    assert_eq!(runtime.stats().binding_runs, 3);
    count.revise(|value| *value = 4);
    runtime.flush().unwrap();
    assert_eq!(&*parity_seen.borrow(), &[1, 0]);
}

#[test]
fn dynamic_dependencies_are_replaced_and_diamonds_never_glitch() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let flag = owner.cage(true);
    let a = owner.cage(10);
    let b = owner.cage(20);
    let chosen = owner.bond(move || if flag.get() { a.get() } else { b.get() });
    let doubled = owner.bond(move || chosen.get() * 2);
    let result = owner.bond(move || chosen.get() + doubled.get());
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    owner.binding(move || output.borrow_mut().push(result.get()));
    runtime.flush().unwrap();
    flag.revise(|flag| *flag = false);
    runtime.flush().unwrap();
    let runs = runtime.stats().derived_runs;
    a.revise(|a| *a = 999);
    runtime.flush().unwrap();
    assert_eq!(runtime.stats().derived_runs, runs);
    runtime
        .batch(|| {
            b.revise(|b| *b = 30);
            b.revise(|b| *b = 40);
        })
        .unwrap();
    assert_eq!(&*seen.borrow(), &[30, 60, 120]);
}

#[test]
fn default_bonds_accept_non_comparable_values_and_custom_comparators() {
    struct Value(i32);
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(1);
    let value = owner
        .bond(move || Value(source.get() % 2))
        .with_eq(|a, b| a.0 == b.0);
    let calls = Rc::new(Cell::new(0));
    let seen = calls.clone();
    owner.binding(move || {
        value.with(|v| assert_eq!(v.0, 1));
        seen.set(seen.get() + 1);
    });
    runtime.flush().unwrap();
    source.revise(|v| *v = 3);
    runtime.flush().unwrap();
    assert_eq!(calls.get(), 1);
}

#[test]
fn bare_and_untracked_reads_do_not_subscribe_and_large_reads_do_not_clone() {
    struct Large(Vec<u64>); // Deliberately not Clone.
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let data = owner.cage(Large((0..1_000_000).collect()));
    let tick = owner.cage(0);
    assert_eq!(data.with(|data| data.0[999_999]), 999_999);
    assert_eq!(runtime.stats().edges, 0);
    let calls = Rc::new(Cell::new(0));
    let seen = calls.clone();
    owner.binding(move || {
        tick.get();
        untracked(|| data.with(|data| data.0[42]));
        seen.set(seen.get() + 1);
    });
    runtime.flush().unwrap();
    data.revise(|data| data.0[42] = 100);
    runtime.flush().unwrap();
    assert_eq!(calls.get(), 1);
    tick.revise(|tick| *tick += 1);
    runtime.flush().unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(runtime.stats().edges, 1);
}

#[test]
fn owners_instances_tasks_and_subscriptions_are_recycled_with_the_window() {
    let runtime = Runtime::new();
    let root = runtime.scope();
    let shared = root.cage(0);
    for _ in 0..10_000 {
        let row = root.child();
        let local = row.cage(String::from("row"));
        let derived = row.bond(move || shared.get());
        let binding = row.binding(move || {
            derived.get();
            local.with(|v| assert_eq!(v, "row"));
        });
        let task = row.task();
        let token = task.token();
        assert_eq!(row.parent(), Some(root.id()));
        runtime.flush().unwrap();
        drop(row);
        assert!(!local.is_alive());
        assert!(!binding.is_alive());
        assert!(!token.is_current());
        assert_eq!(local.try_get(), Err(AccessError::Disposed));
        drop(task);
    }
    let stats = runtime.stats();
    assert_eq!(
        (
            stats.nodes,
            stats.owners,
            stats.instances,
            stats.tasks,
            stats.edges,
            stats.queued
        ),
        (1, 2, 1, 0, 0, 0)
    );
    assert!(stats.node_slots <= 4);
    assert!(stats.owner_slots <= 3);
    assert!(stats.instance_slots <= 2);
    assert!(stats.task_slots <= 1);
    drop(root);
    assert_eq!((runtime.stats().nodes, runtime.stats().instances), (0, 0));
}

#[test]
fn disposing_before_flush_removes_pending_history_and_old_handles_cannot_alias() {
    let runtime = Runtime::new();
    let mut previous = None::<Cage<i32>>;
    for _ in 0..10_000 {
        let owner = runtime.owner();
        let signal = owner.cage(1);
        owner.binding(move || {
            signal.get();
        });
        if let Some(previous) = previous {
            assert!(!previous.is_alive());
            assert_eq!(previous.try_get(), Err(AccessError::Disposed));
        }
        drop(owner);
        previous = Some(signal);
    }
    assert_eq!(runtime.stats().queued, 0);
    assert!(runtime.stats().node_slots <= 2);
    runtime.flush().unwrap();
}

#[test]
fn runtimes_are_isolated_and_cross_runtime_tracking_is_rejected() {
    let a = Runtime::new();
    let a_owner = a.owner();
    let x = a_owner.cage(1);
    let b = Runtime::new();
    let b_owner = b.owner();
    let y = b_owner.cage(2);
    let errors = Rc::new(RefCell::new(Vec::new()));
    let output = errors.clone();
    b_owner.binding(move || {
        y.get();
        output.borrow_mut().push(x.try_get());
    });
    b.flush().unwrap();
    assert_eq!(&*errors.borrow(), &[Err(AccessError::WrongRuntime)]);
    x.revise(|v| *v = 9);
    a.flush().unwrap();
    b.flush().unwrap();
    assert_eq!(errors.borrow().len(), 1);
    drop(a);
    assert_eq!(x.try_get(), Err(AccessError::RuntimeGone));
    assert_eq!(y.get(), 2);
}

#[test]
fn pure_computations_cannot_write_even_when_untracked() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let value = owner.cage(1);
    let computed = owner.bond(move || untracked(|| value.try_revise(|v| *v += 1)));
    assert_eq!(computed.get(), Err(AccessError::WriteDuringCompute));
    assert_eq!(value.get(), 1);
}

#[test]
fn panic_unwinds_tracking_and_retries_a_binding_without_poisoning_the_runtime() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let value = owner.cage(1);
    let fail = Rc::new(Cell::new(true));
    let trigger = fail.clone();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    owner.binding(move || {
        let value = value.get();
        if trigger.replace(false) {
            panic!("first evaluation");
        }
        output.borrow_mut().push(value);
    });
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush())).is_err());
    runtime.flush().unwrap();
    value.revise(|v| *v = 2);
    runtime.flush().unwrap();
    assert_eq!(&*seen.borrow(), &[1, 2]);
    assert_eq!(runtime.stats().edges, 1);
}

#[test]
fn partial_writes_that_panic_still_invalidate_observers() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let value = owner.cage(1);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    owner.binding(move || output.borrow_mut().push(value.get()));
    runtime.flush().unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| value.revise(|v| {
            *v = 7;
            panic!("partial mutation");
        })))
        .is_err()
    );
    runtime.flush().unwrap();
    assert_eq!(&*seen.borrow(), &[1, 7]);
}

#[test]
fn effects_run_after_bindings_and_writes_stabilize_in_another_pass() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(0);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    owner.binding(move || output.borrow_mut().push(source.get()));
    owner.effect(move || {
        if source.get() < 3 {
            source.revise(|v| *v += 1);
        }
    });
    runtime.flush().unwrap();
    assert_eq!(&*seen.borrow(), &[0, 1, 2, 3]);
}

#[test]
fn recursive_bonds_report_cycles_without_corrupting_tracking() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let slot = Rc::new(Cell::new(None::<Bond<Result<i32, AccessError>>>));
    let read = slot.clone();
    let bond = owner.bond(move || read.get().unwrap().try_get().and_then(|value| value));
    slot.set(Some(bond));
    assert_eq!(bond.get(), Err(AccessError::Cycle));
    let other = owner.cage(7);
    assert_eq!(other.get(), 7);
}

#[test]
fn cancellation_revokes_late_commit_and_completion_does_not_cancel() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let value = owner.cage(0);
    let cancelled = Rc::new(Cell::new(0));
    let count = cancelled.clone();
    let task = owner.task();
    let token = task.token();
    task.on_cancel(move || count.set(count.get() + 1)).unwrap();
    owner.dispose();
    assert_eq!(cancelled.get(), 1);
    assert_eq!(
        token.commit(|| value.revise(|v| *v = 1)),
        Err(AccessError::Disposed)
    );
    drop(task);
    assert_eq!(cancelled.get(), 1);
    let next = runtime.owner();
    let task = next.task();
    let token = task.token();
    let count = cancelled.clone();
    task.on_cancel(move || count.set(count.get() + 1)).unwrap();
    task.complete();
    assert!(!token.is_current());
    assert_eq!(cancelled.get(), 1);
}

#[derive(Default)]
struct Scheduled {
    future: RefCell<Option<LocalFuture>>,
    cancelled: Cell<bool>,
}
#[derive(Default)]
struct TestHost {
    tasks: RefCell<Vec<Rc<Scheduled>>>,
    wakes: Cell<usize>,
    cancellations: Rc<Cell<usize>>,
}
impl TestHost {
    fn tick(&self) {
        let mut context = Context::from_waker(Waker::noop());
        let tasks = std::mem::take(&mut *self.tasks.borrow_mut());
        for task in tasks {
            let future = task.future.borrow_mut().take();
            if let Some(mut future) = future
                && !task.cancelled.get()
                && future.as_mut().poll(&mut context) == Poll::Pending
                && !task.cancelled.get()
            {
                *task.future.borrow_mut() = Some(future);
                self.tasks.borrow_mut().push(task);
            }
        }
    }
}
impl Host for TestHost {
    fn spawn(&self, future: LocalFuture) -> Result<Box<dyn FnOnce()>, HostError> {
        let task = Rc::new(Scheduled {
            future: RefCell::new(Some(future)),
            cancelled: Cell::new(false),
        });
        self.tasks.borrow_mut().push(task.clone());
        let count = self.cancellations.clone();
        Ok(Box::new(move || {
            task.cancelled.set(true);
            task.future.borrow_mut().take();
            count.set(count.get() + 1);
        }))
    }
    fn wake(&self) {
        self.wakes.set(self.wakes.get() + 1);
    }
    fn now(&self) -> Duration {
        Duration::from_secs(1)
    }
}

#[test]
fn hosts_schedule_without_blocking_and_owner_disposal_really_drops_futures() {
    let host = Rc::new(TestHost::default());
    let runtime = Runtime::with_host(host.clone());
    let owner = runtime.owner();
    let value = owner.cage(0);
    let token = owner
        .spawn(async move {
            value.revise(|v| *v = 4);
        })
        .unwrap();
    assert_eq!(value.get(), 0);
    assert!(token.is_current());
    host.tick();
    assert_eq!(value.get(), 4);
    assert!(!token.is_current());
    assert_eq!(host.cancellations.get(), 0);
    assert_eq!(host.wakes.get(), 1);
    assert_eq!(runtime.now(), Some(Duration::from_secs(1)));
    struct Dropped(Rc<Cell<bool>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let dropped = Rc::new(Cell::new(false));
    let guard = Dropped(dropped.clone());
    let token = owner
        .spawn(async move {
            let _guard = guard;
            std::future::pending::<()>().await;
        })
        .unwrap();
    host.tick();
    assert!(!dropped.get());
    owner.dispose();
    assert!(dropped.get());
    assert!(!token.is_current());
    assert_eq!(host.cancellations.get(), 1);
    assert_eq!(runtime.stats().tasks, 0);
}

#[test]
fn nested_batches_and_unwinding_do_not_render_intermediate_values() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let value = owner.cage(0);
    let values = Rc::new(RefCell::new(Vec::new()));
    let output = values.clone();
    owner.binding(move || output.borrow_mut().push(value.get()));
    runtime.flush().unwrap();
    runtime
        .batch(|| {
            value.revise(|v| *v = 1);
            runtime.batch(|| value.revise(|v| *v = 2)).unwrap();
            runtime.flush().unwrap();
            assert_eq!(&*values.borrow(), &[0]);
            value.revise(|v| *v = 3);
        })
        .unwrap();
    assert_eq!(&*values.borrow(), &[0, 3]);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.batch(|| {
            value.revise(|v| *v = 4);
            panic!("abort action");
        })))
        .is_err()
    );
    runtime.flush().unwrap();
    assert_eq!(&*values.borrow(), &[0, 3, 4]);
}

#[test]
fn all_cancellation_hooks_run_once_and_never_at_registration_time() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let task = owner.task();
    let calls = Rc::new(RefCell::new(Vec::new()));
    for value in 0..3 {
        let calls = calls.clone();
        task.on_cancel(move || calls.borrow_mut().push(value))
            .unwrap();
    }
    assert!(calls.borrow().is_empty());
    task.token().cancel();
    assert_eq!(&*calls.borrow(), &[0, 1, 2]);
    drop(task);
    owner.dispose();
    assert_eq!(&*calls.borrow(), &[0, 1, 2]);
}

#[test]
fn no_executor_fallback_blocks_the_calling_thread() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    assert!(matches!(
        owner.spawn(std::future::pending()),
        Err(AccessError::NoHost)
    ));
    assert_eq!(runtime.stats().tasks, 0);
}

#[test]
fn an_effect_loop_returns_a_diagnostic_and_can_be_disposed() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let count = owner.cage(0_u64);
    owner.effect(move || {
        count.get();
        count.revise(|v| *v += 1);
    });
    assert_eq!(runtime.flush(), Err(AccessError::RunawayEffect));
    owner.dispose();
    runtime.flush().unwrap();
    assert_eq!(runtime.stats().nodes, 0);
}

#[test]
fn cleanup_callbacks_can_reenter_the_runtime_after_identities_are_invalidated() {
    let runtime = Rc::new(Runtime::new());
    let owner = runtime.owner();
    let outside = runtime.owner();
    let value = outside.cage(0);
    let local = owner.cage(1);
    let runtime_for_cleanup = runtime.clone();
    owner
        .on_cleanup(move || {
            assert_eq!(local.try_get(), Err(AccessError::Disposed));
            let temporary = runtime_for_cleanup.owner();
            assert_eq!(temporary.cage(2).get(), 2);
            value.revise(|value| *value += 1);
        })
        .unwrap();
    owner.dispose();
    assert_eq!(value.get(), 1);
    drop(owner);
    assert_eq!(value.get(), 1);
}

#[test]
fn cleanup_cannot_register_implicit_resources_in_the_disposer_or_model_owner() {
    let runtime = Runtime::new();
    let global = runtime.owner();
    let model = global.cage((1usize,));
    let caller = runtime.owner();
    for _ in 0..1_000 {
        let retiring = runtime.owner();
        retiring
            .on_cleanup(move || {
                assert!(matches!(
                    model.try_field("value", |v| &v.0, |v| &mut v.0),
                    Err(AccessError::Disposed)
                ));
                // Reads/writes remain usable; only implicit construction must
                // not fall back to an unrelated live Owner.
                model.revise(|v| v.0 += 1);
            })
            .unwrap();
        caller.enter(|| retiring.dispose());
        assert_eq!((runtime.stats().nodes, runtime.stats().fields), (1, 0));
    }
    assert_eq!(model.with(|v| v.0), 1_001);
}

#[test]
fn cleanup_reads_do_not_subscribe_the_effect_that_disposes_an_owner() {
    let runtime = Runtime::new();
    let global = runtime.owner();
    let unrelated = global.cage(1);
    let trigger = global.cage(0);
    let retiring = runtime.owner();
    retiring
        .on_cleanup(move || assert_eq!(unrelated.get(), 1))
        .unwrap();
    let mut retiring = Some(retiring);
    let calls = Rc::new(Cell::new(0));
    let runs = calls.clone();
    global.effect(move || {
        trigger.get();
        runs.set(runs.get() + 1);
        if let Some(owner) = retiring.take() {
            owner.dispose();
        }
    });
    runtime.flush().unwrap();
    unrelated.revise(|v| *v = 2);
    runtime.flush().unwrap();
    assert_eq!(calls.get(), 1);
    trigger.revise(|v| *v += 1);
    runtime.flush().unwrap();
    assert_eq!(calls.get(), 2);
}

#[test]
fn task_cancellation_uses_its_owner_for_both_live_and_disposed_tasks() {
    for dispose in [false, true] {
        let runtime = Runtime::new();
        let global = runtime.owner();
        let model = global.cage((1usize,));
        let task_owner = runtime.owner();
        let caller = runtime.owner();
        let task = task_owner.task();
        let result = Rc::new(RefCell::new(None));
        let out = result.clone();
        let calls = Rc::new(Cell::new(0));
        let runs = calls.clone();
        task.on_cancel(move || {
            runs.set(runs.get() + 1);
            *out.borrow_mut() = Some(model.try_field("value", |v| &v.0, |v| &mut v.0));
        })
        .unwrap();
        let token = task.detach();
        caller.enter(|| {
            if dispose {
                task_owner.dispose();
            } else {
                token.cancel();
            }
        });
        caller.dispose();
        let selected = result.borrow_mut().take().unwrap();
        if dispose {
            assert!(matches!(selected, Err(AccessError::Disposed)));
        } else {
            let selected = selected.unwrap();
            assert!(selected.is_alive());
            task_owner.dispose();
            assert!(!selected.is_alive());
        }
        token.cancel();
        assert_eq!(calls.get(), 1);
        assert_eq!((runtime.stats().tasks, runtime.stats().fields), (0, 0));
    }
}

#[test]
fn cleanup_entry_is_restored_after_nested_foreign_entry_and_panic() {
    let runtime = Runtime::new();
    let foreign = Rc::new(Runtime::new());
    let foreign_owner = Rc::new(foreign.owner());
    let foreign_model = foreign_owner.cage(1);
    let global = runtime.owner();
    let model = global.cage((1usize,));
    let caller = runtime.owner();
    let retiring = runtime.owner();
    let owner = foreign_owner.clone();
    retiring
        .on_cleanup(move || {
            owner.enter(|| assert_eq!(foreign_model.map(|v| v + 1).get(), 2));
            assert!(matches!(
                model.try_field("value", |v| &v.0, |v| &mut v.0),
                Err(AccessError::Disposed)
            ));
            panic!("cleanup unwind");
        })
        .unwrap();
    let field = caller.enter(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| retiring.dispose())).is_err()
        );
        model.field("value", |v| &v.0, |v| &mut v.0)
    });
    assert!(field.is_alive());
    caller.dispose();
    assert!(!field.is_alive());
    assert_eq!(foreign.stats().nodes, 2);
}

#[test]
fn large_numbers_of_independent_bindings_are_not_mistaken_for_a_feedback_loop() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let calls = Rc::new(Cell::new(0));
    for _ in 0..100_001 {
        let calls = calls.clone();
        owner.binding(move || calls.set(calls.get() + 1));
    }
    runtime.flush().unwrap();
    assert_eq!(calls.get(), 100_001);
    assert_eq!(runtime.stats().queued, 0);
}

#[test]
fn values_are_dropped_promptly_without_borrowing_the_graph_or_waiting_for_copy_handles() {
    struct Dropped(Cage<usize>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.revise(|count| *count += 1);
        }
    }
    let runtime = Runtime::new();
    let outside = runtime.owner();
    let drops = outside.cage(0);
    let owner = runtime.owner();
    let value = owner.cage(Dropped(drops));
    assert!(value.is_alive());
    owner.dispose();
    assert_eq!(drops.get(), 1);
    assert!(!value.is_alive());
    assert_eq!(value.try_with(|_| ()), Err(AccessError::Disposed));
    drop(owner);
    assert_eq!(drops.get(), 1);
}

#[test]
fn construction_context_owns_maps_of_global_sources_and_unwinds_after_panics() {
    let runtime = Runtime::new();
    let root = runtime.owner();
    let source = root.cage(1);
    for _ in 0..1000 {
        let row = root.child();
        let mapped = row.enter(|| source.map(|value| value + 1));
        assert_eq!(mapped.get(), 2);
        drop(row);
        assert!(!mapped.is_alive());
        assert_eq!(runtime.stats().nodes, 1);
        assert!(runtime.stats().node_slots <= 2);
    }
    let row = root.child();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || row.enter(|| panic!("factory error"))
        ))
        .is_err()
    );
    let global = source.map(|value| *value);
    drop(row);
    assert!(global.is_alive());
}

#[test]
fn async_polling_and_token_commits_use_the_task_owner_for_derived_maps() {
    let host = Rc::new(TestHost::default());
    let runtime = Runtime::with_host(host.clone());
    let root = runtime.owner();
    let source = root.cage(1);
    let row = root.child();
    let mapped = Rc::new(Cell::new(None));
    let result = mapped.clone();
    row.spawn(async move {
        result.set(Some(source.map(|value| value + 1)));
        std::future::pending::<()>().await;
    })
    .unwrap();
    host.tick();
    let derived = mapped.get().unwrap();
    assert!(derived.is_alive());
    let task = row.task();
    let another = task
        .token()
        .commit(|| source.map(|value| value + 2))
        .unwrap();
    assert_eq!(another.get(), 3);
    row.dispose();
    assert!(!derived.is_alive());
    assert!(!another.is_alive());
    assert!(source.is_alive());
    assert_eq!(runtime.stats().nodes, 1);
}

#[test]
fn pure_computations_cannot_allocate_a_new_reactive_graph_on_every_evaluation() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(1);
    let invalid = owner.bond(move || source.map(|value| *value).get());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| invalid.get())).is_err());
    assert_eq!(runtime.stats().nodes, 2);
}

#[test]
fn a_write_burst_propagates_each_pending_diamond_once_without_cloning_the_source() {
    struct Large {
        count: usize,
        bytes: Vec<u8>,
    }
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(Large {
        count: 0,
        bytes: vec![7; 1_048_576],
    });
    let pointer = source.with(|value| value.bytes.as_ptr());
    let root = source.map(|value| value.count % 2).with_partial_eq();
    let calls = Rc::new(Cell::new(0));
    for index in 0..64 {
        let left = owner.bond(move || root.get() + index).with_partial_eq();
        let right = owner.bond(move || root.get() * 3 + index).with_partial_eq();
        let sum = owner
            .bond(move || left.get() + right.get())
            .with_partial_eq();
        let calls = calls.clone();
        owner.binding(move || {
            sum.get();
            calls.set(calls.get() + 1);
        });
    }
    runtime.flush().unwrap();
    let before = runtime.stats();
    runtime
        .batch(|| {
            for _ in 0..1000 {
                source.revise(|value| value.count += 1);
            }
        })
        .unwrap();
    let after = runtime.stats();
    assert_eq!(after.source_writes - before.source_writes, 1000);
    assert_eq!(
        after.marked - before.marked,
        257,
        "each pending diamond must be forwarded only once"
    );
    assert_eq!(after.derived_runs - before.derived_runs, 1);
    assert_eq!(after.binding_runs - before.binding_runs, 0);
    assert_eq!(calls.get(), 64);
    assert_eq!(source.with(|value| value.count), 1000);
    assert_eq!(source.with(|value| value.bytes.as_ptr()), pointer);
}

#[test]
fn reads_inside_a_batch_rearm_upstream_propagation_and_dynamic_dependencies() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(0);
    let other = owner.cage(20);
    let flag = owner.cage(true);
    let parity = source.map(|value| value % 2).with_partial_eq();
    let chosen = owner
        .bond(move || {
            if flag.get() {
                parity.get() + 10
            } else {
                other.get()
            }
        })
        .with_partial_eq();
    let result = owner.bond(move || chosen.get() * 2).with_partial_eq();
    let values = Rc::new(RefCell::new(Vec::new()));
    let output = values.clone();
    owner.binding(move || output.borrow_mut().push(result.get()));
    runtime.flush().unwrap();
    runtime
        .batch(|| {
            source.revise(|value| *value = 1);
            assert_eq!(result.get(), 22);
            source.revise(|value| *value = 2);
            assert_eq!(result.get(), 20);
            source.revise(|value| *value = 3);
            flag.revise(|value| *value = false);
            assert_eq!(result.get(), 40);
            source.revise(|value| *value = 4);
            other.revise(|value| *value = 99);
            assert_eq!(result.get(), 198);
            other.revise(|value| *value = 100);
            assert_eq!(values.borrow().as_slice(), &[20]);
        })
        .unwrap();
    assert_eq!(values.borrow().as_slice(), &[20, 200]);
    flag.revise(|value| *value = true);
    runtime.flush().unwrap();
    assert_eq!(values.borrow().last(), Some(&20));
    source.revise(|value| *value = 5);
    runtime.flush().unwrap();
    assert_eq!(values.borrow().last(), Some(&22));
    let before = runtime.stats().binding_runs;
    other.revise(|value| *value = 999);
    runtime.flush().unwrap();
    assert_eq!(runtime.stats().binding_runs, before);
}

#[test]
fn subscribers_added_after_unobserved_writes_and_recycled_owners_receive_future_changes() {
    let runtime = Runtime::new();
    let root = runtime.owner();
    let source = root.cage(0usize);
    for _ in 0..1000 {
        source.revise(|value| *value += 1);
    }
    let observed = Rc::new(Cell::new(0));
    for index in 0..300 {
        let row = root.child();
        let mapped = row.enter(|| source.map(|value| value * 2).with_partial_eq());
        let output = observed.clone();
        row.binding(move || output.set(mapped.get()));
        runtime.flush().unwrap();
        assert_eq!(observed.get(), source.get() * 2);
        runtime
            .batch(|| {
                source.revise(|value| *value += 1);
                source.revise(|value| *value += 1);
            })
            .unwrap();
        assert_eq!(observed.get(), source.get() * 2);
        if index % 2 == 0 {
            source.revise(|value| *value += 1);
        }
        row.dispose();
        assert!(!mapped.is_alive());
        runtime.flush().unwrap();
        assert_eq!(runtime.stats().nodes, 1);
        assert_eq!(runtime.stats().edges, 0);
        assert!(runtime.stats().node_slots <= 3);
        assert_eq!(runtime.stats().queued, 0);
    }
}

#[test]
fn shared_source_diamonds_and_on_demand_reads_never_leave_a_clean_stale_consumer() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let a = owner.cage(1);
    let b = owner.cage(2);
    let left = owner.bond(move || a.get() + b.get()).with_partial_eq();
    let right = owner.bond(move || a.get() - b.get()).with_partial_eq();
    let sum = owner
        .bond(move || left.get() + right.get())
        .with_partial_eq();
    let seen = Rc::new(Cell::new(0));
    let output = seen.clone();
    owner.binding(move || output.set(sum.get()));
    runtime.flush().unwrap();
    for index in 0..100 {
        runtime
            .batch(|| {
                a.revise(|value| *value = index);
                b.revise(|value| *value = index + 1);
                assert_eq!(left.get(), 2 * index + 1);
                a.revise(|value| *value = index + 2);
                assert_eq!(sum.get(), 2 * (index + 2));
                b.revise(|value| *value = index + 3);
                a.revise(|value| *value = index + 4);
            })
            .unwrap();
        assert_eq!(seen.get(), 2 * (index + 4));
    }
}

#[test]
fn effects_rearm_computing_dependencies_and_panicking_evaluations_can_retry() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let source = owner.cage(0);
    let trigger = owner.cage(false);
    let calls = Rc::new(Cell::new(0));
    let output = calls.clone();
    owner.effect(move || {
        let active = trigger.get();
        let value = source.get();
        output.set(output.get() + 1);
        if active && value < 5 {
            source.revise(|value| *value += 1);
        }
    });
    runtime.flush().unwrap();
    runtime
        .batch(|| {
            trigger.revise(|value| *value = true);
            source.revise(|_| {});
        })
        .unwrap();
    assert_eq!(source.get(), 5);
    assert_eq!(calls.get(), 7);
    let failed = Rc::new(Cell::new(false));
    let failure = failed.clone();
    let mapped = owner.bond(move || {
        let value = source.get();
        if failure.replace(false) {
            panic!("retry projection");
        }
        value * 2
    });
    assert_eq!(mapped.get(), 10);
    failed.set(true);
    source.revise(|value| *value = 6);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mapped.get())).is_err());
    source.revise(|value| *value = 7);
    assert_eq!(mapped.get(), 14);
    runtime.flush().unwrap();
}

#[test]
fn deterministic_mixed_batches_match_plain_dynamic_diamond_evaluation() {
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let a = owner.cage(1i64);
    let b = owner.cage(2i64);
    let flag = owner.cage(true);
    let left = owner.bond(move || a.get() * 2).with_partial_eq();
    let chosen = owner
        .bond(move || if flag.get() { a.get() } else { b.get() })
        .with_partial_eq();
    let result = owner
        .bond(move || (left.get() + chosen.get()).rem_euclid(17))
        .with_partial_eq();
    let values = Rc::new(RefCell::new(Vec::new()));
    let output = values.clone();
    owner.binding(move || output.borrow_mut().push(result.get()));
    runtime.flush().unwrap();
    let (mut x, mut y, mut selected, mut seed) = (1i64, 2i64, true, 19u64);
    for batch in 0..1000 {
        let callbacks = values.borrow().len();
        runtime
            .batch(|| {
                for _ in 0..(batch % 13 + 1) {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    match (seed >> 32) % 5 {
                        0 => {
                            x = (seed % 1000) as i64;
                            a.revise(|value| *value = x);
                        }
                        1 => {
                            y = (seed % 1000) as i64;
                            b.revise(|value| *value = y);
                        }
                        2 => {
                            selected = !selected;
                            flag.revise(|value| *value = selected);
                        }
                        3 => {
                            assert_eq!(
                                result.get(),
                                (2 * x + if selected { x } else { y }).rem_euclid(17)
                            );
                        }
                        _ => {
                            runtime
                                .batch(|| {
                                    a.revise(|value| *value = x);
                                    b.revise(|value| *value = y);
                                })
                                .unwrap();
                        }
                    }
                }
                assert_eq!(values.borrow().len(), callbacks);
            })
            .unwrap();
        assert_eq!(
            values.borrow().last().copied(),
            Some((2 * x + if selected { x } else { y }).rem_euclid(17))
        );
        assert!(values.borrow().len() <= callbacks + 1);
        assert_eq!(runtime.stats().queued, 0);
    }
}

#[test]
fn repeated_subtree_retirement_preserves_cleanup_order_and_reused_binding_identity() {
    let runtime = Rc::new(Runtime::new());
    let parent = runtime.owner();
    let child = parent.child();
    let grandchild = child.child();
    let trace = Rc::new(RefCell::new(Vec::new()));
    for (owner, name) in [
        (&parent, "parent"),
        (&child, "child"),
        (&grandchild, "grandchild"),
    ] {
        for index in 0..2 {
            let trace = trace.clone();
            owner
                .on_cleanup(move || trace.borrow_mut().push((name, index)))
                .unwrap();
        }
    }
    struct Capture(Rc<Runtime>, Rc<Cell<usize>>);
    impl Drop for Capture {
        fn drop(&mut self) {
            // Captures must be released after the graph borrow, exactly once.
            assert_eq!(self.0.stats().nodes, 0);
            let live = self.0.owner();
            assert_eq!(live.cage(7).get(), 7);
            self.1.set(self.1.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    let capture = Capture(runtime.clone(), drops.clone());
    let old = grandchild.binding(move || {
        std::hint::black_box(&capture);
    });
    parent.dispose();
    assert_eq!(
        &*trace.borrow(),
        &[
            ("grandchild", 1),
            ("grandchild", 0),
            ("child", 1),
            ("child", 0),
            ("parent", 1),
            ("parent", 0)
        ]
    );
    assert_eq!(drops.get(), 1);
    let replacement = runtime.owner();
    let called = Rc::new(Cell::new(0));
    let out = called.clone();
    let binding = replacement.binding(move || out.set(out.get() + 1));
    for _ in 0..3 {
        parent.dispose();
        child.dispose();
        grandchild.dispose();
        old.dispose();
    }
    runtime.flush().unwrap();
    assert!(binding.is_alive());
    assert_eq!((drops.get(), called.get()), (1, 1));
}
