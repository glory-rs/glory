//! Document-local clipboard admission. Driver futures are polled without an
//! engine borrow; completions only enqueue validated current semantic events.
use super::*;
use crate::clipboard::{Clipboard, ClipboardError};
use futures_timer::Delay;
use glory_render::ClipboardOutcome;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
    time::Instant,
};
#[cfg(feature = "shell")]
#[path = "clipboard_editor.rs"]
pub(super) mod editor;

enum Payload {
    Written,
    #[cfg(feature = "shell")]
    Text(String),
}
type Completion = Result<Payload, ClipboardError>;

#[derive(Clone, Copy)]
enum Consumer {
    Semantic {
        node: NodeId,
        request: u64,
        revision: u64,
    },
    #[cfg(feature = "shell")]
    Editor(editor::Lease),
}
struct Work {
    ticket: u64,
    consumer: Consumer,
    future: Pin<Box<dyn Future<Output = Completion> + Send>>,
    started: bool,
    reported: bool,
    deadline: Instant,
    timer: Option<Delay>,
}
pub(super) struct Io {
    config: Option<Clipboard>,
    next: u64,
    active: Option<u64>,
    work: Option<Work>,
    #[cfg(feature = "shell")]
    editor_cancelled: bool,
}
impl Io {
    pub fn new(config: Option<Clipboard>) -> Self {
        Self {
            config,
            next: 0,
            active: None,
            work: None,
            #[cfg(feature = "shell")]
            editor_cancelled: false,
        }
    }
    pub fn configured(&self) -> bool {
        self.config.is_some()
    }
    #[cfg(all(test, feature = "shell"))]
    pub(in crate::document) fn expire(&mut self) {
        self.work.as_mut().expect("pending clipboard").deadline = Instant::now();
    }
}
fn outcome(error: ClipboardError) -> ClipboardOutcome {
    match error {
        ClipboardError::Denied => ClipboardOutcome::Denied,
        ClipboardError::Unavailable => ClipboardOutcome::Unavailable,
        _ => ClipboardOutcome::Failed,
    }
}
fn pending(state: &State, consumer: Consumer) -> bool {
    match consumer {
        Consumer::Semantic {
            node,
            request,
            revision,
        } => state
            .mapper
            .tree()
            .clipboard_pending(node, request, revision)
            .unwrap_or(false),
        #[cfg(feature = "shell")]
        Consumer::Editor(lease) => editor::current(state, lease),
    }
}
fn available(state: &State, consumer: Consumer) -> bool {
    match consumer {
        Consumer::Semantic { node, .. } => {
            state.mapper.tree().active(node)
                && state.mapper.tree().enabled(node)
                && !state.mapper.tree().inert(node)
        }
        #[cfg(feature = "shell")]
        Consumer::Editor(lease) => editor::current(state, lease),
    }
}
fn finish(state: &mut State, consumer: Consumer, result: Completion) {
    if !pending(state, consumer) {
        return;
    }
    let (node, request, revision) = match consumer {
        Consumer::Semantic {
            node,
            request,
            revision,
        } => (node, request, revision),
        #[cfg(feature = "shell")]
        Consumer::Editor(lease) => {
            editor::finish(state, lease, result);
            return;
        }
    };
    let outcome = match result {
        Ok(_) => ClipboardOutcome::Written,
        Err(error) => outcome(error),
    };
    match state.mapper.finish_clipboard(node, request, revision) {
        Ok(true) => state.inputs.push(Input::Event {
            node,
            event: Event::Clipboard {
                request,
                revision,
                outcome,
            },
        }),
        Ok(false) => {}
        Err(error) => state.input_error = Some(error),
    }
}
pub(super) fn copy(
    state: &mut State,
    node: NodeId,
    revision: u64,
    request: u64,
) -> Result<(), Error> {
    let Some(config) = state.clipboard.config.clone() else {
        return Err(Error::Unsupported("native text clipboard write"));
    };
    if !state.layers.allowed(state.mapper.tree(), node) {
        return Err(Error::Unsupported(
            "clipboard source outside document modal scope",
        ));
    }
    state
        .mapper
        .admit_clipboard(glory_render::Action::CopyText {
            node,
            revision,
            request,
        })?;
    let consumer = Consumer::Semantic {
        node,
        revision,
        request,
    };
    if state.clipboard.active.is_some() || state.clipboard.next == u64::MAX {
        finish(state, consumer, Err(ClipboardError::Busy));
        state.observation_pending = true;
        return Ok(());
    }
    let text = state
        .mapper
        .tree()
        .node(node)?
        .descriptor
        .view()
        .semantics
        .as_ref()
        .and_then(|s| s.clipboard.as_ref())
        .expect("validated clipboard source")
        .text
        .clone();
    state.clipboard.next += 1;
    let ticket = state.clipboard.next;
    let future =
        Box::pin(async move { config.driver.write(text).await.map(|()| Payload::Written) });
    schedule(state, ticket, consumer, future, config.timeout);
    Ok(())
}
fn schedule(
    state: &mut State,
    ticket: u64,
    consumer: Consumer,
    future: Pin<Box<dyn Future<Output = Completion> + Send>>,
    timeout: std::time::Duration,
) {
    state.clipboard.active = Some(ticket);
    #[cfg(feature = "shell")]
    {
        state.clipboard.editor_cancelled = false;
    }
    state.clipboard.work = Some(Work {
        ticket,
        consumer,
        future,
        started: false,
        reported: false,
        deadline: Instant::now() + timeout,
        timer: Some(Delay::new(timeout)),
    });
    state.observation_pending = true;
}

pub(super) fn poll(renderer: &Renderer) {
    let (mut work, wake, valid) = {
        let mut state = renderer.0.borrow_mut();
        let Some(work) = state.clipboard.work.take() else {
            return;
        };
        let valid = pending(&state, work.consumer) && available(&state, work.consumer);
        (work, state.dom.resource_wake.clone(), valid)
    };
    if !work.started && (!valid || Instant::now() >= work.deadline) {
        let mut state = renderer.0.borrow_mut();
        if state.clipboard.active == Some(work.ticket) {
            state.clipboard.active = None;
            finish(&mut state, work.consumer, Err(ClipboardError::Failed));
        }
        drop(state);
        drop(work);
        return;
    }
    work.started = true;
    let waker = Waker::from(wake);
    let mut cx = Context::from_waker(&waker);
    let result = work.future.as_mut().poll(&mut cx);
    let expired = !work.reported
        && (Instant::now() >= work.deadline
            || work
                .timer
                .as_mut()
                .is_some_and(|timer| Pin::new(timer).poll(&mut cx).is_ready()));
    let mut state = renderer.0.borrow_mut();
    if state.clipboard.active != Some(work.ticket) {
        drop(state);
        drop(work);
        return;
    }
    match result {
        Poll::Ready(result) => {
            state.clipboard.active = None;
            if !work.reported {
                let result = if !expired && available(&state, work.consumer) {
                    result
                } else {
                    Err(ClipboardError::Failed)
                };
                finish(&mut state, work.consumer, result);
            }
        }
        Poll::Pending => {
            if expired {
                work.reported = true;
                finish(&mut state, work.consumer, Err(ClipboardError::Failed));
            }
            let timer = if expired { work.timer.take() } else { None };
            state.clipboard.work = Some(work);
            drop(state);
            drop(timer);
            return;
        }
    }
    drop(state);
    drop(work);
}
