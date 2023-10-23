mod cage;
pub use cage::{Cage, ReadCage};
mod bond;
pub use bond::Bond;
pub mod scheduler;
pub use scheduler::{batch, schedule};

use std::cell::{ Ref, RefCell};
use std::fmt::{self, Display};
use std::hash::Hash;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use indexmap::{IndexMap, IndexSet};

#[cfg(not(feature = "__single_holder"))]
use crate::HolderId;
use crate::ViewId;

thread_local! {
    #[cfg(feature = "__single_holder")]
    pub(crate) static REVISING_ITEMS: RefCell<IndexMap<RevisableId, Box<dyn Signal>>> = RefCell::default();
    #[cfg(not(feature = "__single_holder"))]
    pub(crate) static REVISING_ITEMS: RefCell<IndexMap<HolderId, IndexMap<RevisableId, Box<dyn Signal>>>> = RefCell::default();
    
    #[cfg(feature = "__single_holder")]
    pub(crate) static PENDING_ITEMS: RefCell<IndexMap<RevisableId, Box<dyn Signal>>> = RefCell::default();
    #[cfg(not(feature = "__single_holder"))]
    pub(crate) static PENDING_ITEMS: RefCell<IndexMap<HolderId, IndexMap<RevisableId, Box<dyn Signal>>>> = RefCell::default();

    pub(crate) static TRACKING_STACK: RefCell<TrackingStack> = RefCell::new(TrackingStack::new());
}

#[derive(Default)]
pub(crate) struct TrackingStack {
    pub layers: Vec<IndexMap<RevisableId, Box<dyn Signal>>>,
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
    pub(crate) fn pop_layer(&mut self) -> Option<IndexMap<RevisableId, Box<dyn Signal>>> {
        self.layers.pop()
    }
    pub(crate) fn track(&mut self, item: impl Into<Box<dyn Signal>>) {
        let item = item.into();
        for layer in &mut self.layers {
            layer.insert(item.id(), item.clone_boxed());
        }
    }
}

#[cfg(feature = "__single_holder")]
pub fn untrack<O, R>(opt: O) -> R
where
    O: FnOnce() -> R,
{
    scheduler::BATCHING.with(|batching| {
        if !batching.get() {
            batching.set(true);
            let out = opt();
            batching.set(false);
            out
        } else {
            opt()
        }
    })
}

#[cfg(not(feature = "__single_holder"))]

pub fn untrack<O, R>(holder_id: HolderId, opt: O) -> R
where
    O: FnOnce() -> R,
{
    scheduler::BATCHING.with(|batching| {
        if !batching.borrow().get(&holder_id).map(|v| *v).unwrap_or(false) {
            batching.borrow_mut().insert(holder_id, true);
            let out = opt();
            batching.borrow_mut().insert(holder_id, false);
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
    #[cfg(not(feature = "__single_holder"))]
    fn holder_id(&self) -> Option<HolderId>;
    fn version(&self) -> usize;
    fn bind_view(&self, view_id: &ViewId);
    fn is_revising(&self) -> bool {
        REVISING_ITEMS.with(|revising_items| {
            cfg_if! {
                if #[cfg(feature = "__single_holder")] {
                    revising_items.borrow().contains_key(&self.id())
                } else {
                    if let Some(holder_id) = self.holder_id() {
                        revising_items.borrow_mut().entry(holder_id).or_default().contains_key(&self.id())
                    } else {
                        tracing::debug!("Revisable::is_revising: holder_id is None");
                        false
                    }
                }
            }
        })
    }
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

pub trait Signal: Revisable + fmt::Debug {
    fn view_ids(&self) -> Rc<RefCell<IndexSet<ViewId>>>;
    fn signal(&self);
    fn clone_boxed(&self) -> Box<dyn Signal>;
}

pub trait Record<S>: Revisable
where
    S: fmt::Debug,
{
    fn get(&self) -> Ref<'_, S>;
    fn get_untracked(&self) -> Ref<'_, S>;
}
