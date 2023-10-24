#[cfg(feature = "__single_holder")]
use std::cell::Cell;
#[cfg(not(feature = "__single_holder"))]
use std::cell::RefCell;
use std::ops::Deref;

#[cfg(not(feature = "__single_holder"))]
use indexmap::IndexMap;
use indexmap::IndexSet;

use super::{PENDING_ITEMS, REVISING_ITEMS};
#[cfg(not(feature = "__single_holder"))]
use crate::HolderId;
use crate::{ViewId, ROOT_VIEWS};

thread_local! {
    #[cfg(feature = "__single_holder")]
    pub(crate) static RUNING: Cell<bool> = Cell::new(false);
    #[cfg(not(feature = "__single_holder"))]
    pub(crate) static RUNING: RefCell<IndexMap<HolderId, bool>> = RefCell::new(IndexMap::new());

    #[cfg(feature = "__single_holder")]
    pub(crate) static UNTRACKING: Cell<bool> = Cell::new(false);
    #[cfg(not(feature = "__single_holder"))]
    pub(crate) static UNTRACKING: RefCell<IndexMap<HolderId, bool>> = RefCell::new(IndexMap::new());

    #[cfg(feature = "__single_holder")]
    pub(crate) static BATCHING: Cell<bool> = Cell::new(false);
    #[cfg(not(feature = "__single_holder"))]
    pub(crate) static BATCHING: RefCell<IndexMap<HolderId, bool>> = RefCell::new(IndexMap::new());
}

#[cfg(feature = "__single_holder")]
pub fn is_running() -> bool {
    RUNING.with(|running| running.get())
}
#[cfg(not(feature = "__single_holder"))]
pub fn is_running(holder_id: HolderId) -> bool {
    RUNING.with_borrow(|running| running.get(&holder_id).map(|v| *v).unwrap_or(false))
}

#[cfg(feature = "__single_holder")]
pub fn is_untracking() -> bool {
    UNTRACKING.with(|untracking| untracking.get())
}
#[cfg(not(feature = "__single_holder"))]
pub fn is_untracking(holder_id: HolderId) -> bool {
    UNTRACKING.with_borrow(|untracking| untracking.get(&holder_id).map(|v| *v).unwrap_or(false))
}


#[cfg(feature = "__single_holder")]
pub fn is_batching() -> bool {
    BATCHING.with(|batching| batching.get())
}
#[cfg(not(feature = "__single_holder"))]
pub fn is_batching(holder_id: HolderId) -> bool {
    BATCHING.with_borrow(|batching| batching.get(&holder_id).map(|v| *v).unwrap_or(false))
}

#[cfg(feature = "__single_holder")]
pub fn batch<O, R>(opt: O) -> R
where
    O: FnOnce() -> R,
{
    BATCHING.with(|batching| {
        if !batching.get() {
            batching.set(true);
            let out = opt();
            batching.set(false);
            schedule();
            out
        } else {
            opt()
        }
    })
}
#[cfg(not(feature = "__single_holder"))]
pub fn batch<O, R>(holder_id: HolderId, opt: O) -> R
where
    O: FnOnce() -> R,
{
    BATCHING.with(|batching| {
        if !batching.borrow().get(&holder_id).map(|v| *v).unwrap_or(false) {
            batching.borrow_mut().insert(holder_id, true);
            let out = opt();
            batching.borrow_mut().insert(holder_id, false);
            schedule(holder_id);
            out
        } else {
            opt()
        }
    })
}

#[cfg(feature = "__single_holder")]
pub fn schedule() {
    if !is_running() && !is_batching() {
        run();
    }
}
#[cfg(not(feature = "__single_holder"))]
pub fn schedule(holder_id: HolderId) {
    if !is_running(holder_id) && !is_batching(holder_id) {
        run(holder_id);
    }
}

fn run(#[cfg(not(feature = "__single_holder"))] holder_id: HolderId) {
    cfg_if! {
        if #[cfg(feature = "__single_holder")] {
            RUNING.with(|running| running.set(true));
        } else {
            RUNING.with_borrow_mut(|running| running.insert(holder_id, true));
        }
    }
    let mut loop_counts = 0;
    loop {
        let mut revising_view_ids = IndexSet::<ViewId>::default();
        REVISING_ITEMS.with(|revising_items| {
            cfg_if! {
                if #[cfg(feature = "__single_holder")] {
                    let revising_items = revising_items.borrow_mut();
                } else {
                    let mut revising_items = revising_items.borrow_mut();
                    let revising_items = revising_items.entry(holder_id).or_default();
                }
            }

            for (_, item) in revising_items.iter() {
                for view_id in item.view_ids().borrow().deref() {
                    revising_view_ids.insert(view_id.clone());
                }
            }
        });
        let mut not_found_view_ids = Vec::with_capacity(revising_view_ids.len());
        if !revising_view_ids.is_empty() {
            ROOT_VIEWS.with_borrow_mut(|root_views| {
                #[cfg(not(feature = "__single_holder"))]
                let root_views = root_views.entry(holder_id).or_default();

                for view_id in revising_view_ids {
                    if let Some(view) = root_views.get_mut(&view_id) {
                        view.widget.patch(&mut view.scope);
                    } else {
                        crate::info!("view not found: {:?}", view_id);
                        not_found_view_ids.push(view_id);
                    }
                }
            });
        }
        if !not_found_view_ids.is_empty() {
            REVISING_ITEMS.with_borrow_mut(|revising_items| {
                #[cfg(not(feature = "__single_holder"))]
                let revising_items = revising_items.entry(holder_id).or_default();

                for view_id in not_found_view_ids {
                    for (_, item) in revising_items.iter() {
                        item.view_ids().borrow_mut().remove(&view_id);
                    }
                }
            });
        }
        loop_counts += 1;

        cfg_if! {
            if #[cfg(feature = "__single_holder")] {
                let pending_items = PENDING_ITEMS.with(|pending_items| pending_items.take());
            } else {
                let pending_items = PENDING_ITEMS.with_borrow_mut(|pending_items| pending_items.remove(&holder_id).unwrap_or_default());
            }
        }
        if !pending_items.is_empty() {
            if loop_counts > 8 {
                crate::warn!("schedule loop_counts > 8 and force break. pending_items: {:?}", pending_items);
                break;
            } else {
                REVISING_ITEMS.with(|revising_items| {
                    cfg_if! {
                        if #[cfg(feature = "__single_holder")] {
                            revising_items.replace(pending_items);
                        } else {
                            revising_items.borrow_mut().insert(holder_id, pending_items);
                        }
                    }
                });
            }
        } else {
            REVISING_ITEMS.with(|revising_items| {
                cfg_if! {
                    if #[cfg(feature = "__single_holder")] {
                        revising_items.borrow_mut().clear();
                    }  else {
                        revising_items.borrow_mut().remove(&holder_id);
                    }
                }
            });
            break;
        }
    }

    cfg_if! {
        if #[cfg(feature = "__single_holder")] {
            RUNING.with(|running| running.set(false));
        } else {
            RUNING.with_borrow_mut(|running| running.insert(holder_id, false));
        }
    }
}
