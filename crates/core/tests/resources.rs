use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use glory_core::{
    AccessError, Cage, Host, HostError, LocalFuture, Resource, ResourceSeed, ResourceState, Runtime,
};

struct Scheduled {
    future: RefCell<Option<LocalFuture>>,
    cancelled: Cell<bool>,
}
#[derive(Default)]
struct Executor {
    tasks: RefCell<Vec<Rc<Scheduled>>>,
    cancelled: Rc<Cell<usize>>,
    reject: Cell<bool>,
}
impl Executor {
    fn tick(&self) {
        let mut context = Context::from_waker(Waker::noop());
        let tasks = std::mem::take(&mut *self.tasks.borrow_mut());
        for task in tasks {
            let future = task.future.borrow_mut().take();
            if let Some(mut future) = future
                && !task.cancelled.get()
                && future.as_mut().poll(&mut context).is_pending()
                && !task.cancelled.get()
            {
                *task.future.borrow_mut() = Some(future);
                self.tasks.borrow_mut().push(task);
            }
        }
    }
}
impl Host for Executor {
    fn spawn(&self, future: LocalFuture) -> Result<Box<dyn FnOnce()>, HostError> {
        if self.reject.get() {
            return Err(HostError("executor closed".into()));
        }
        let task = Rc::new(Scheduled {
            future: RefCell::new(Some(future)),
            cancelled: Cell::new(false),
        });
        self.tasks.borrow_mut().push(task.clone());
        let cancelled = self.cancelled.clone();
        Ok(Box::new(move || {
            task.cancelled.set(true);
            let future = task.future.borrow_mut().take();
            drop(future);
            cancelled.set(cancelled.get() + 1);
        }))
    }
    fn wake(&self) {}
    fn now(&self) -> Duration {
        Duration::ZERO
    }
}

#[test]
fn source_changes_cancel_real_futures_and_only_latest_result_publishes() {
    struct Guard(Rc<Cell<usize>>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let executor = Rc::new(Executor::default());
    let runtime = Runtime::with_host(executor.clone());
    let owner = runtime.owner();
    let key = owner.cage(1);
    let done = Rc::new(Cell::new(false));
    let dropped = Rc::new(Cell::new(0));
    let resource = owner.resource(move || key.get(), {
        let done = done.clone();
        let dropped = dropped.clone();
        move |key| {
            let done = done.clone();
            let guard = Guard(dropped.clone());
            async move {
                let _guard = guard;
                std::future::poll_fn(|_| {
                    if done.get() {
                        Poll::Ready(())
                    } else {
                        Poll::Pending
                    }
                })
                .await;
                Ok::<_, String>(format!("value:{key}"))
            }
        }
    });
    let output = Rc::new(RefCell::new(Vec::new()));
    let log = output.clone();
    owner.binding(move || {
        if let ResourceState::Ready { key, value } = resource.get() {
            log.borrow_mut().push((*key, value.to_string()));
        }
    });
    runtime.flush().unwrap();
    assert!(resource.get().is_pending());
    executor.tick();
    runtime
        .batch(|| {
            key.revise(|key| *key = 2);
            key.revise(|key| *key = 3);
        })
        .unwrap();
    assert_eq!(dropped.get(), 1);
    assert_eq!(executor.cancelled.get(), 1);
    assert_eq!(resource.get().key(), Some(&3));
    assert_eq!(runtime.stats().tasks, 1);
    done.set(true);
    executor.tick();
    runtime.flush().unwrap();
    assert_eq!(&*output.borrow(), &[(3, "value:3".into())]);
    assert_eq!(runtime.stats().tasks, 0);
    assert_eq!(dropped.get(), 2);
    let runs = runtime.stats().effect_runs;
    key.revise(|key| *key = 3); // Equal keys do not refetch.
    runtime.flush().unwrap();
    assert_eq!(runtime.stats().effect_runs, runs + 1);
    assert_eq!(runtime.stats().tasks, 0);
}

#[test]
fn refresh_revokes_a_completion_even_when_called_inside_the_loader_poll() {
    type Handle = Resource<usize, usize, ()>;
    let executor = Rc::new(Executor::default());
    let runtime = Runtime::with_host(executor.clone());
    let owner = runtime.owner();
    let handle = Rc::new(Cell::new(None::<Handle>));
    let calls = Rc::new(Cell::new(0));
    let resource = owner.resource(|| 0, {
        let handle = handle.clone();
        let calls = calls.clone();
        move |_| {
            let handle = handle.clone();
            let calls = calls.clone();
            async move {
                let n = calls.get() + 1;
                calls.set(n);
                if n == 1 {
                    handle.get().unwrap().refresh().unwrap();
                }
                Ok(n)
            }
        }
    });
    handle.set(Some(resource));
    runtime.flush().unwrap();
    executor.tick();
    assert!(resource.get().is_pending()); // Old Ok(1) never becomes visible.
    runtime.flush().unwrap();
    executor.tick();
    assert!(matches!(resource.get(), ResourceState::Ready { value, .. } if *value == 2));
}

#[test]
fn cancellation_stays_idle_until_refresh_or_key_change_and_batches_coalesce() {
    let executor = Rc::new(Executor::default());
    let runtime = Runtime::with_host(executor.clone());
    let owner = runtime.owner();
    let key = owner.cage(0);
    let resource = owner.resource(
        move || key.get(),
        |_| std::future::pending::<Result<(), ()>>(),
    );
    runtime.flush().unwrap();
    executor.tick();
    resource.cancel().unwrap();
    assert!(matches!(resource.get(), ResourceState::Idle));
    assert_eq!(runtime.stats().tasks, 0);
    runtime.flush().unwrap();
    assert!(matches!(resource.get(), ResourceState::Idle));
    key.revise(|key| *key = 1);
    runtime.flush().unwrap();
    assert!(resource.get().is_pending());
    runtime
        .batch(|| {
            for _ in 0..200 {
                resource.refresh().unwrap();
            }
        })
        .unwrap();
    assert_eq!(runtime.stats().tasks, 1);
    assert_eq!(executor.cancelled.get(), 2);
    resource.cancel().unwrap();
    resource.refresh().unwrap();
    runtime.flush().unwrap();
    assert!(resource.get().is_pending());
    resource.refresh().unwrap();
    resource.cancel().unwrap();
    runtime.flush().unwrap();
    assert!(matches!(resource.get(), ResourceState::Idle));
}

#[test]
fn terminal_seeds_match_keys_and_do_not_require_a_host_or_cloneable_results() {
    struct Value(u64);
    struct Failure(u64);
    let runtime = Runtime::new();
    let owner = runtime.owner();
    let key = owner.cage(5);
    let ready = owner.resource_seeded(
        move || key.get(),
        |_| async { Ok::<_, Failure>(Value(0)) },
        Some(ResourceSeed {
            key: 5,
            result: Ok(Value(42)),
        }),
    );
    let failed = owner.resource_seeded(
        || 5,
        |_| async { Ok(Value(0)) },
        Some(ResourceSeed {
            key: 5,
            result: Err(Failure(7)),
        }),
    );
    runtime.flush().unwrap();
    assert!(matches!(ready.get(), ResourceState::Ready { value, .. } if value.0 == 42));
    assert!(matches!(failed.get(), ResourceState::Failed { error, .. } if error.0 == 7));
    assert_eq!(runtime.stats().tasks, 0);
    failed.refresh().unwrap();
    runtime.flush().unwrap();
    assert!(matches!(
        failed.get(),
        ResourceState::Unavailable {
            error: AccessError::NoHost,
            ..
        }
    ));
    key.revise(|key| *key = 6);
    runtime.flush().unwrap();
    assert!(
        matches!(ready.get(), ResourceState::Unavailable { key, error: AccessError::NoHost } if *key == 6)
    );
    let mismatch = owner.resource_seeded(
        || 9,
        |_| async { Ok::<_, ()>(0) },
        Some(ResourceSeed {
            key: 8,
            result: Ok(88),
        }),
    );
    runtime.flush().unwrap();
    assert!(matches!(mismatch.get(), ResourceState::Unavailable { key, .. } if *key == 9));
    let refreshed = owner.resource_seeded(
        || 5,
        |_| async { Ok::<_, ()>(0) },
        Some(ResourceSeed {
            key: 5,
            result: Ok(88),
        }),
    );
    refreshed.refresh().unwrap();
    runtime.flush().unwrap();
    assert!(matches!(
        refreshed.get(),
        ResourceState::Unavailable {
            error: AccessError::NoHost,
            ..
        }
    ));
}

#[test]
fn business_errors_host_rejection_and_retries_are_distinct() {
    let executor = Rc::new(Executor::default());
    executor.reject.set(true);
    let runtime = Runtime::with_host(executor.clone());
    let owner = runtime.owner();
    let resource = owner.resource(|| "key", |_| async { Err::<(), _>("not found") });
    runtime.flush().unwrap();
    assert!(matches!(
        resource.get(),
        ResourceState::Unavailable {
            error: AccessError::Host(_),
            ..
        }
    ));
    assert_eq!(runtime.stats().tasks, 0);
    executor.reject.set(false);
    resource.refresh().unwrap();
    runtime.flush().unwrap();
    executor.tick();
    assert!(matches!(resource.get(), ResourceState::Failed { error, .. } if *error == "not found"));
}

#[test]
fn dynamic_source_dependencies_rebind_and_loader_reads_do_not_subscribe_driver() {
    let executor = Rc::new(Executor::default());
    let runtime = Runtime::with_host(executor.clone());
    let owner = runtime.owner();
    let choose = owner.cage(true);
    let a = owner.cage(1);
    let b = owner.cage(2);
    let incidental = owner.cage(3);
    let calls = Rc::new(Cell::new(0));
    let resource = owner.resource(move || if choose.get() { a.get() } else { b.get() }, {
        let calls = calls.clone();
        move |key| {
            incidental.get();
            calls.set(calls.get() + 1);
            async move { Ok::<_, ()>(key) }
        }
    });
    runtime.flush().unwrap();
    executor.tick();
    incidental.revise(|v| *v += 1);
    runtime.flush().unwrap();
    assert_eq!(calls.get(), 1);
    choose.revise(|v| *v = false);
    runtime.flush().unwrap();
    executor.tick();
    assert_eq!(calls.get(), 2);
    a.revise(|v| *v += 1);
    runtime.flush().unwrap();
    assert_eq!(runtime.stats().tasks, 0);
    assert_eq!(resource.get().key(), Some(&2));
}

#[test]
fn result_destructors_and_cancellation_may_read_graph_without_borrow_conflicts() {
    struct Guard(Cage<u32>, Rc<Cell<usize>>);
    impl Drop for Guard {
        fn drop(&mut self) {
            assert_eq!(self.0.get(), 4);
            self.1.set(self.1.get() + 1);
        }
    }
    let executor = Rc::new(Executor::default());
    let runtime = Runtime::with_host(executor.clone());
    let root = runtime.owner();
    let value = root.cage(4);
    let child = root.child();
    let drops = Rc::new(Cell::new(0));
    let resource = child.resource(|| 1, {
        let drops = drops.clone();
        move |_| {
            let guard = Guard(value, drops.clone());
            async move { Ok::<_, ()>(guard) }
        }
    });
    runtime.flush().unwrap();
    executor.tick();
    resource.refresh().unwrap();
    runtime.flush().unwrap();
    assert_eq!(drops.get(), 1);
    executor.tick();
    child.dispose();
    assert_eq!(drops.get(), 2);
    assert!(matches!(resource.try_get(), Err(AccessError::Disposed)));
}

#[test]
fn owner_churn_bounds_tasks_graph_slots_and_drops_unpolled_and_pending_work() {
    let executor = Rc::new(Executor::default());
    let runtime = Runtime::with_host(executor.clone());
    let root = runtime.owner();
    for n in 0..1000 {
        let row = root.child();
        let resource = row.resource(move || n, |_| std::future::pending::<Result<(), ()>>());
        runtime.flush().unwrap();
        if n % 2 == 0 {
            executor.tick();
        }
        assert_eq!(runtime.stats().tasks, 1);
        row.dispose();
        assert!(!resource.is_alive());
        assert!(matches!(resource.refresh(), Err(AccessError::Disposed)));
        executor.tick();
        assert_eq!(runtime.stats().nodes, 0);
        assert_eq!(runtime.stats().tasks, 0);
        assert!(runtime.stats().node_slots <= 3);
        assert!(runtime.stats().task_slots <= 1);
    }
    assert_eq!(executor.cancelled.get(), 1000);
}
