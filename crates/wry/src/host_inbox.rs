//! The native callback boundary: raw IPC is admitted before notifying Tao.
//! Only a coalesced, body-free wake enters the platform event queue.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    task::Wake,
};

use crate::protocol::{MAX_INPUTS, MAX_IPC_BYTES, MAX_PENDING_BYTES};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InboxError {
    MessageBytes,
    Capacity,
    Closed,
}
impl std::fmt::Display for InboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::MessageBytes => "Wry host IPC message budget exceeded",
            Self::Capacity => "Wry host IPC inbox budget exceeded",
            Self::Closed => "Wry host IPC inbox closed",
        })
    }
}

#[derive(Default)]
struct State {
    messages: VecDeque<String>,
    bytes: usize,
    fault: Option<InboxError>,
    notified: bool,
    closed: bool,
}
struct Shared {
    state: Mutex<State>,
    notify: Box<dyn Fn() + Send + Sync>,
}
impl Shared {
    fn notify(&self, signal: bool) {
        if signal {
            (self.notify)();
        }
    }
}
impl State {
    fn signal(&mut self) -> bool {
        if self.closed || self.notified {
            false
        } else {
            self.notified = true;
            true
        }
    }
}

#[derive(Clone)]
pub(crate) struct Sender(Arc<Shared>);
pub(crate) struct Receiver(Arc<Shared>);

pub(crate) fn channel(notify: impl Fn() + Send + Sync + 'static) -> (Sender, Receiver) {
    let shared = Arc::new(Shared {
        state: Mutex::default(),
        notify: Box::new(notify),
    });
    (Sender(shared.clone()), Receiver(shared))
}

impl Sender {
    /// Overflow is terminal: retain the accepted FIFO prefix, reject further
    /// messages, then surface the fault. Silently dropping an acknowledgement
    /// or terminal edit and continuing would desynchronize the interpreter.
    pub(crate) fn post(&self, message: String) -> Result<(), InboxError> {
        let (result, signal) = {
            let mut state = self.0.state.lock().expect("Wry host inbox");
            if state.closed {
                return Err(InboxError::Closed);
            }
            if let Some(error) = state.fault {
                return Err(error);
            }
            let error = if message.len() > MAX_IPC_BYTES {
                Some(InboxError::MessageBytes)
            } else if state.messages.len() >= MAX_INPUTS
                || message.len() > MAX_PENDING_BYTES - state.bytes
            {
                Some(InboxError::Capacity)
            } else {
                None
            };
            let result = if let Some(error) = error {
                state.fault = Some(error);
                Err(error)
            } else {
                state.bytes += message.len();
                state.messages.push_back(message);
                Ok(())
            };
            (result, state.signal())
        };
        self.0.notify(signal);
        result
    }
    fn signal(&self) {
        let signal = self.0.state.lock().expect("Wry host inbox").signal();
        self.0.notify(signal);
    }
}
impl Wake for Sender {
    fn wake(self: Arc<Self>) {
        self.signal();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.signal();
    }
}

impl Receiver {
    /// Acknowledge only an actual platform wake, not unrelated window events:
    /// this keeps at most one body-free notification in the platform queue.
    pub(crate) fn acknowledge(&self) {
        self.0.state.lock().expect("Wry host inbox").notified = false;
    }
    pub(crate) fn pop(&self) -> Result<Option<String>, InboxError> {
        let mut state = self.0.state.lock().expect("Wry host inbox");
        if let Some(message) = state.messages.pop_front() {
            state.bytes -= message.len();
            Ok(Some(message))
        } else if let Some(error) = state.fault {
            Err(error)
        } else {
            Ok(None)
        }
    }
    /// A bounded host turn consumes one message. Arrange another turn for the
    /// remaining prefix/fault; posts racing this check use the same lock/bit.
    pub(crate) fn resignal(&self) {
        let signal = {
            let mut state = self.0.state.lock().expect("Wry host inbox");
            (!state.messages.is_empty() || state.fault.is_some()) && state.signal()
        };
        self.0.notify(signal);
    }
    pub(crate) fn close(&self) {
        let messages = {
            let mut state = self.0.state.lock().expect("Wry host inbox");
            state.closed = true;
            state.notified = false;
            state.bytes = 0;
            state.fault = None;
            std::mem::take(&mut state.messages)
        };
        drop(messages);
    }
}
impl Drop for Receiver {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{
            Barrier,
            atomic::{AtomicUsize, Ordering},
        },
        task::Waker,
    };

    fn inbox() -> (Sender, Receiver, Arc<AtomicUsize>) {
        let wakes = Arc::new(AtomicUsize::new(0));
        let count = wakes.clone();
        let (sender, receiver) = channel(move || {
            count.fetch_add(1, Ordering::SeqCst);
        });
        (sender, receiver, wakes)
    }

    #[test]
    fn paused_consumer_bounds_count_and_preserves_prefix_before_terminal_failure() {
        let (sender, receiver, wakes) = inbox();
        for index in 0..MAX_INPUTS {
            sender
                .post(format!("{index}:ack-or-terminal-input"))
                .unwrap();
        }
        for _ in 0..10_000 {
            assert_eq!(sender.post("overflow".into()), Err(InboxError::Capacity));
        }
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        for index in 0..MAX_INPUTS {
            receiver.acknowledge();
            assert_eq!(
                receiver.pop().unwrap(),
                Some(format!("{index}:ack-or-terminal-input"))
            );
            receiver.resignal();
        }
        assert_eq!(receiver.pop(), Err(InboxError::Capacity));
        let state = sender.0.state.lock().unwrap();
        assert_eq!(state.bytes, 0);
        assert!(state.messages.is_empty());
    }

    #[test]
    fn renderer_receives_ready_ack_and_terminal_fault_in_callback_order() {
        let (sender, receiver, _) = inbox();
        let renderer = crate::Renderer::new(glory_render::RuntimeId::new());
        let message = |payload: serde_json::Value| {
            let mut message = serde_json::json!({
                "v": crate::protocol::VERSION, "session": renderer.session()
            });
            message
                .as_object_mut()
                .unwrap()
                .extend(payload.as_object().unwrap().clone());
            message.to_string()
        };
        sender
            .post(message(serde_json::json!({"type":"ready"})))
            .unwrap();
        renderer.receive(&receiver.pop().unwrap().unwrap()).unwrap();
        renderer.set_author_css(".example { color: red; }").unwrap();
        let script = renderer.take_script().unwrap().unwrap();
        let batch: serde_json::Value = serde_json::from_str(
            script
                .strip_prefix("window.__gloryWryReceive(")
                .unwrap()
                .strip_suffix(");")
                .unwrap(),
        )
        .unwrap();
        assert!(renderer.in_flight());
        sender
            .post(message(
                serde_json::json!({"type":"ack","sequence":batch["sequence"]}),
            ))
            .unwrap();
        sender
            .post(message(
                serde_json::json!({"type":"fault","message":"terminal"}),
            ))
            .unwrap();
        renderer.receive(&receiver.pop().unwrap().unwrap()).unwrap();
        assert!(!renderer.in_flight());
        assert!(
            renderer
                .receive(&receiver.pop().unwrap().unwrap())
                .unwrap_err()
                .to_string()
                .contains("terminal")
        );
        assert_eq!(receiver.pop().unwrap(), None);
    }

    #[test]
    fn paused_consumer_bounds_total_bytes_and_releases_admission_on_pop() {
        let (sender, receiver, wakes) = inbox();
        let part = "x".repeat(MAX_PENDING_BYTES / 2);
        sender.post(part.clone()).unwrap();
        sender.post(part.clone()).unwrap();
        assert_eq!(sender.0.state.lock().unwrap().bytes, MAX_PENDING_BYTES);
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        assert_eq!(receiver.pop().unwrap(), Some(part.clone()));
        sender.post(part).unwrap();
        assert_eq!(sender.post("x".into()), Err(InboxError::Capacity));
        receiver.close();
        let state = sender.0.state.lock().unwrap();
        assert_eq!(state.bytes, 0);
        assert_eq!(state.messages.capacity(), 0);
    }

    #[test]
    fn oversized_message_faults_without_retaining_body_and_late_callbacks_are_closed() {
        let (sender, receiver, wakes) = inbox();
        assert_eq!(
            sender.post("x".repeat(MAX_IPC_BYTES + 1)),
            Err(InboxError::MessageBytes)
        );
        assert_eq!(receiver.pop(), Err(InboxError::MessageBytes));
        assert_eq!(sender.0.state.lock().unwrap().bytes, 0);
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        drop(receiver);
        for _ in 0..1000 {
            assert_eq!(sender.post("late".into()), Err(InboxError::Closed));
            sender.signal();
        }
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn external_wakes_share_notification_and_resume_never_loses_remaining_work() {
        let (sender, receiver, wakes) = inbox();
        let waker = Waker::from(Arc::new(sender.clone()));
        for index in 0..100 {
            sender.post(index.to_string()).unwrap();
            waker.wake_by_ref();
        }
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        for index in 0..100 {
            receiver.acknowledge();
            assert_eq!(receiver.pop().unwrap(), Some(index.to_string()));
            receiver.resignal();
            assert_eq!(wakes.load(Ordering::SeqCst), (index + 1).min(99) + 1);
        }
        assert_eq!(receiver.pop().unwrap(), None);
        waker.wake_by_ref();
        assert_eq!(wakes.load(Ordering::SeqCst), 101);
    }

    #[test]
    fn concurrent_post_wake_and_close_are_bounded_and_do_not_deadlock() {
        let (sender, receiver, wakes) = inbox();
        let start = Arc::new(Barrier::new(9));
        let threads = (0..8)
            .map(|_| {
                let sender = sender.clone();
                let start = start.clone();
                std::thread::spawn(move || {
                    start.wait();
                    for _ in 0..2000 {
                        let _ = sender.post("message".into());
                        sender.signal();
                    }
                })
            })
            .collect::<Vec<_>>();
        start.wait();
        receiver.close();
        for thread in threads {
            thread.join().unwrap();
        }
        let state = sender.0.state.lock().unwrap();
        assert!(state.closed);
        assert_eq!(state.bytes, 0);
        assert!(state.messages.is_empty());
        assert!(wakes.load(Ordering::SeqCst) <= 1);
    }

    #[test]
    fn posts_racing_consumption_always_leave_a_notification_for_pending_work() {
        let (sender, receiver, wakes) = inbox();
        let barrier = Arc::new(Barrier::new(2));
        let producer = std::thread::spawn({
            let sender = sender.clone();
            let barrier = barrier.clone();
            move || {
                for index in 0..1000 {
                    barrier.wait();
                    sender.post(index.to_string()).unwrap();
                    barrier.wait();
                }
            }
        });
        for _ in 0..1000 {
            receiver.acknowledge();
            barrier.wait();
            let _ = receiver.pop().unwrap();
            receiver.resignal();
            barrier.wait();
            let state = sender.0.state.lock().unwrap();
            assert!(state.messages.is_empty() || state.notified);
        }
        producer.join().unwrap();
        assert!(wakes.load(Ordering::SeqCst) > 0);
    }
}
