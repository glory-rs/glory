//! Generational-box arena backing the `Copy` reactive primitives.
//!
//! `Cage<T>` (and the other reactive types layered on top) used to hold
//! their data through `Rc<RefCell<T>>`. That works correctly but it
//! forces every consumer to call `.clone()` whenever they want to pass a
//! cage to a new closure or widget, and it can't be `Copy` because each
//! `Rc::clone` must bump a refcount.
//!
//! This module switches the implementation strategy to a per-thread
//! arena: every `Cage::new` allocates a slot in [`UnsyncStorage`] and
//! tags the handle with a generation counter. The public `Cage<T>`
//! becomes a small `Copy` value (`{ slot, generation, _phantom }`); the
//! actual `T`, version counter, and subscriber-id set live in the
//! arena, accessed via runtime borrow checks.
//!
//! ## Lifetime model
//!
//! For now the arena is **append-only**: slots are allocated on
//! `Cage::new` and never freed during the program's lifetime. This is
//! intentional for the initial migration — Glory apps today already
//! keep most reactive state alive for the full mount lifetime, so the
//! practical memory cost is small. A future enhancement will tie slots
//! to an explicit `Owner` (similar to dioxus's `generational-box`) so
//! per-component state can be reclaimed when the component drops.
//!
//! ## Sync
//!
//! `UnsyncStorage` is the only backend right now and lives in a
//! `thread_local!`. A `SyncStorage` variant (using a global
//! `RwLock<Slab<_>>`) is on the roadmap for SSR workflows that want to
//! pass `Cage`s across async task boundaries; the [`Storage`] trait is
//! the seam those backends will sit behind.
#![allow(dead_code)]

use std::any::Any;
use std::cell::RefCell;
use std::marker::PhantomData;
#[cfg(feature = "sync-storage")]
use std::sync::{Arc, RwLock};

use slab::Slab;

thread_local! {
    pub(crate) static UNSYNC_ARENA: RefCell<Arena> = RefCell::new(Arena::new());
}

/// One slot in the arena: a generation counter plus the type-erased
/// payload. The generation is bumped when (in the future) the slot is
/// recycled; today nothing recycles, so generation stays at 0.
pub(crate) struct Slot {
    pub generation: u64,
    pub data: Box<dyn Any>,
}

#[derive(Default)]
pub(crate) struct Arena {
    slots: Slab<Slot>,
}

impl Arena {
    pub fn new() -> Self {
        Self { slots: Slab::new() }
    }
    pub(crate) fn alloc<T: 'static>(&mut self, data: T) -> (usize, u64) {
        let generation = 0_u64;
        let key = self.slots.insert(Slot {
            generation,
            data: Box::new(data),
        });
        (key, generation)
    }
    /// Borrow a slot's payload as `&T` after verifying the generation
    /// still matches. Returns `None` if the slot was recycled (this
    /// can't happen yet, but the check is wired up for forward
    /// compatibility).
    pub(crate) fn try_borrow<T: 'static>(&self, slot: usize, generation: u64) -> Option<&T> {
        let s = self.slots.get(slot)?;
        if s.generation != generation {
            return None;
        }
        s.data.downcast_ref::<T>()
    }
    pub(crate) fn try_borrow_mut<T: 'static>(&mut self, slot: usize, generation: u64) -> Option<&mut T> {
        let s = self.slots.get_mut(slot)?;
        if s.generation != generation {
            return None;
        }
        s.data.downcast_mut::<T>()
    }
}

/// Convenience: borrow `T` from the unsync arena, panicking with a
/// helpful message on generation mismatch (which would indicate use of
/// a freed handle once explicit deallocation lands).
pub(crate) fn with_slot<T: 'static, R>(slot: usize, generation: u64, f: impl FnOnce(&T) -> R) -> R {
    UNSYNC_ARENA.with(|arena| {
        let arena = arena.borrow();
        let val = arena
            .try_borrow::<T>(slot, generation)
            .expect("storage::with_slot: handle was freed or has wrong type");
        f(val)
    })
}

pub(crate) fn with_slot_mut<T: 'static, R>(slot: usize, generation: u64, f: impl FnOnce(&mut T) -> R) -> R {
    UNSYNC_ARENA.with(|arena| {
        let mut arena = arena.borrow_mut();
        let val = arena
            .try_borrow_mut::<T>(slot, generation)
            .expect("storage::with_slot_mut: handle was freed or has wrong type");
        f(val)
    })
}

/// Type-erased `Copy` handle into the unsync arena.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Handle<T: 'static> {
    pub slot: usize,
    pub generation: u64,
    pub _phantom: PhantomData<T>,
}

impl<T: 'static> Handle<T> {
    pub fn alloc(data: T) -> Self {
        let (slot, generation) = UNSYNC_ARENA.with(|arena| arena.borrow_mut().alloc(data));
        Self {
            slot,
            generation,
            _phantom: PhantomData,
        }
    }
    pub fn with<R>(self, f: impl FnOnce(&T) -> R) -> R {
        with_slot::<T, R>(self.slot, self.generation, f)
    }
    pub fn with_mut<R>(self, f: impl FnOnce(&mut T) -> R) -> R {
        with_slot_mut::<T, R>(self.slot, self.generation, f)
    }
}

/// Equality of `Handle`s is identity-based (same slot + same generation).
impl<T: 'static> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot && self.generation == other.generation
    }
}
impl<T: 'static> Eq for Handle<T> {}

impl<T: 'static> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.slot.hash(state);
        self.generation.hash(state);
    }
}

#[cfg(feature = "sync-storage")]
/// Thread-safe storage backend for SSR and native runtimes.
///
/// This backend intentionally mirrors [`Arena`]'s generation checks,
/// but stores slots behind `Arc<RwLock<_>>` so handles can be moved
/// across threads. The current `Cage<T>` migration still uses the
/// unsync fast path by default; this is the feature-gated foundation
/// for switching the public reactive primitives to a sync backend.
#[derive(Clone, Default)]
pub struct SyncStorage {
    arena: Arc<RwLock<SyncArena>>,
}

#[cfg(feature = "sync-storage")]
#[derive(Default)]
struct SyncArena {
    slots: Slab<SyncSlot>,
}

#[cfg(feature = "sync-storage")]
struct SyncSlot {
    generation: u64,
    data: Arc<dyn Any + Send + Sync>,
}

#[cfg(feature = "sync-storage")]
impl SyncStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc<T>(&self, data: T) -> SyncHandle<T>
    where
        T: Send + Sync + 'static,
    {
        let mut arena = self.arena.write().expect("SyncStorage::alloc: lock poisoned");
        let generation = 0_u64;
        let slot = arena.slots.insert(SyncSlot {
            generation,
            data: Arc::new(RwLock::new(data)),
        });
        SyncHandle {
            storage: self.clone(),
            slot,
            generation,
            _phantom: PhantomData,
        }
    }

    fn read_cell<T>(&self, slot: usize, generation: u64) -> Option<Arc<RwLock<T>>>
    where
        T: Send + Sync + 'static,
    {
        let arena = self.arena.read().ok()?;
        let slot = arena.slots.get(slot)?;
        if slot.generation != generation {
            return None;
        }
        slot.data.clone().downcast::<RwLock<T>>().ok()
    }
}

#[cfg(feature = "sync-storage")]
#[derive(Clone)]
pub struct SyncHandle<T>
where
    T: Send + Sync + 'static,
{
    storage: SyncStorage,
    slot: usize,
    generation: u64,
    _phantom: PhantomData<T>,
}

#[cfg(feature = "sync-storage")]
impl<T> std::fmt::Debug for SyncHandle<T>
where
    T: Send + Sync + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncHandle")
            .field("slot", &self.slot)
            .field("generation", &self.generation)
            .finish()
    }
}

#[cfg(feature = "sync-storage")]
impl<T> SyncHandle<T>
where
    T: Send + Sync + 'static,
{
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        let cell = self.storage.read_cell::<T>(self.slot, self.generation)?;
        let guard = cell.read().ok()?;
        Some(f(&*guard))
    }

    pub fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        let cell = self.storage.read_cell::<T>(self.slot, self.generation)?;
        let mut guard = cell.write().ok()?;
        Some(f(&mut *guard))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_and_read() {
        let h = Handle::<i32>::alloc(42);
        h.with(|v| assert_eq!(*v, 42));
    }

    #[test]
    fn mutate_via_handle() {
        let h = Handle::<i32>::alloc(1);
        h.with_mut(|v| *v += 41);
        h.with(|v| assert_eq!(*v, 42));
    }

    #[test]
    fn handles_are_copy() {
        let h = Handle::<i32>::alloc(7);
        let h2 = h; // Copy, not move
        assert_eq!(h, h2);
        h.with(|v| assert_eq!(*v, 7));
        h2.with(|v| assert_eq!(*v, 7));
    }

    #[test]
    fn distinct_slots_are_unequal() {
        let a = Handle::<i32>::alloc(1);
        let b = Handle::<i32>::alloc(1);
        assert_ne!(a, b);
    }

    #[cfg(feature = "sync-storage")]
    #[test]
    fn sync_storage_handles_cross_threads() {
        let storage = SyncStorage::new();
        let handle = storage.alloc(1_i32);
        let handle_for_thread = handle.clone();

        std::thread::spawn(move || {
            handle_for_thread.with_mut(|value| *value += 41).unwrap();
        })
        .join()
        .unwrap();

        assert_eq!(handle.with(|value| *value).unwrap(), 42);
    }
}
