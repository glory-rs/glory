use std::cell::{Cell, Ref, RefCell, RefMut};
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

use educe::Educe;
use indexmap::IndexSet;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Bond, Record, Revisable, RevisableId, Signal, PENDING_ITEMS, REVISING_ITEMS, TRACKING_STACK};
use crate::reflow::{self, scheduler};
use crate::ViewId;

#[derive(Educe)]
#[educe(Debug)]
pub struct Cage<T>
where
    T: fmt::Debug + 'static,
{
    id: RevisableId,
    version: Rc<Cell<usize>>,
    source: Rc<RefCell<T>>,
    view_ids: Rc<RefCell<IndexSet<ViewId>>>,
}
impl<T> Revisable for Cage<T>
where
    T: fmt::Debug,
{
    fn id(&self) -> RevisableId {
        self.id
    }
    #[cfg(not(feature = "__single_holder"))]
    fn holder_id(&self) -> Option<crate::HolderId> {
        self.view_ids().borrow().first().map(|view_id| view_id.holder_id())
    }
    fn version(&self) -> usize {
        self.version.get()
    }
    fn bind_view(&self, view_id: &ViewId) {
        (*self.view_ids).borrow_mut().insert(view_id.clone());
    }
}
impl<T> Signal for Cage<T>
where
    T: fmt::Debug + 'static,
{
    fn view_ids(&self) -> Rc<RefCell<IndexSet<ViewId>>> {
        self.view_ids.clone()
    }
    fn signal(&self) {
        #[cfg(not(feature = "__single_holder"))]
        let Some(holder_id) = self.holder_id() else {
            tracing::debug!("Cage::signal: holder_id is None");
            return;
        };
        if scheduler::is_untracking(
            #[cfg(not(feature = "__single_holder"))]
            holder_id,
        ) {
            return;
        }
        let is_running = scheduler::is_running(
            #[cfg(not(feature = "__single_holder"))]
            holder_id,
        );

        if is_running {
            PENDING_ITEMS.with_borrow_mut(|items| {
                #[cfg(not(feature = "__single_holder"))]
                let items = items.entry(holder_id).or_default();
                if !items.contains_key(&self.id()) {
                    items.insert(self.id(), self.clone_boxed());
                }
            });
        } else {
            let need_schedule = REVISING_ITEMS.with_borrow_mut(|items| {
                #[cfg(not(feature = "__single_holder"))]
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
                    #[cfg(not(feature = "__single_holder"))]
                    holder_id,
                );
            }
        }
    }

    fn clone_boxed(&self) -> Box<dyn Signal> {
        Box::new(self.clone())
    }
}
impl<T> Record<T> for Cage<T>
where
    T: fmt::Debug,
{
    fn get(&self) -> Ref<'_, T> {
        let this = self;
        TRACKING_STACK.with(|tracking_items| {
            let mut tracking_items = tracking_items.borrow_mut();
            if !tracking_items.is_idle() {
                tracking_items.track(this.clone_boxed());
            }
        });
        self.source.borrow()
    }
    fn get_untracked(&self) -> Ref<'_, T> {
        self.source.borrow()
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
        T::serialize(self.source.deref().borrow().deref(), serializer)
    }
}

impl<T> Cage<T>
where
    T: fmt::Debug,
{
    pub fn revise<F, R>(&self, opt: F) -> R
    where
        F: FnOnce(RefMut<'_, T>) -> R,
    {
        let result = (opt)(self.source.deref().borrow_mut());
        self.version.set(self.version.get() + 1);
        self.signal();
        result
    }
    pub fn revise_silent<F, R>(&self, opt: F) -> R
    where
        F: FnOnce(RefMut<'_, T>) -> R,
    {
        let result = (opt)(self.source.deref().borrow_mut());
        self.version.set(self.version.get() + 1);
        result
    }
    // pub fn source<'a>(&'a self) -> std::cell::Ref<'a, S> {
    //     self.source.borrow()
    // }
    pub fn borrow(&self) -> Ref<'_, T> {
        self.source.borrow()
    }
    pub fn map<M, G>(&self, mapper: M) -> Bond<impl Fn() -> G + Clone + 'static, G>
    where
        M: Fn(Ref<'_, T>) -> G + Clone + 'static,
        G: fmt::Debug + 'static,
    {
        let this = self.clone();
        Bond::new(move || mapper(this.get()))
    }

    pub fn read(&self) -> ReadCage<T> {
        ReadCage::new(self.clone())
    }
}

impl<T> Cage<T>
where
    T: fmt::Debug,
{
    pub fn new(source: T) -> Self {
        Cage {
            id: RevisableId::next(),
            version: Rc::new(Cell::new(1)),
            source: Rc::new(RefCell::new(source)),
            view_ids: Default::default(),
        }
    }
}

impl<T> Default for Cage<T>
where
    T: fmt::Debug + Default,
{
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T> Clone for Cage<T>
where
    T: fmt::Debug,
{
    fn clone(&self) -> Self {
        Cage {
            id: self.id,
            version: self.version.clone(),
            source: self.source.clone(),
            view_ids: self.view_ids.clone(),
        }
    }
}

impl<T> From<T> for Cage<T>
where
    T: fmt::Debug,
{
    fn from(source: T) -> Self {
        Self::new(source)
    }
}
impl<T> Eq for Cage<T> where T: fmt::Debug + Eq {}
impl<T> PartialEq<Cage<T>> for Cage<T>
where
    T: fmt::Debug + Eq,
{
    #[inline]
    fn eq(&self, other: &Cage<T>) -> bool {
        self.source == other.source
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

#[derive(Debug)]
pub struct ReadCage<T>(Cage<T>)
where
    T: fmt::Debug + 'static;

impl<T> Revisable for ReadCage<T>
where
    T: fmt::Debug + 'static,
{
    fn id(&self) -> RevisableId {
        self.0.id
    }
    #[cfg(not(feature = "__single_holder"))]
    fn holder_id(&self) -> Option<crate::HolderId> {
        self.0.holder_id()
    }
    fn version(&self) -> usize {
        self.0.version.get()
    }
    fn bind_view(&self, view_id: &ViewId) {
        (*self.0.view_ids).borrow_mut().insert(view_id.clone());
    }
}
impl<T> Record<T> for ReadCage<T>
where
    T: fmt::Debug + 'static,
{
    fn get(&self) -> Ref<'_, T> {
        self.0.get()
    }
    fn get_untracked(&self) -> Ref<'_, T> {
        self.0.get_untracked()
    }
}

impl<'de, T> Deserialize<'de> for ReadCage<T>
where
    T: Deserialize<'de> + fmt::Debug + 'static,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(|v| ReadCage::new(Cage::new(v)))
    }
}

impl<T> Serialize for ReadCage<T>
where
    T: Serialize + fmt::Debug + 'static,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        T::serialize(self.0.source.deref().borrow().deref(), serializer)
    }
}

impl<T> ReadCage<T>
where
    T: fmt::Debug,
{
    pub fn borrow(&self) -> Ref<'_, T> {
        self.0.borrow()
    }
    pub fn map<M, G>(&self, mapper: M) -> Bond<impl Fn() -> G + Clone + 'static, G>
    where
        M: Fn(Ref<'_, T>) -> G + Clone + 'static,
        G: fmt::Debug + 'static,
    {
        self.0.map(mapper)
    }
}

impl<T> ReadCage<T>
where
    T: fmt::Debug,
{
    pub fn new(cage: Cage<T>) -> Self {
        ReadCage(cage)
    }
}
impl<T> Default for ReadCage<T>
where
    T: fmt::Debug + Default,
{
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T> Clone for ReadCage<T>
where
    T: fmt::Debug,
{
    fn clone(&self) -> Self {
        Self::new(self.0.clone())
    }
}
