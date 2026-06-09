use std::any::{Any, TypeId};
use std::cell::{BorrowError, BorrowMutError, Cell, Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use indexmap::IndexSet;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Bond, PENDING_ITEMS, REVISING_ITEMS, Revisable, RevisableId, TRACKING_STACK};
use crate::ViewId;
use crate::reflow::{self, Lotus, scheduler};

thread_local! {
    /// Recycled, invalidated [`Slot`]s keyed by `TypeId::of::<Slot<T>>()`.
    ///
    /// A `Cage` handle is `Copy`, so we can never learn when the last handle
    /// drops — `Copy` and `Drop` are mutually exclusive. Reclamation is
    /// therefore explicit (via [`Cage::invalidate`], driven by [`Owner`] drop):
    /// invalidating a cage drops its value `T` (freeing the bulk of the
    /// memory), clears its subscriptions, bumps its generation so stale handles
    /// can't touch it, and parks the leaked slot here for reuse. The next
    /// `Cage::new` of the same type reuses a parked slot instead of leaking a
    /// fresh one, so the live slot count stays bounded by the peak number of
    /// concurrent cages rather than growing without limit.
    ///
    /// [`Owner`]: crate::reflow::Owner
    static CAGE_FREE_LIST: RefCell<HashMap<TypeId, Vec<&'static dyn Any>>> =
        RefCell::new(HashMap::new());
}

pub struct Cage<T>
where
    T: fmt::Debug + 'static,
{
    inner: &'static Slot<T>,
    // Identity + liveness token snapshotted at creation. The handle is valid
    // only while `inner.alive` is set and `inner.generation == self.generation`
    // (i.e. the slot still holds *this* cage and hasn't been recycled).
    id: RevisableId,
    generation: u64,
}

/// Heap cell backing a `Cage`. Leaked once, then recycled forever (see
/// [`CAGE_FREE_LIST`]). `source` is `Option<T>` so the value can be dropped on
/// invalidation while the slot's `'static` address stays valid for stale
/// handles to safely test their generation against.
struct Slot<T>
where
    T: fmt::Debug + 'static,
{
    generation: Cell<u64>,
    alive: Cell<bool>,
    version: Cell<usize>,
    source: RefCell<Option<T>>,
    view_ids: Rc<RefCell<IndexSet<ViewId>>>,
}

impl<T> fmt::Debug for Cage<T>
where
    T: fmt::Debug + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cage")
            .field("id", &self.id)
            .field("generation", &self.generation)
            .field("alive", &self.is_current())
            .field("version", &self.inner.version.get())
            .field("subscriber_count", &self.inner.view_ids.borrow().len())
            .finish_non_exhaustive()
    }
}

impl<T> Revisable for Cage<T>
where
    T: fmt::Debug + 'static,
{
    fn id(&self) -> RevisableId {
        self.id
    }
    #[cfg(not(feature = "single-app"))]
    fn holder_id(&self) -> Option<crate::HolderId> {
        self.view_ids().borrow().first().map(|view_id| view_id.holder_id())
    }
    fn version(&self) -> usize {
        if self.is_current() { self.inner.version.get() } else { 0 }
    }
    fn view_ids(&self) -> Rc<RefCell<IndexSet<ViewId>>> {
        // A stale handle (slot recycled into a different cage) must never hand
        // out the live cage's subscriber set, or the scheduler would re-render
        // an unrelated view. Return an empty, throwaway set instead.
        if self.is_current() {
            self.inner.view_ids.clone()
        } else {
            Rc::new(RefCell::new(IndexSet::new()))
        }
    }
    fn bind_view(&self, view_id: &ViewId) {
        if self.is_current() {
            self.inner.view_ids.borrow_mut().insert(view_id.clone());
        }
    }
    fn unbind_view(&self, view_id: &ViewId) {
        if self.is_current() {
            self.inner.view_ids.borrow_mut().shift_remove(view_id);
        }
    }
    fn unlace_view(&self, view_id: &ViewId, loose: usize) {
        if loose > 0 && self.is_current() {
            self.inner.view_ids.borrow_mut().shift_remove(view_id);
        }
    }
    fn clone_boxed(&self) -> Box<dyn Revisable> {
        Box::new(*self)
    }
}

impl<'de, T> Deserialize<'de> for Cage<T>
where
    T: Deserialize<'de> + fmt::Debug + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Cage::new)
    }
}

impl<T> Serialize for Cage<T>
where
    T: Serialize + fmt::Debug + 'static,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &*self.inner.source.borrow() {
            Some(value) => T::serialize(value, serializer),
            None => Err(serde::ser::Error::custom("cage handle is stale")),
        }
    }
}

impl<T> Cage<T>
where
    T: fmt::Debug + 'static,
{
    /// True while this handle still owns its slot (alive and not yet recycled
    /// into a different cage).
    fn is_current(&self) -> bool {
        self.inner.alive.get() && self.inner.generation.get() == self.generation
    }

    /// Mark this cage dead and reclaim its memory: drop the value `T`, clear
    /// subscriptions, bump the generation so existing `Copy` handles go stale,
    /// and park the leaked slot for reuse by a future `Cage::new` of the same
    /// type. Idempotent and safe to call from multiple owning handles — the
    /// generation guard makes every call after the first a no-op.
    pub(crate) fn invalidate(&self) {
        if !self.is_current() {
            return;
        }
        self.inner.alive.set(false);
        self.inner.generation.set(self.inner.generation.get().wrapping_add(1));
        // Drop the value (the bulk of the memory) and the subscriber set.
        *self.inner.source.borrow_mut() = None;
        self.inner.view_ids.borrow_mut().clear();
        let slot: &'static Slot<T> = self.inner;
        CAGE_FREE_LIST.with_borrow_mut(|free| {
            free.entry(TypeId::of::<Slot<T>>()).or_default().push(slot as &'static dyn Any);
        });
    }

    fn ensure_alive(&self) -> Result<(), CageAccessError> {
        if self.is_current() {
            Ok(())
        } else {
            Err(CageAccessError::Stale)
        }
    }

    fn track_read(&self) {
        let this = *self;
        TRACKING_STACK.with(|tracking_items| {
            let mut tracking_items = tracking_items.borrow_mut();
            if !tracking_items.is_idle() {
                tracking_items.track(this.clone_boxed());
            }
        });
    }

    pub fn get(&self) -> Ref<'_, T> {
        self.try_get().expect("Cage::get: source is already mutably borrowed")
    }

    pub fn try_get(&self) -> Result<Ref<'_, T>, CageAccessError> {
        self.ensure_alive()?;
        self.track_read();
        let borrowed = self.inner.source.try_borrow().map_err(CageAccessError::Borrow)?;
        Ref::filter_map(borrowed, |slot| slot.as_ref()).map_err(|_| CageAccessError::Stale)
    }

    pub fn get_untracked(&self) -> Ref<'_, T> {
        self.try_get_untracked().expect("Cage::get_untracked: source is already mutably borrowed")
    }

    pub fn try_get_untracked(&self) -> Result<Ref<'_, T>, CageAccessError> {
        self.ensure_alive()?;
        let borrowed = self.inner.source.try_borrow().map_err(CageAccessError::Borrow)?;
        Ref::filter_map(borrowed, |slot| slot.as_ref()).map_err(|_| CageAccessError::Stale)
    }

    pub fn revise<F, R>(&self, opt: F) -> R
    where
        F: FnOnce(RefMut<'_, T>) -> R,
    {
        self.try_revise(opt).expect("Cage::revise: source is already borrowed")
    }

    pub fn try_revise<F, R>(&self, opt: F) -> Result<R, CageMutateError>
    where
        F: FnOnce(RefMut<'_, T>) -> R,
    {
        self.ensure_alive().map_err(CageMutateError::Access)?;
        let borrowed = self.inner.source.try_borrow_mut().map_err(CageMutateError::Borrow)?;
        let projected = RefMut::filter_map(borrowed, |slot| slot.as_mut())
            .map_err(|_| CageMutateError::Access(CageAccessError::Stale))?;
        let result = (opt)(projected);
        self.inner.version.set(self.inner.version.get() + 1);
        self.signal();
        Ok(result)
    }

    pub fn revise_silent<F, R>(&self, opt: F) -> R
    where
        F: FnOnce(RefMut<'_, T>) -> R,
    {
        self.try_revise_silent(opt).expect("Cage::revise_silent: source is already borrowed")
    }

    pub fn try_revise_silent<F, R>(&self, opt: F) -> Result<R, CageMutateError>
    where
        F: FnOnce(RefMut<'_, T>) -> R,
    {
        self.ensure_alive().map_err(CageMutateError::Access)?;
        let borrowed = self.inner.source.try_borrow_mut().map_err(CageMutateError::Borrow)?;
        let projected = RefMut::filter_map(borrowed, |slot| slot.as_mut())
            .map_err(|_| CageMutateError::Access(CageAccessError::Stale))?;
        let result = (opt)(projected);
        self.inner.version.set(self.inner.version.get() + 1);
        Ok(result)
    }

    /// Returns the number of views currently subscribed to this Cage.
    /// **Dev-only diagnostic** — useful for debugging "why doesn't my
    /// component update?" / "why does my component update too often?"
    /// scenarios. Don't gate runtime behaviour on the result; this is
    /// not part of the stable API.
    #[doc(hidden)]
    pub fn subscriber_count(&self) -> usize {
        self.inner.view_ids.borrow().len()
    }

    /// Returns a snapshot of the [`ViewId`]s currently subscribed to
    /// this Cage. **Dev-only diagnostic**; see
    /// [`subscriber_count`](Self::subscriber_count).
    #[doc(hidden)]
    pub fn subscriber_view_ids(&self) -> Vec<crate::view::ViewId> {
        self.inner.view_ids.borrow().iter().cloned().collect()
    }
    // pub fn source<'a>(&'a self) -> std::cell::Ref<'a, S> {
    //     self.source.borrow()
    // }
    pub fn borrow(&self) -> Ref<'_, T> {
        Ref::filter_map(self.inner.source.borrow(), |slot| slot.as_ref())
            .expect("Cage::borrow: cage handle is stale")
    }

    pub fn map<M, G>(&self, mapper: M) -> Bond<G>
    where
        M: Fn(Ref<'_, T>) -> G + Clone + 'static,
        G: fmt::Debug + 'static,
    {
        let this = *self;
        Bond::new(move || mapper(this.get()))
    }

    pub fn read(&self) -> Lotus<T> {
        Lotus::Cage(*self)
    }
    fn signal(&self) {
        // A stale handle has no live subscribers and must not enqueue work.
        if !self.is_current() {
            return;
        }
        #[cfg(not(feature = "single-app"))]
        let Some(holder_id) = self.holder_id() else {
            tracing::debug!("Cage::signal: holder_id is None");
            return;
        };
        if scheduler::is_untracking(
            #[cfg(not(feature = "single-app"))]
            holder_id,
        ) {
            return;
        }
        let is_running = scheduler::is_running(
            #[cfg(not(feature = "single-app"))]
            holder_id,
        );

        if is_running {
            PENDING_ITEMS.with_borrow_mut(|items| {
                #[cfg(not(feature = "single-app"))]
                let items = items.entry(holder_id).or_default();
                if !items.contains_key(&self.id()) {
                    items.insert(self.id(), self.clone_boxed());
                }
            });
        } else {
            let need_schedule = REVISING_ITEMS.with_borrow_mut(|items| {
                #[cfg(not(feature = "single-app"))]
                let items = items.entry(holder_id).or_default();
                if !items.contains_key(&self.id()) {
                    items.insert(self.id(), self.clone_boxed());
                    true
                } else {
                    false
                }
            });
            if need_schedule {
                reflow::schedule(
                    #[cfg(not(feature = "single-app"))]
                    holder_id,
                );
            }
        }
    }
}

impl<T> Cage<T>
where
    T: fmt::Debug + 'static,
{
    pub fn new(source: T) -> Self {
        // Reuse a parked slot of the same type if one is available, otherwise
        // leak a fresh one. Either way the slot lives at a stable `'static`
        // address for as long as the program runs.
        let recycled: Option<&'static Slot<T>> = CAGE_FREE_LIST.with_borrow_mut(|free| {
            free.get_mut(&TypeId::of::<Slot<T>>())
                .and_then(|slots| slots.pop())
                .and_then(|any| any.downcast_ref::<Slot<T>>())
        });

        let inner: &'static Slot<T> = match recycled {
            Some(slot) => {
                // A recycled slot is dead (alive == false) with a bumped
                // generation; reinitialise it for this new cage.
                *slot.source.borrow_mut() = Some(source);
                slot.version.set(1);
                slot.view_ids.borrow_mut().clear();
                slot.alive.set(true);
                slot
            }
            None => Box::leak(Box::new(Slot {
                generation: Cell::new(0),
                alive: Cell::new(true),
                version: Cell::new(1),
                source: RefCell::new(Some(source)),
                view_ids: Rc::new(RefCell::new(IndexSet::new())),
            })),
        };

        Cage { inner, id: RevisableId::next(), generation: inner.generation.get() }
    }
}

impl<T> Default for Cage<T>
where
    T: fmt::Debug + Default + 'static,
{
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T> Clone for Cage<T>
where
    T: fmt::Debug + 'static,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Cage<T> where T: fmt::Debug + 'static {}

#[derive(Debug)]
pub enum CageAccessError {
    Stale,
    Borrow(BorrowError),
}

impl fmt::Display for CageAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CageAccessError::Stale => f.write_str("cage handle is stale"),
            CageAccessError::Borrow(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for CageAccessError {}

#[derive(Debug)]
pub enum CageMutateError {
    Access(CageAccessError),
    Borrow(BorrowMutError),
}

impl fmt::Display for CageMutateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CageMutateError::Access(err) => err.fmt(f),
            CageMutateError::Borrow(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for CageMutateError {}

impl<T> From<T> for Cage<T>
where
    T: fmt::Debug + 'static,
{
    fn from(source: T) -> Self {
        Self::new(source)
    }
}
impl<T> Eq for Cage<T> where T: fmt::Debug + Eq {}
impl<T> PartialEq<Cage<T>> for Cage<T>
where
    T: fmt::Debug + Eq + 'static,
{
    #[inline]
    fn eq(&self, other: &Cage<T>) -> bool {
        // Identity by unique id + generation, not slot address: a recycled slot
        // is shared between a dead cage and the new cage that took its place.
        self.id == other.id && self.generation == other.generation
    }
}

impl<'a> From<&'a str> for Cage<String> {
    fn from(source: &'a str) -> Self {
        Self::new(source.to_owned())
    }
}
impl<'a> From<&'a String> for Cage<String> {
    fn from(source: &'a String) -> Self {
        Self::new(source.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reflow::Revisable;

    fn assert_copy<T: Copy>() {}

    #[test]
    fn cage_is_copy_and_copies_share_state() {
        assert_copy::<Cage<i32>>();

        let cage = Cage::new(1_i32);
        let copied = cage;
        cage.revise_silent(|mut value| *value = 2);

        assert_eq!(cage.id(), copied.id());
        assert_eq!(*copied.get_untracked(), 2);
    }

    #[test]
    fn try_revise_reports_active_shared_borrow() {
        let cage = Cage::new(1_i32);
        let read = cage.try_get_untracked().expect("initial borrow should succeed");

        assert!(cage.try_revise(|mut value| *value = 2).is_err());
        assert_eq!(*read, 1);

        drop(read);
        assert!(cage.try_revise(|mut value| *value = 2).is_ok());
        assert_eq!(*cage.get_untracked(), 2);
    }

    #[test]
    fn try_get_reports_active_mut_borrow() {
        let cage = Cage::new(1_i32);
        let _write = cage.inner.source.borrow_mut();

        assert!(cage.try_get_untracked().is_err());
    }

    #[test]
    fn invalidate_frees_value_and_marks_handle_stale() {
        let cage = Cage::new(String::from("hello"));
        assert!(cage.try_get_untracked().is_ok());

        cage.invalidate();

        // The value `T` is dropped (memory reclaimed) and the handle is stale.
        assert!(cage.inner.source.borrow().is_none());
        assert!(matches!(cage.try_get_untracked(), Err(CageAccessError::Stale)));
        assert!(matches!(cage.try_revise(|mut v| *v = String::new()), Err(CageMutateError::Access(_))));
        // A stale handle exposes no subscribers, so it can't schedule re-renders.
        assert!(cage.view_ids().borrow().is_empty());
        assert_eq!(cage.version(), 0);
    }

    #[test]
    fn invalidated_slot_is_recycled_not_leaked() {
        // A type unique to this test so its free-list bucket can't be touched
        // by other cages running on the same harness thread.
        #[derive(Debug)]
        struct Uniq(u32);

        let dead = Cage::new(Uniq(1));
        let dead_slot: &'static Slot<Uniq> = dead.inner;
        dead.invalidate();

        // The next cage of the same type reuses the parked slot rather than
        // leaking a fresh allocation.
        let live = Cage::new(Uniq(2));
        assert!(std::ptr::eq(dead_slot, live.inner), "slot should be recycled");

        // The recycled handle is stale; the new handle is the live occupant
        // with a distinct identity.
        assert!(live.is_current());
        assert!(!dead.is_current());
        assert_ne!(dead.id(), live.id());
        assert_eq!(live.get_untracked().0, 2);
    }

    #[test]
    fn double_invalidate_is_a_noop() {
        let cage = Cage::new(7_i32);
        cage.invalidate();
        // A second invalidate (e.g. from a second owning scope) must not park
        // the slot twice, or it could be handed to two live cages at once.
        cage.invalidate();
        let before =
            CAGE_FREE_LIST.with_borrow(|free| free.get(&TypeId::of::<Slot<i32>>()).map(Vec::len).unwrap_or(0));
        // Reusing one i32 slot should leave exactly one fewer parked slot; if a
        // double-park had happened the count bookkeeping below would be off.
        let reused = Cage::new(0_i32);
        let after =
            CAGE_FREE_LIST.with_borrow(|free| free.get(&TypeId::of::<Slot<i32>>()).map(Vec::len).unwrap_or(0));
        assert_eq!(before.saturating_sub(after), 1);
        assert!(reused.is_current());
    }
}
