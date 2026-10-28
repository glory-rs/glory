use super::*;
use std::{cell::RefCell, collections::BTreeMap};

#[derive(Default)]
struct State {
    records: BTreeMap<u64, Option<PermissionState>>,
    native: Option<u64>,
    begins: usize,
    cancels: usize,
    closes: usize,
}
#[derive(Clone, Default)]
struct Fake(Rc<RefCell<State>>);
impl Fake {
    fn complete(&self) {
        let mut state = self.0.borrow_mut();
        if let Some(id) = state.native.take()
            && let Some(reply) = state.records.get_mut(&id)
        {
            *reply = Some(PermissionState {
                status: Status::Granted,
                rationale: false,
            });
        }
    }
}
impl PermissionDriver for Fake {
    fn begin(&self, id: u64, _: Permission, action: Action) -> Result<(), PermissionError> {
        let mut state = self.0.borrow_mut();
        if action == Action::Request && state.native.is_some() {
            return Err(PermissionError::Busy);
        }
        state.begins += 1;
        if action == Action::Request {
            state.native = Some(id);
        }
        state.records.insert(id, None);
        Ok(())
    }
    fn poll(&self, id: u64) -> Result<Option<PermissionState>, PermissionError> {
        let mut state = self.0.borrow_mut();
        let value = *state.records.get(&id).ok_or(PermissionError::Closed)?;
        if value.is_some() {
            state.records.remove(&id);
        }
        Ok(value)
    }
    fn cancel(&self, id: u64) {
        let mut state = self.0.borrow_mut();
        state.records.remove(&id);
        state.cancels += 1;
    }
    fn close(&self) {
        let mut state = self.0.borrow_mut();
        state.records.clear();
        state.closes += 1;
    }
    fn stats(&self) -> Result<NativeStats, PermissionError> {
        let state = self.0.borrow();
        Ok(NativeStats {
            records: state.records.len(),
            active_prompts: usize::from(state.native.is_some()),
            active_queries: 0,
        })
    }
}
fn poll(future: &mut PermissionFuture) -> Poll<Result<PermissionState, PermissionError>> {
    Pin::new(future).poll(&mut Context::from_waker(std::task::Waker::noop()))
}
fn limits() -> PermissionLimits {
    PermissionLimits {
        poll_interval: Duration::from_millis(5),
        ..Default::default()
    }
}

#[test]
fn requests_are_lazy_cancel_releases_delivery_but_not_the_os_prompt_and_old_ids_cannot_publish() {
    let fake = Fake::default();
    let service = Permissions::new(fake.clone(), limits()).unwrap();
    drop(service.request(Permission::Camera));
    assert_eq!(fake.0.borrow().begins, 0);
    let mut first = service.request(Permission::Camera);
    assert!(poll(&mut first).is_pending());
    drop(first);
    assert_eq!(service.stats().waiting, 0);
    assert_eq!(
        service.native_stats().unwrap(),
        NativeStats {
            records: 0,
            active_prompts: 1,
            active_queries: 0
        }
    );
    assert_eq!(
        poll(&mut service.request(Permission::Microphone)),
        Poll::Ready(Err(PermissionError::Busy))
    );
    fake.complete();
    let mut next = service.request(Permission::Camera);
    assert!(poll(&mut next).is_pending());
    assert_eq!(fake.0.borrow().native, Some(3));
    fake.complete();
    // The OS timer thread can wake later than 8 ms under a parallel build.
    // Wait for actual settlement, bounded independently of the service timeout.
    let deadline = Instant::now() + Duration::from_secs(2);
    let ready = loop {
        let ready = poll(&mut next);
        if ready.is_ready() {
            break ready;
        }
        assert!(
            Instant::now() < deadline,
            "completed permission did not settle"
        );
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(
        ready,
        Poll::Ready(Ok(PermissionState {
            status: Status::Granted,
            rationale: false
        }))
    );
    assert_eq!(service.stats().waiting, 0);
}

#[test]
fn close_and_deadline_revoke_pending_delivery_without_reopening_the_driver() {
    let fake = Fake::default();
    let service = Permissions::new(
        fake.clone(),
        PermissionLimits {
            timeout: Duration::from_millis(20),
            ..limits()
        },
    )
    .unwrap();
    let mut pending = service.request(Permission::Camera);
    assert!(poll(&mut pending).is_pending());
    std::thread::sleep(Duration::from_millis(25));
    assert_eq!(
        poll(&mut pending),
        Poll::Ready(Err(PermissionError::Deadline))
    );
    assert_eq!(service.native_stats().unwrap().active_prompts, 1);
    fake.complete();
    let mut second = service.request(Permission::Camera);
    assert!(poll(&mut second).is_pending());
    service.close();
    service.close();
    assert_eq!(poll(&mut second), Poll::Ready(Err(PermissionError::Closed)));
    fake.complete();
    assert_eq!(
        poll(&mut service.query(Permission::Camera)),
        Poll::Ready(Err(PermissionError::Closed))
    );
    drop(second);
    drop(pending);
    drop(service);
    assert_eq!(fake.0.borrow().closes, 1);
}

#[test]
fn bridge_refuses_malformed_status_flags_stats_and_errors() {
    for code in [1, 0x100, 0x108, 0x140, 0x1000, i64::MAX, -99] {
        assert_eq!(bridge::reply(code), Err(PermissionError::InvalidReply));
    }
    for status in 1..=7 {
        assert!(bridge::reply(0x100 | status).unwrap().is_some());
        assert!(bridge::reply(0x120 | status).unwrap().unwrap().rationale);
    }
    for code in [33, 33 << 17, 0x800000, i64::MAX] {
        assert_eq!(bridge::stats(code), Err(PermissionError::InvalidReply));
    }
    assert_eq!(
        bridge::stats(0x10000).unwrap(),
        NativeStats {
            records: 0,
            active_prompts: 1,
            active_queries: 0
        }
    );
    assert_eq!(bridge::accepted(0x102), Err(PermissionError::InvalidReply));
    assert_eq!(bridge::reply(-4), Err(PermissionError::MissingDeclaration));
}

#[test]
fn admission_limits_are_validated_and_identity_exhaustion_does_not_wrap() {
    let fake = Fake::default();
    assert!(
        Permissions::new(
            fake.clone(),
            PermissionLimits {
                pending: 0,
                ..limits()
            }
        )
        .is_err()
    );
    assert_eq!(fake.0.borrow().closes, 1);
    let service = Permissions::new(fake.clone(), limits()).unwrap();
    service.0.next.set(i64::MAX as u64 + 1);
    assert_eq!(
        poll(&mut service.request(Permission::Camera)),
        Poll::Ready(Err(PermissionError::Exhausted))
    );
    assert_eq!(fake.0.borrow().begins, 0);
}

#[test]
fn pending_budget_refuses_before_native_work_and_cancellation_recovers_capacity() {
    let fake = Fake::default();
    let service = Permissions::new(
        fake.clone(),
        PermissionLimits {
            pending: 1,
            ..limits()
        },
    )
    .unwrap();
    let mut first = service.query(Permission::Notifications);
    assert!(poll(&mut first).is_pending());
    for _ in 0..200 {
        assert_eq!(
            poll(&mut service.query(Permission::Camera)),
            Poll::Ready(Err(PermissionError::Busy))
        );
    }
    assert_eq!(fake.0.borrow().begins, 1);
    assert_eq!(service.stats().peak_waiting, 1);
    drop(first);
    let mut next = service.query(Permission::Camera);
    assert!(poll(&mut next).is_pending());
    assert_eq!(fake.0.borrow().begins, 2);
    assert!(fake.0.borrow().records.contains_key(&2));
    drop(next);
    assert_eq!(service.stats().waiting, 0);
}

#[test]
fn native_begin_reentry_cannot_exceed_the_local_pending_budget() {
    struct Reentrant {
        service: Rc<RefCell<Option<Permissions>>>,
        fake: Fake,
    }
    impl PermissionDriver for Reentrant {
        fn begin(&self, id: u64, kind: Permission, action: Action) -> Result<(), PermissionError> {
            let service = self.service.borrow().as_ref().unwrap().clone();
            assert_eq!(
                poll(&mut service.query(Permission::Microphone)),
                Poll::Ready(Err(PermissionError::Busy))
            );
            self.fake.begin(id, kind, action)
        }
        fn poll(&self, id: u64) -> Result<Option<PermissionState>, PermissionError> {
            self.fake.poll(id)
        }
        fn cancel(&self, id: u64) {
            self.fake.cancel(id);
        }
        fn close(&self) {
            self.fake.close();
        }
    }
    let holder = Rc::new(RefCell::new(None));
    let fake = Fake::default();
    let service = Permissions::new(
        Reentrant {
            service: holder.clone(),
            fake: fake.clone(),
        },
        PermissionLimits {
            pending: 1,
            ..limits()
        },
    )
    .unwrap();
    *holder.borrow_mut() = Some(service.clone());
    let mut outer = service.query(Permission::Camera);
    assert!(poll(&mut outer).is_pending());
    assert_eq!(service.stats().peak_waiting, 1);
    assert_eq!(fake.0.borrow().begins, 1);
    drop(outer);
    holder.borrow_mut().take();
}

#[test]
fn close_from_a_native_poll_revokes_a_result_that_was_not_yet_delivered() {
    struct Closing(Rc<RefCell<Option<Permissions>>>);
    impl PermissionDriver for Closing {
        fn begin(&self, _: u64, _: Permission, _: Action) -> Result<(), PermissionError> {
            Ok(())
        }
        fn poll(&self, _: u64) -> Result<Option<PermissionState>, PermissionError> {
            let service = self.0.borrow().as_ref().unwrap().clone();
            service.close();
            Ok(Some(PermissionState {
                status: Status::Granted,
                rationale: false,
            }))
        }
        fn cancel(&self, _: u64) {}
        fn close(&self) {}
    }
    let holder = Rc::new(RefCell::new(None));
    let service = Permissions::new(Closing(holder.clone()), limits()).unwrap();
    *holder.borrow_mut() = Some(service.clone());
    assert_eq!(
        poll(&mut service.request(Permission::Camera)),
        Poll::Ready(Err(PermissionError::Closed))
    );
    assert_eq!(service.stats().waiting, 0);
    holder.borrow_mut().take();
}

#[cfg(feature = "shell")]
#[test]
fn owner_retirement_drops_the_permission_future_and_never_runs_a_late_application_callback() {
    use glory_core::{Runtime, executor::Executor};
    let executor = Rc::new(Executor::new(2));
    let runtime = Runtime::with_host(executor.clone());
    let owner = runtime.owner();
    let fake = Fake::default();
    let service = Permissions::new(fake.clone(), limits()).unwrap();
    let called = Rc::new(Cell::new(false));
    let callback = called.clone();
    let request = service.request(Permission::Camera);
    owner
        .spawn(async move {
            let _ = request.await;
            callback.set(true);
        })
        .unwrap();
    assert_eq!(fake.0.borrow().begins, 0);
    assert!(executor.poll_one());
    drop(owner);
    assert_eq!(runtime.stats().tasks, 0);
    assert_eq!(service.stats().waiting, 0);
    assert_eq!(service.native_stats().unwrap().active_prompts, 1);
    fake.complete();
    std::thread::sleep(Duration::from_millis(8));
    assert!(!executor.poll_one());
    assert!(!called.get());
}
