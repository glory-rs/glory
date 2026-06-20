use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::reflow::scheduler::{BATCHING, RUNNING};
use crate::reflow::{PENDING_ITEMS, REVISING_ITEMS};
use crate::renderer::{Command, CommandNode, CommandQueue, CommandRenderer, EventData, QueryResponse};
use crate::{Holder, HolderId, ROOT_VIEWS, Scope, Truck, ViewId, Widget};

/// Headless holder for command-stream backends.
///
/// `CommandHolder` mounts a widget tree whose render mutations are recorded
/// as serializable [`Command`]s instead of touching a real UI. Hosts (the
/// desktop window host, tests, future native/TUI shells) drive it in a
/// simple transaction loop:
///
/// ```text
/// let holder = CommandHolder::new().mount(App);   // initial tree
/// sink.flush(holder.take_batch());                 // batch 1: full build
/// loop {
///     let event: EventData = host.next_event();
///     holder.dispatch_event(event);                // handler + reactive settle
///     sink.flush(holder.take_batch());             // batch N: minimal patch
/// }
/// ```
///
/// The widget tree and this holder are thread-local state — keep them on
/// one thread (the UI/event-loop thread) and marshal events onto it.
pub struct CommandHolder {
    id: HolderId,
    truck: Rc<RefCell<Truck>>,
    renderer: CommandRenderer,
    host_node: CommandNode,
    next_root_view_id: AtomicU64,
}

impl Default for CommandHolder {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHolder {
    pub fn new() -> Self {
        let queue = CommandQueue::new();
        let renderer = CommandRenderer::from_queue(queue.clone());
        let host_node = CommandNode::host(queue.clone());
        let id = HolderId::next();
        crate::renderer::command::register_holder_queue(id, queue);
        Self {
            id,
            truck: Rc::new(RefCell::new(Truck::new())),
            renderer,
            host_node,
            next_root_view_id: AtomicU64::new(0),
        }
    }

    pub fn renderer(&self) -> &CommandRenderer {
        &self.renderer
    }

    /// The reserved root node (id 0); consumers must map it to their root
    /// container (e.g. `document.body`).
    pub fn host_node(&self) -> &CommandNode {
        &self.host_node
    }

    /// Enables batch coalescing (recommended for IPC/remote sinks).
    pub fn set_coalesce(&self, enabled: bool) {
        self.renderer.queue().set_coalesce(enabled);
    }

    /// Drains the commands produced since the previous call. Call once
    /// after `mount` and once after every `dispatch_event`.
    pub fn take_batch(&self) -> Vec<Command> {
        self.renderer.take_batch()
    }

    /// Runs `f` with this holder's queue installed as thread-current and
    /// inside a reactive `batch`: every signal write settles — and its
    /// commands land in this holder's buffer — before `update` returns.
    ///
    /// Any programmatic state change from outside an event handler (timers,
    /// IPC pushes, test code) must go through here; otherwise widgets
    /// created during re-render would allocate from an isolated queue.
    pub fn update<R>(&self, f: impl FnOnce() -> R) -> R {
        let _guard = self.renderer.queue().make_current();
        crate::reflow::batch(self.id, f)
    }

    /// Delivers a backend event to its registered handler inside
    /// [`Self::update`]. Returns `false` when no handler is registered (a
    /// normal race when the node was removed while the event was in
    /// flight on IPC backends).
    pub fn dispatch_event(&self, data: EventData) -> bool {
        self.update(|| self.renderer.dispatch_event(data))
    }

    /// Delivers a consumer's answer to a pending node query.
    pub fn resolve_query(&self, response: QueryResponse) -> bool {
        self.update(|| self.renderer.resolve_query(response))
    }
}

impl Debug for CommandHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandHolder").field("id", &self.id).finish()
    }
}

impl Drop for CommandHolder {
    fn drop(&mut self) {
        crate::renderer::command::unregister_holder_queue(self.id);
        ROOT_VIEWS.with_borrow_mut(|root_views| {
            root_views.shift_remove(&self.id);
        });
        REVISING_ITEMS.with_borrow_mut(|revising_items| {
            revising_items.shift_remove(&self.id);
        });
        PENDING_ITEMS.with_borrow_mut(|pending_items| {
            pending_items.shift_remove(&self.id);
        });
        RUNNING.with_borrow_mut(|running| {
            running.shift_remove(&self.id);
        });
        BATCHING.with_borrow_mut(|batching| {
            batching.shift_remove(&self.id);
        });
    }
}

impl Holder for CommandHolder {
    fn mount(self, widget: impl Widget) -> Self {
        let _guard = self.renderer.queue().make_current();
        let view_id = ViewId::new(self.id, self.next_root_view_id.fetch_add(1, Ordering::Relaxed).to_string());
        let scope = Scope::new_root(view_id, self.truck.clone());
        widget.mount_to(scope, &self.host_node);
        self
    }

    fn truck(&self) -> Rc<RefCell<Truck>> {
        self.truck.clone()
    }
}
