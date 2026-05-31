//! Fine-grained reactivity primitives.
//!
//! Glory's reactivity is built on three concrete types and a thread-local
//! scheduler:
//!
//! - [`Cage<T>`][Cage] — mutable cell. Reads in a tracking context
//!   subscribe; writes via `revise` queue re-renders for subscribers.
//! - [`Bond<T>`][Bond] — derived value with a mapper closure. Re-runs
//!   when any captured dependency's `(id, version)` changes. Optional
//!   equality gate via [`Bond::with_eq`].
//! - [`Lotus<T>`][Lotus] — read-only enum of `Bare(T) | Cage(_) |
//!   Bond(_)`. Use it for "anything reactively observable" type
//!   parameters.
//!
//! Scheduling primitives:
//! - [`batch`] — defer signal propagation until the closure returns,
//!   then flush all re-renders once. Use around code that does many
//!   writes in a row.
//! - [`untrack`] — suppress signal propagation for writes (the
//!   re-renders are dropped, not just deferred). Rare; mainly for
//!   bookkeeping mutations.
//! - [`untracked_read`] — peek at a reactive value without subscribing
//!   the current tracking layer. Common inside `Bond` mappers that
//!   want to read auxiliary state.
//! - [`schedule`] — internal; invoked by `Cage::revise`. Walks
//!   `REVISING_ITEMS`, calls `Widget::patch` for each bound view.

mod cage;
pub mod storage;
pub use cage::Cage;
mod bond;
pub use bond::{Bond, selector};
mod owner;
pub use owner::Owner;
mod lotus;
pub use lotus::Lotus;
mod effect;
pub use effect::{Effect, effect_in, resource_in};
pub mod scheduler;
pub use scheduler::{batch, schedule};

use std::cell::RefCell;
use std::fmt::{self, Display};
use std::hash::Hash;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use indexmap::{IndexMap, IndexSet};

#[cfg(not(feature = "single-app"))]
use crate::HolderId;
use crate::ViewId;

thread_local! {
    #[cfg(feature = "single-app")]
    pub(crate) static REVISING_ITEMS: RefCell<IndexMap<RevisableId, Box<dyn Revisable>>> = RefCell::default();
    #[cfg(not(feature = "single-app"))]
    pub(crate) static REVISING_ITEMS: RefCell<IndexMap<HolderId, IndexMap<RevisableId, Box<dyn Revisable>>>> = RefCell::default();

    #[cfg(feature = "single-app")]
    pub(crate) static PENDING_ITEMS: RefCell<IndexMap<RevisableId, Box<dyn Revisable>>> = RefCell::default();
    #[cfg(not(feature = "single-app"))]
    pub(crate) static PENDING_ITEMS: RefCell<IndexMap<HolderId, IndexMap<RevisableId, Box<dyn Revisable>>>> = RefCell::default();

    pub(crate) static TRACKING_STACK: RefCell<TrackingStack> = RefCell::new(TrackingStack::new());
}

#[derive(Default)]
pub(crate) struct TrackingStack {
    pub layers: Vec<IndexMap<RevisableId, Box<dyn Revisable>>>,
}
impl TrackingStack {
    pub(crate) fn new() -> Self {
        Self { layers: Default::default() }
    }
    pub(crate) fn is_idle(&self) -> bool {
        self.layers.is_empty()
    }
    pub(crate) fn push_layer(&mut self) {
        self.layers.push(Default::default());
    }
    pub(crate) fn pop_layer(&mut self) -> Option<IndexMap<RevisableId, Box<dyn Revisable>>> {
        self.layers.pop()
    }
    pub(crate) fn track(&mut self, item: impl Into<Box<dyn Revisable>>) {
        let item = item.into();
        for layer in &mut self.layers {
            layer.insert(item.id(), item.clone_boxed());
        }
    }
}

pub fn gather<R>(func: impl FnOnce() -> R) -> (IndexMap<RevisableId, Box<dyn Revisable>>, R) {
    TRACKING_STACK.with(|tracking_items| tracking_items.borrow_mut().push_layer());
    let result = (func)();
    let gathers = TRACKING_STACK.with(|tracking_items| tracking_items.borrow_mut().pop_layer().unwrap());
    (gathers, result)
}

/// Read reactive values inside `func` **without subscribing** the current
/// tracking layer to them. Useful inside a `Bond` mapper or a widget
/// `build` / `patch` where you want to peek at a `Cage` / `Bond` value
/// without forcing a re-run when that source changes.
///
/// ```ignore
/// let derived = Bond::new(move || {
///     // re-runs when `must_track` changes
///     let a = *must_track.get();
///     // intentionally does NOT subscribe to `dont_track`
///     let b = untracked_read(|| *dont_track.get());
///     a + b
/// });
/// ```
///
/// Implementation note: temporarily swaps out the thread-local
/// `TRACKING_STACK` so reads cannot push into any active gather layer.
/// Restores the previous stack on return, even if `func` panics, so
/// downstream tracking is unaffected.
pub fn untracked_read<R>(func: impl FnOnce() -> R) -> R {
    struct Guard(Option<TrackingStack>);
    impl Drop for Guard {
        fn drop(&mut self) {
            if let Some(saved) = self.0.take() {
                TRACKING_STACK.with(|stack| *stack.borrow_mut() = saved);
            }
        }
    }
    let saved = TRACKING_STACK.with(|stack| std::mem::take(&mut *stack.borrow_mut()));
    let _guard = Guard(Some(saved));
    func()
}

/// Suppress signal propagation for the duration of `opt`. Writes via
/// `Cage::revise` inside `opt` mutate the state and bump the cage
/// version, but do not enqueue re-renders on bound views; observers
/// will still notice the bumped version the next time they read.
///
/// Different from [`gather`] / [`untracked_read`]: this gates the WRITE
/// side (signal/scheduling), not the read side.
///
/// Different from [`batch`]: `batch` defers and then flushes all queued
/// re-renders, whereas `untrack` drops them.
///
/// Mainly useful for one-shot bookkeeping mutations that don't represent
/// user-visible state changes (e.g. updating an internal cursor while
/// processing a list).
#[cfg(feature = "single-app")]
pub fn untrack<O, R>(opt: O) -> R
where
    O: FnOnce() -> R,
{
    scheduler::UNTRACKING.with(|untracking| {
        if !untracking.get() {
            untracking.set(true);
            let out = opt();
            untracking.set(false);
            out
        } else {
            opt()
        }
    })
}

/// See the single-holder variant above. In multi-holder mode the
/// suppression is scoped to a specific `HolderId`.
#[cfg(not(feature = "single-app"))]
pub fn untrack<O, R>(holder_id: HolderId, opt: O) -> R
where
    O: FnOnce() -> R,
{
    scheduler::UNTRACKING.with(|untracking| {
        if !untracking.borrow().get(&holder_id).map(|v| *v).unwrap_or(false) {
            untracking.borrow_mut().insert(holder_id, true);
            let out = opt();
            untracking.borrow_mut().insert(holder_id, false);
            out
        } else {
            opt()
        }
    })
}

static NEXT_REVISABLE_ID: AtomicU64 = AtomicU64::new(1);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RevisableId(u64);
impl RevisableId {
    pub fn next() -> RevisableId {
        RevisableId(NEXT_REVISABLE_ID.fetch_add(1, Ordering::Relaxed))
    }
}
impl Display for RevisableId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RevisableId({})", self.0)
    }
}

pub trait Revisable: fmt::Debug {
    fn id(&self) -> RevisableId;
    #[cfg(not(feature = "single-app"))]
    fn holder_id(&self) -> Option<HolderId>;
    fn version(&self) -> usize;
    fn view_ids(&self) -> Rc<RefCell<IndexSet<ViewId>>>;
    fn bind_view(&self, view_id: &ViewId);
    fn unbind_view(&self, view_id: &ViewId);
    fn unlace_view(&self, view_id: &ViewId, loose: usize);
    fn is_revising(&self) -> bool {
        REVISING_ITEMS.with_borrow(|revising_items| {
            cfg_if! {
                if #[cfg(feature = "single-app")] {
                    revising_items.contains_key(&self.id())
                } else {
                    if let Some(holder_id) = self.holder_id() {
                        revising_items.get(&holder_id).map(|items|items.contains_key(&self.id())).unwrap_or(false)
                    } else {
                        tracing::debug!("Revisable::is_revising: holder_id is None");
                        false
                    }
                }
            }
        })
    }
    fn clone_boxed(&self) -> Box<dyn Revisable>;
}
impl Eq for dyn Revisable {}
impl PartialEq for dyn Revisable {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}
impl Hash for dyn Revisable {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}
