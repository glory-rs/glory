use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use crate::{Host, HostError, LocalFuture};

#[derive(Default)]
struct Queue {
    closed: bool,
    live: BTreeSet<usize>,
    ready: VecDeque<usize>,
    queued: BTreeSet<usize>,
    dirty: bool,
    waker: Option<Waker>,
    notified: bool,
}
impl Queue {
    fn signal(&mut self) -> Option<Waker> {
        if self.notified {
            return None;
        }
        let waker = self.waker.clone();
        self.notified = waker.is_some();
        waker
    }
}
#[derive(Default)]
struct Notify(Mutex<Queue>);
impl Notify {
    fn task(&self, id: usize) {
        let waker = {
            let mut queue = self.0.lock().expect("local task queue");
            if !queue.live.contains(&id) || !queue.queued.insert(id) {
                return;
            }
            queue.ready.push_back(id);
            queue.signal()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
    fn remove(&self, id: usize) {
        let mut queue = self.0.lock().expect("local task queue");
        queue.live.remove(&id);
        queue.queued.remove(&id);
        queue.ready.retain(|entry| *entry != id);
    }
}
struct TaskWake {
    notify: Arc<Notify>,
    id: usize,
}
impl Wake for TaskWake {
    fn wake(self: Arc<Self>) {
        self.notify.task(self.id);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.notify.task(self.id);
    }
}
type Tasks = Rc<RefCell<BTreeMap<usize, Rc<RefCell<Option<LocalFuture>>>>>>;

/// Local futures stay local. Only ids/ready bits/wakers cross threads, allowing
/// network/timer wakers without unsafe Send wrappers around Rust UI state.
pub struct Executor {
    tasks: Tasks,
    notify: Arc<Notify>,
    next: std::cell::Cell<usize>,
    started: Instant,
    limit: usize,
}
impl Executor {
    pub fn new(limit: usize) -> Self {
        Self {
            tasks: Rc::default(),
            notify: Arc::default(),
            next: std::cell::Cell::new(0),
            started: Instant::now(),
            limit,
        }
    }
    /// Begin a host poll turn and acknowledge its previous notification. All
    /// subsequent task/dirty wakes coalesce until the next host turn.
    pub fn observe(&self, waker: &Waker) {
        let previous = {
            let mut queue = self.notify.0.lock().expect("local task queue");
            if queue.closed {
                return;
            }
            queue.notified = false;
            queue.waker.replace(waker.clone())
        };
        drop(previous);
    }
    pub fn dirty(&self) -> bool {
        std::mem::take(&mut self.notify.0.lock().expect("local task queue").dirty)
    }
    pub fn work(&self) -> bool {
        let queue = self.notify.0.lock().expect("local task queue");
        !queue.ready.is_empty() || queue.dirty
    }
    pub fn len(&self) -> usize {
        self.tasks.borrow().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn poll_one(&self) -> bool {
        let id = {
            let mut queue = self.notify.0.lock().expect("local task queue");
            let id = queue.ready.pop_front();
            if let Some(id) = id {
                queue.queued.remove(&id);
            }
            id
        };
        let Some(id) = id else { return false };
        let entry = self.tasks.borrow().get(&id).cloned();
        let Some(entry) = entry else { return true };
        let future = entry.borrow_mut().take();
        let Some(mut future) = future else {
            return true;
        };
        let waker = Waker::from(Arc::new(TaskWake {
            notify: self.notify.clone(),
            id,
        }));
        let mut context = Context::from_waker(&waker);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(()) => {
                let removed = self.tasks.borrow_mut().remove(&id);
                self.notify.remove(id);
                drop(removed);
            }
            Poll::Pending => {
                if self.tasks.borrow().contains_key(&id) {
                    *entry.borrow_mut() = Some(future);
                }
            }
        }
        true
    }
    pub fn close(&self) {
        let waker = {
            let mut queue = self.notify.0.lock().expect("local task queue");
            queue.closed = true;
            queue.dirty = false;
            queue.notified = false;
            queue.live.clear();
            queue.ready.clear();
            queue.queued.clear();
            queue.waker.take()
        };
        let tasks = std::mem::take(&mut *self.tasks.borrow_mut());
        drop(waker);
        drop(tasks);
    }
}
impl Host for Executor {
    fn spawn(&self, future: LocalFuture) -> Result<Box<dyn FnOnce()>, HostError> {
        if self.notify.0.lock().expect("local task queue").closed {
            return Err(HostError("local executor closed".into()));
        }
        if self.tasks.borrow().len() >= self.limit {
            return Err(HostError("local task limit".into()));
        }
        let id = self.next.get();
        self.next.set(
            id.checked_add(1)
                .ok_or_else(|| HostError("local task ids exhausted".into()))?,
        );
        self.tasks
            .borrow_mut()
            .insert(id, Rc::new(RefCell::new(Some(future))));
        self.notify
            .0
            .lock()
            .expect("local task queue")
            .live
            .insert(id);
        self.notify.task(id);
        let tasks = Rc::downgrade(&self.tasks);
        let notify = self.notify.clone();
        Ok(Box::new(move || {
            notify.remove(id);
            let removed = tasks
                .upgrade()
                .and_then(|tasks| tasks.borrow_mut().remove(&id));
            drop(removed); // Drop arbitrary user futures outside every borrow.
        }))
    }
    fn wake(&self) {
        let waker = {
            let mut queue = self.notify.0.lock().expect("local task queue");
            if queue.closed || queue.dirty {
                return;
            }
            queue.dirty = true;
            queue.signal()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
    fn now(&self) -> Duration {
        self.started.elapsed()
    }
}
impl Drop for Executor {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct CountWake(AtomicUsize);
    impl Wake for CountWake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn ready_only_external_wakes_are_coalesced_and_retired_wakers_do_nothing() {
        let executor = Executor::new(1);
        let notified = Arc::new(CountWake::default());
        executor.observe(&Waker::from(notified.clone()));
        let polls = Rc::new(Cell::new(0));
        let saved = Arc::new(Mutex::new(None));
        let cancel = executor
            .spawn(Box::pin(std::future::poll_fn({
                let polls = polls.clone();
                let saved = saved.clone();
                move |context| {
                    polls.set(polls.get() + 1);
                    *saved.lock().unwrap() = Some(context.waker().clone());
                    Poll::Pending
                }
            })))
            .unwrap();
        assert_eq!(polls.get(), 0, "spawn must not synchronously poll");
        assert!(executor.poll_one());
        assert!(
            !executor.poll_one(),
            "pending task is not ready until woken"
        );
        executor.observe(&Waker::from(notified.clone()));
        let task_waker = saved.lock().unwrap().clone().unwrap();
        let before = notified.0.load(Ordering::SeqCst);
        std::thread::spawn({
            let waker = task_waker.clone();
            move || {
                for _ in 0..10_000 {
                    waker.wake_by_ref();
                }
            }
        })
        .join()
        .unwrap();
        assert_eq!(notified.0.load(Ordering::SeqCst), before + 1);
        assert!(executor.poll_one());
        assert_eq!(polls.get(), 2);
        cancel();
        let before = notified.0.load(Ordering::SeqCst);
        task_waker.wake_by_ref();
        assert_eq!(notified.0.load(Ordering::SeqCst), before);
        assert!(executor.is_empty());
        assert!(!executor.work());
    }
    #[test]
    fn many_self_wakes_send_one_host_notification_per_turn() {
        let executor = Executor::new(1);
        let notified = Arc::new(CountWake::default());
        let waker = Waker::from(notified.clone());
        executor.observe(&waker);
        let cancel = executor
            .spawn(Box::pin(std::future::poll_fn(|cx| {
                cx.waker().wake_by_ref();
                Poll::Pending
            })))
            .unwrap();
        for turn in 0..3 {
            executor.observe(&waker);
            let before = notified.0.load(Ordering::SeqCst);
            for _ in 0..64 {
                assert!(executor.poll_one());
                executor.wake();
            }
            assert_eq!(notified.0.load(Ordering::SeqCst), before + 1, "turn {turn}");
            executor.dirty();
        }
        cancel();
    }
    #[test]
    fn cancellation_and_close_drop_futures_outside_borrows_and_reject_reopening() {
        struct Cleanup {
            executor: std::rc::Weak<Executor>,
            observed: Rc<Cell<usize>>,
        }
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let executor = self.executor.upgrade().unwrap();
                self.observed.set(self.observed.get() + 1);
                assert!(executor.is_empty());
                executor.wake(); // Must not hold notify/task borrows during Drop.
            }
        }
        let executor = Rc::new(Executor::new(1));
        let observed = Rc::new(Cell::new(0));
        let future = || {
            let cleanup = Cleanup {
                executor: Rc::downgrade(&executor),
                observed: observed.clone(),
            };
            Box::pin(async move {
                let _cleanup = cleanup;
                std::future::pending::<()>().await;
            }) as LocalFuture
        };
        let cancel = executor.spawn(future()).unwrap();
        cancel();
        assert_eq!(observed.get(), 1);
        let _cancel = executor.spawn(future()).unwrap();
        executor.close();
        assert_eq!(observed.get(), 2);
        assert!(!executor.work());
        assert!(executor.spawn(future()).is_err());
        assert_eq!(observed.get(), 3);
        assert!(!executor.work());
    }
}
