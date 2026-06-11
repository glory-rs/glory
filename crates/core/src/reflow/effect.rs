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
    let future_fn = Rc::new(future_fn);
    effect_in(parent, move || {
        let future = (future_fn)();
        let cell = cell_for_effect;
        let run = ResourceRun::start(&generation);
        crate::spawn::spawn_local(async move {
            let val = future.await;
            run.commit(cell, val);
        });
    });
    cell
}

#[derive(Clone, Debug)]
struct ResourceRun {
    generation: Rc<Cell<u64>>,
    value: u64,
}

impl ResourceRun {
    fn start(generation: &Rc<Cell<u64>>) -> Self {
        let value = generation.get().wrapping_add(1);
        generation.set(value);
        Self {
            generation: generation.clone(),
            value,
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
            return false;
        }

        cell.revise(|mut current| *current = Some(value));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_run_ignores_stale_completion() {
        let generation = Rc::new(Cell::new(0_u64));
        let cell = super::super::Cage::new(None::<i32>);

        let slow_run = ResourceRun::start(&generation);
        let fast_run = ResourceRun::start(&generation);

        assert!(fast_run.commit(cell, 20));
        assert_eq!(*cell.get_untracked(), Some(20));

        assert!(!slow_run.commit(cell, 10));
        assert_eq!(*cell.get_untracked(), Some(20));
    }
}
