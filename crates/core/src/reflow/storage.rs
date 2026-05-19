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

use std::any::Any;
use std::cell::{Ref, RefCell, RefMut};
use std::marker::PhantomData;

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
}
