#[cfg(feature = "single-app")]
use std::cell::Cell;
#[cfg(not(feature = "single-app"))]
use std::cell::RefCell;
use std::ops::Deref;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[cfg(not(feature = "single-app"))]
use indexmap::IndexMap;
use indexmap::IndexSet;

use super::{PENDING_ITEMS, REVISING_ITEMS};
#[cfg(not(feature = "single-app"))]
use crate::HolderId;
use crate::{ROOT_VIEWS, ViewId};

thread_local! {
    #[cfg(feature = "single-app")]
    pub(crate) static RUNNING: Cell<bool> = Cell::new(false);
    #[cfg(not(feature = "single-app"))]
    pub(crate) static RUNNING: RefCell<IndexMap<HolderId, bool>> = RefCell::new(IndexMap::new());

    #[cfg(feature = "single-app")]
    pub(crate) static UNTRACKING: Cell<bool> = Cell::new(false);
    #[cfg(not(feature = "single-app"))]
    pub(crate) static UNTRACKING: RefCell<IndexMap<HolderId, bool>> = RefCell::new(IndexMap::new());

    #[cfg(feature = "single-app")]
    pub(crate) static BATCHING: Cell<bool> = Cell::new(false);
    #[cfg(not(feature = "single-app"))]
    pub(crate) static BATCHING: RefCell<IndexMap<HolderId, bool>> = RefCell::new(IndexMap::new());
}

#[cfg(feature = "single-app")]
pub fn is_running() -> bool {
    RUNNING.with(|running| running.get())
}
#[cfg(not(feature = "single-app"))]
pub fn is_running(holder_id: HolderId) -> bool {
    RUNNING.with_borrow(|running| running.get(&holder_id).copied().unwrap_or(false))
}

#[cfg(feature = "single-app")]
pub fn is_untracking() -> bool {
    UNTRACKING.with(|untracking| untracking.get())
}
#[cfg(not(feature = "single-app"))]
pub fn is_untracking(holder_id: HolderId) -> bool {
    UNTRACKING.with_borrow(|untracking| untracking.get(&holder_id).copied().unwrap_or(false))
}

#[cfg(feature = "single-app")]
pub fn is_batching() -> bool {
    BATCHING.with(|batching| batching.get())
}
#[cfg(not(feature = "single-app"))]
pub fn is_batching(holder_id: HolderId) -> bool {
    BATCHING.with_borrow(|batching| batching.get(&holder_id).copied().unwrap_or(false))
}

#[cfg(feature = "single-app")]
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
#[cfg(not(feature = "single-app"))]
pub fn batch<O, R>(holder_id: HolderId, opt: O) -> R
where
    O: FnOnce() -> R,
{
    BATCHING.with(|batching| {
        if !batching.borrow().get(&holder_id).copied().unwrap_or(false) {
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

#[cfg(feature = "single-app")]
pub fn schedule() {
    if !is_running() && !is_batching() {
        run();
    }
}
#[cfg(not(feature = "single-app"))]
pub fn schedule(holder_id: HolderId) {
    if !is_running(holder_id) && !is_batching(holder_id) {
        run(holder_id);
    }
}

fn run(#[cfg(not(feature = "single-app"))] holder_id: HolderId) {
    // Patches may create widgets (e.g. keyed-list rows); on command-stream
    // backends their nodes must allocate from the owning holder's queue.
    #[cfg(all(not(feature = "single-app"), not(all(target_arch = "wasm32", feature = "web-csr"))))]
    let _queue_guard = crate::renderer::command::make_holder_queue_current(holder_id);

    cfg_if! {
        if #[cfg(feature = "single-app")] {
            RUNNING.with(|running| running.set(true));
        } else {
            RUNNING.with_borrow_mut(|running| running.insert(holder_id, true));
        }
    }
    let mut loop_counts = 0;
    loop {
        let mut revising_view_ids = IndexSet::<ViewId>::default();
        REVISING_ITEMS.with(|revising_items| {
            cfg_if! {
                if #[cfg(feature = "single-app")] {
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
                #[cfg(not(feature = "single-app"))]
                let root_views = root_views.entry(holder_id).or_default();

                for view_id in revising_view_ids {
                    if let Some(view) = root_views.get_mut(&view_id) {
                        let boundary_id = view.scope.error_boundary.clone();
                        let patch_result = catch_unwind(AssertUnwindSafe(|| {
                            view.widget.patch(&mut view.scope);
                        }));
                        if let Err(payload) = patch_result {
                            let Some(boundary_id) = boundary_id else {
                                resume_unwind(payload);
                            };
                            let Some(boundary) = root_views.get_mut(&boundary_id) else {
                                resume_unwind(payload);
                            };
                            let error = crate::BoundaryError::from_panic(payload, Some(view_id));
                            if !boundary.widget.capture_error(&mut boundary.scope, error) {
                                panic!("error boundary `{}` rejected captured panic", boundary_id);
                            }
                        }
                    } else {
                        // Expected during large keyed-list replace/clear: a
                        // revised cage whose view was already detached. The
                        // block below prunes it; only surface it in debug.
                        crate::debug_warn!("view not found: {:?}", view_id);
                        not_found_view_ids.push(view_id);
                    }
                }
            });
        }
        if !not_found_view_ids.is_empty() {
            REVISING_ITEMS.with_borrow_mut(|revising_items| {
                #[cfg(not(feature = "single-app"))]
                let revising_items = revising_items.entry(holder_id).or_default();

                for view_id in not_found_view_ids {
                    for (_, item) in revising_items.iter() {
                        item.view_ids().borrow_mut().shift_remove(&view_id);
                    }
                }
            });
        }
        loop_counts += 1;

        cfg_if! {
            if #[cfg(feature = "single-app")] {
                let pending_items = PENDING_ITEMS.with(|pending_items| pending_items.take());
            } else {
                let pending_items = PENDING_ITEMS.with_borrow_mut(|pending_items| pending_items.shift_remove(&holder_id).unwrap_or_default());
            }
        }
        if !pending_items.is_empty() {
            if loop_counts > 8 {
                crate::warn!("schedule loop_counts > 8 and force break. pending_items: {:?}", pending_items);
                break;
            } else {
                REVISING_ITEMS.with(|revising_items| {
                    cfg_if! {
                        if #[cfg(feature = "single-app")] {
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
                    if #[cfg(feature = "single-app")] {
                        revising_items.borrow_mut().clear();
                    }  else {
                        revising_items.borrow_mut().shift_remove(&holder_id);
                    }
                }
            });
            break;
        }
    }

    cfg_if! {
        if #[cfg(feature = "single-app")] {
            RUNNING.with(|running| running.set(false));
        } else {
            RUNNING.with_borrow_mut(|running| running.insert(holder_id, false));
        }
    }
}
