use std::cell::{BorrowError, BorrowMutError, Cell, Ref, RefCell, RefMut};
use std::fmt;
use std::rc::Rc;

use indexmap::IndexSet;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Bond, PENDING_ITEMS, REVISING_ITEMS, Revisable, RevisableId, TRACKING_STACK};
use crate::ViewId;
use crate::reflow::{self, Lotus, scheduler};

pub struct Cage<T>
where
    T: fmt::Debug + 'static,
{
    inner: &'static CageInner<T>,
    generation: u64,
}

struct CageInner<T>
where
    T: fmt::Debug + 'static,
{
    id: RevisableId,
    generation: Cell<u64>,
    alive: Cell<bool>,
    version: Cell<usize>,
    source: RefCell<T>,
    view_ids: Rc<RefCell<IndexSet<ViewId>>>,
}

impl<T> fmt::Debug for Cage<T>
where
    T: fmt::Debug + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cage")
            .field("id", &self.inner.id)
            .field("generation", &self.generation)
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
        self.inner.id
    }
    #[cfg(not(feature = "single-app"))]
    fn holder_id(&self) -> Option<crate::HolderId> {
        self.view_ids().borrow().first().map(|view_id| view_id.holder_id())
    }
    fn version(&self) -> usize {
        self.inner.version.get()
    }
    fn view_ids(&self) -> Rc<RefCell<IndexSet<ViewId>>> {
        self.inner.view_ids.clone()
    }
    fn bind_view(&self, view_id: &ViewId) {
        self.inner.view_ids.borrow_mut().insert(view_id.clone());
    }
    fn unbind_view(&self, view_id: &ViewId) {
        self.inner.view_ids.borrow_mut().shift_remove(view_id);
    }
    fn unlace_view(&self, view_id: &ViewId, loose: usize) {
        if loose > 0 {
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
        T::serialize(&*self.inner.source.borrow(), serializer)
    }
}

impl<T> Cage<T>
where
    T: fmt::Debug + 'static,
{
    pub(crate) fn invalidate(&self) {
        self.inner.alive.set(false);
        self.inner.generation.set(self.inner.generation.get().wrapping_add(1));
    }

    fn ensure_alive(&self) -> Result<(), CageAccessError> {
        if self.inner.alive.get() && self.inner.generation.get() == self.generation {
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
        self.inner.source.try_borrow().map_err(CageAccessError::Borrow)
    }

    pub fn get_untracked(&self) -> Ref<'_, T> {
        self.try_get_untracked().expect("Cage::get_untracked: source is already mutably borrowed")
    }

    pub fn try_get_untracked(&self) -> Result<Ref<'_, T>, CageAccessError> {
        self.ensure_alive()?;
        self.inner.source.try_borrow().map_err(CageAccessError::Borrow)
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
        let result = (opt)(self.inner.source.try_borrow_mut().map_err(CageMutateError::Borrow)?);
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
        let result = (opt)(self.inner.source.try_borrow_mut().map_err(CageMutateError::Borrow)?);
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
        self.inner.source.borrow()
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
        let inner = Box::leak(Box::new(CageInner {
            id: RevisableId::next(),
            generation: Cell::new(0),
            alive: Cell::new(true),
            version: Cell::new(1),
            source: RefCell::new(source),
            view_ids: Default::default(),
        }));
        Cage { inner, generation: 0 }
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
        std::ptr::eq(self.inner, other.inner)
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
}
