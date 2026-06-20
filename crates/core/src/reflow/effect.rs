//! Imperative reactive effects.
//!
//! Whereas [`Bond`](super::Bond) produces a *value* that downstream
//! observers can read, an [`Effect`] is a side-effecting closure that
//! re-runs whenever its tracked dependencies change. It does not
//! produce a value visible to other signals.
//!
//! Effects are scoped to a parent [`Scope`]: they live as a child
//! `Widget` of that scope, so the host component dropping cancels
//! the effect automatically. No global lifetime, no leaks.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use educe::Educe;
use indexmap::IndexMap;

use super::{Revisable, RevisableId};
use crate::{Scope, ViewId, Widget};

/// A reactive effect widget.
///
/// Implements `Widget` so it plugs into the existing scheduler / scope
/// machinery without adding a parallel runtime. Has no DOM presence.
///
/// Use [`effect_in`] (this module) or [`Scope::effect`](crate::Scope::effect)
/// to construct one; direct construction is allowed but rarely useful.
#[derive(Educe)]
#[educe(Debug)]
pub struct Effect<F>
where
    F: FnMut() + 'static,
{
    #[educe(Debug(ignore))]
    closure: Rc<RefCell<F>>,
    gathers: IndexMap<RevisableId, Box<dyn Revisable>>,
}

impl<F> Effect<F>
where
    F: FnMut() + 'static,
{
    pub fn new(closure: F) -> Self {
        Self {
            closure: Rc::new(RefCell::new(closure)),
            gathers: IndexMap::new(),
        }
    }
}

impl<F> Widget for Effect<F>
where
    F: FnMut() + 'static,
{
    fn build(&mut self, ctx: &mut Scope) {
        let closure = self.closure.clone();
        let (gathers, ()) = super::gather(move || (closure.borrow_mut())());
        self.gathers = gathers;
        for gather in self.gathers.values() {
            gather.bind_view(ctx.view_id());
        }
    }

    fn patch(&mut self, ctx: &mut Scope) {
        // Drop subscriptions from the previous run; the new run will
        // pick up its own set (which may overlap or not).
        for gather in std::mem::take(&mut self.gathers).values() {
            gather.unbind_view(ctx.view_id());
        }
        let closure = self.closure.clone();
        let (gathers, ()) = super::gather(move || (closure.borrow_mut())());
        self.gathers = gathers;
        for gather in self.gathers.values() {
            gather.bind_view(ctx.view_id());
        }
    }

    fn detach(&mut self, ctx: &mut Scope) {
        for gather in std::mem::take(&mut self.gathers).values() {
            gather.unbind_view(ctx.view_id());
        }
        self.detach_children(ctx);
    }
}

/// Register a reactive side effect on `parent`. The closure runs once
/// immediately (so any tracked reads inside it subscribe), and re-runs
/// whenever any of those tracked dependencies revise.
///
/// Returns the effect's [`ViewId`] so the caller can detach it
/// explicitly via `parent.detach_child(...)` if needed; otherwise the
/// effect is dropped automatically when `parent` is detached.
///
/// ```ignore
/// use glory::reflow::{effect_in, Cage};
/// fn build(&mut self, ctx: &mut Scope) {
///     let count = self.count.clone();
///     effect_in(ctx, move || {
///         glory::info!("count is now {}", *count.get());
///     });
/// }
/// ```
pub fn effect_in<F>(parent: &mut Scope, closure: F) -> ViewId
where
    F: FnMut() + 'static,
{
    Effect::new(closure).show_in(parent)
}

/// Asynchronous derived signal — the "fetch on mount, re-fetch on
/// deps change" pattern as a one-liner.
///
/// `future_fn` is invoked synchronously inside an [`Effect`], so any
/// reactive reads it does (typically the URL or query argument it
/// captures) subscribe automatically. When deps change, the effect
/// re-runs, which builds a fresh future and spawns it; the returned
/// [`super::Cage`] is updated when that future resolves.
///
/// The previous future is not cancelled when dependencies change, but
/// stale completions are ignored. A slow request from an older run will
/// not overwrite the value produced by the latest run.
///
/// ```ignore
/// let user = resource_in(ctx, {
///     let id = self.id.clone();
///     move || {
///         let id = *id.get();
///         async move { fetch_user(id).await }
///     }
/// });
/// // ...read `user.get()` anywhere; it'll be `None` until the future
/// // resolves, then `Some(value)`.
/// ```
pub fn resource_in<T, F, Fut>(parent: &mut Scope, future_fn: F) -> super::Cage<Option<T>>
where
    T: std::fmt::Debug + 'static,
    F: Fn() -> Fut + 'static,
    Fut: std::future::Future<Output = T> + 'static,
{
    let cell = super::Cage::new(None::<T>);
    let cell_for_effect = cell;
    let generation = Rc::new(Cell::new(0_u64));
    let active_suspense_generation = Rc::new(Cell::new(0_u64));
    let suspense_boundary = parent.suspense_boundary;
    let future_fn = Rc::new(future_fn);
    effect_in(parent, move || {
        let future = (future_fn)();
        let cell = cell_for_effect;
        let run = ResourceRun::start(&generation, suspense_boundary, &active_suspense_generation);
        crate::spawn::spawn_local(async move {
            let val = future.await;
            run.commit(cell, val);
        });
    });
    cell
}

/// Hydration-aware variant of [`resource_in`].
///
/// Behaves exactly like [`resource_in`], except the resolved value is part of
/// the SSR ↔ hydration contract:
///
/// - **On the server**, when the future resolves its value is serialized and
///   recorded under a stable per-view token. The holder embeds these values in
///   the rendered page (see
///   [`ServerHolder`](crate::web::holders::ServerHolder)).
/// - **On the wasm client**, the same token is looked up first; if the server
///   streamed a value for it, the resource adopts that value immediately and
///   *skips the fetch entirely* — avoiding the double request that plain
///   [`resource_in`] incurs on hydration.
///
/// The extra `Serialize`/`DeserializeOwned` bounds are the price of that
/// contract; reach for [`resource_in`] when the value cannot or need not cross
/// the wire.
pub fn resource_hydratable_in<T, F, Fut>(parent: &mut Scope, future_fn: F) -> super::Cage<Option<T>>
where
    T: std::fmt::Debug + serde::Serialize + serde::de::DeserializeOwned + 'static,
    F: Fn() -> Fut + 'static,
    Fut: std::future::Future<Output = T> + 'static,
{
    let token = parent.next_resource_token();

    // Client hydration fast path: adopt the server-streamed value and skip the
    // fetch when it is present.
    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
    if let Some(value) = crate::web::take_hydrated_resource::<T>(&token) {
        return super::Cage::new(Some(value));
    }

    let cell = super::Cage::new(None::<T>);
    let cell_for_effect = cell;
    let generation = Rc::new(Cell::new(0_u64));
    let active_suspense_generation = Rc::new(Cell::new(0_u64));
    let suspense_boundary = parent.suspense_boundary;
    let future_fn = Rc::new(future_fn);
    let token = Rc::new(token);
    effect_in(parent, move || {
        let future = (future_fn)();
        let cell = cell_for_effect;
        #[cfg(all(feature = "web-ssr", not(feature = "single-app"), not(target_arch = "wasm32")))]
        let token = token.clone();
        #[cfg(not(all(feature = "web-ssr", not(feature = "single-app"), not(target_arch = "wasm32"))))]
        let _ = &token;
        let run = ResourceRun::start(&generation, suspense_boundary, &active_suspense_generation);
        crate::spawn::spawn_local(async move {
            let val = future.await;
            // Capture for hydration before the value is consumed by `commit`,
            // but only persist it when this run is the one that wins the cell.
            #[cfg(all(feature = "web-ssr", not(feature = "single-app"), not(target_arch = "wasm32")))]
            let json = serde_json::to_string(&val).ok();
            let committed = run.commit(cell, val);
            #[cfg(all(feature = "web-ssr", not(feature = "single-app"), not(target_arch = "wasm32")))]
            if committed && let Some(json) = json {
                crate::stream_ssr::record_resource_json(&token, json);
            }
            #[cfg(not(all(feature = "web-ssr", not(feature = "single-app"), not(target_arch = "wasm32"))))]
            let _ = committed;
        });
    });
    cell
}

/// Fire-and-forget async task scoped to `parent`.
///
/// Unlike [`resource_in`], `use_future_in` does not produce a value cell — it
/// is the "spawn this future, and re-spawn it when my reactive inputs change"
/// pattern. `future_fn` is invoked synchronously inside an [`Effect`], so any
/// reactive reads it does before returning the future subscribe automatically;
/// when those deps revise, a fresh future is built and spawned.
///
/// The task is tied to `parent`'s lifecycle: detaching the scope drops the
/// effect, so no new future is spawned afterwards. (An in-flight future from a
/// prior run is not actively cancelled — model long-lived loops with
/// [`use_coroutine_in`] when you need an explicit shutdown channel.)
///
/// ```ignore
/// use glory::reflow::use_future_in;
/// fn build(&mut self, ctx: &mut Scope) {
///     let id = self.id; // Cage<u64>, Copy
///     use_future_in(ctx, move || {
///         let id = *id.get();
///         async move { log_visit(id).await; }
///     });
/// }
/// ```
pub fn use_future_in<F, Fut>(parent: &mut Scope, future_fn: F) -> ViewId
where
    F: Fn() -> Fut + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let future_fn = Rc::new(future_fn);
    effect_in(parent, move || {
        let fut = (future_fn)();
        crate::spawn::spawn_local(fut);
    })
}

/// Handle to a coroutine started with [`use_coroutine_in`]. Cloneable and
/// `Copy`-friendly to move into event handlers; call [`Coroutine::send`] to
/// push a message to the running task.
#[derive(Educe)]
#[educe(Debug)]
pub struct Coroutine<M> {
    #[educe(Debug(ignore))]
    tx: futures::channel::mpsc::UnboundedSender<M>,
}

impl<M> Clone for Coroutine<M> {
    fn clone(&self) -> Self {
        Self { tx: self.tx.clone() }
    }
}

impl<M> Coroutine<M> {
    /// Send a message to the coroutine. Returns `false` if the coroutine has
    /// already finished (its receiver was dropped).
    pub fn send(&self, message: M) -> bool {
        self.tx.unbounded_send(message).is_ok()
    }
}

/// Long-lived, message-driven async task scoped to `parent`.
///
/// `build` receives the receiving end of an unbounded channel and returns the
/// future that drives the coroutine. The returned [`Coroutine`] handle owns the
/// sender; store it (e.g. in your widget) and call [`Coroutine::send`] from
/// event handlers to feed the task. When every handle is dropped — typically
/// when the host widget detaches — the receiver yields `None`, letting a
/// `while let Some(msg) = rx.next().await` loop terminate cleanly.
///
/// ```ignore
/// use glory::reflow::{use_coroutine_in, Coroutine};
/// use futures::StreamExt;
/// // in build:
/// let worker: Coroutine<String> = use_coroutine_in(ctx, |mut rx| async move {
///     while let Some(line) = rx.next().await {
///         send_to_server(line).await;
///     }
/// });
/// // later, in an event handler: worker.send("hello".into());
/// ```
pub fn use_coroutine_in<M, F, Fut>(parent: &mut Scope, build: F) -> Coroutine<M>
where
    M: 'static,
    F: FnOnce(futures::channel::mpsc::UnboundedReceiver<M>) -> Fut,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let _ = parent;
    let (tx, rx) = futures::channel::mpsc::unbounded::<M>();
    let fut = build(rx);
    crate::spawn::spawn_local(fut);
    Coroutine { tx }
}

#[derive(Clone, Debug)]
struct ResourceRun {
    generation: Rc<Cell<u64>>,
    value: u64,
    suspense: Option<ResourceSuspenseRun>,
}

impl ResourceRun {
    fn start(
        generation: &Rc<Cell<u64>>,
        suspense_boundary: Option<crate::scope::SuspenseBoundary>,
        active_suspense_generation: &Rc<Cell<u64>>,
    ) -> Self {
        let value = generation.get().wrapping_add(1);
        generation.set(value);
        let suspense = suspense_boundary.map(|boundary| ResourceSuspenseRun::start(boundary, active_suspense_generation, value));
        Self {
            generation: generation.clone(),
            value,
            suspense,
        }
    }

    fn is_current(&self) -> bool {
        self.generation.get() == self.value
    }

    fn commit<T>(&self, cell: super::Cage<Option<T>>, value: T) -> bool
    where
        T: std::fmt::Debug + 'static,
    {
        if !self.is_current() {
            self.finish_suspense();
            return false;
        }

        cell.revise(|mut current| *current = Some(value));
        self.finish_suspense();
        true
    }

    fn finish_suspense(&self) {
        if let Some(suspense) = &self.suspense {
            suspense.finish();
        }
    }
}

#[derive(Clone, Debug)]
struct ResourceSuspenseRun {
    boundary: crate::scope::SuspenseBoundary,
    active_generation: Rc<Cell<u64>>,
    value: u64,
}

impl ResourceSuspenseRun {
    fn start(boundary: crate::scope::SuspenseBoundary, active_generation: &Rc<Cell<u64>>, value: u64) -> Self {
        if active_generation.replace(value) != 0 {
            boundary.finish();
        }
        boundary.start();
        Self {
            boundary,
            active_generation: active_generation.clone(),
            value,
        }
    }

    fn finish(&self) {
        if self.active_generation.get() == self.value {
            self.active_generation.set(0);
            self.boundary.finish();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coroutine_send_delivers_until_handle_dropped() {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<u32>();
        let handle = Coroutine { tx };
        assert!(handle.send(1));
        let clone = handle.clone();
        assert!(clone.send(2));
        assert_eq!(rx.try_recv().ok(), Some(1));
        assert_eq!(rx.try_recv().ok(), Some(2));
        // Dropping every sender lets the receiver's stream terminate.
        drop(handle);
        drop(clone);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn resource_run_ignores_stale_completion() {
        let generation = Rc::new(Cell::new(0_u64));
        let cell = super::super::Cage::new(None::<i32>);

        let active_suspense_generation = Rc::new(Cell::new(0_u64));
        let slow_run = ResourceRun::start(&generation, None, &active_suspense_generation);
        let fast_run = ResourceRun::start(&generation, None, &active_suspense_generation);

        assert!(fast_run.commit(cell, 20));
        assert_eq!(*cell.get_untracked(), Some(20));

        assert!(!slow_run.commit(cell, 10));
        assert_eq!(*cell.get_untracked(), Some(20));
    }

    #[test]
    fn resource_run_updates_suspense_pending_for_latest_generation() {
        let generation = Rc::new(Cell::new(0_u64));
        let active_suspense_generation = Rc::new(Cell::new(0_u64));
        let cell = super::super::Cage::new(None::<i32>);
        let pending = super::super::Cage::new(0_usize);
        let boundary = crate::scope::SuspenseBoundary::new(pending);

        let slow_run = ResourceRun::start(&generation, Some(boundary), &active_suspense_generation);
        assert_eq!(*pending.get_untracked(), 1);

        let fast_run = ResourceRun::start(&generation, Some(boundary), &active_suspense_generation);
        assert_eq!(*pending.get_untracked(), 1);

        assert!(!slow_run.commit(cell, 10));
        assert_eq!(*pending.get_untracked(), 1);

        assert!(fast_run.commit(cell, 20));
        assert_eq!(*pending.get_untracked(), 0);
        assert_eq!(*cell.get_untracked(), Some(20));
    }
}
