//! One empty spare per stream. Payloads are dropped before borrowing the pool.
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};

// Large bursts must not permanently reserve their peak in every idle stream.
const MAX_BYTES: usize = 1024 * 1024;

pub(crate) struct Pool<T>(RefCell<Vec<T>>);
impl<T> Default for Pool<T> {
    fn default() -> Self {
        Self(RefCell::new(Vec::new()))
    }
}
impl<T> Pool<T> {
    pub(crate) fn take(&self) -> Batch<'_, T> {
        Batch {
            pool: self,
            items: std::mem::take(&mut *self.0.borrow_mut()),
        }
    }
    pub(crate) fn capacity(&self) -> usize {
        self.0.borrow().capacity()
    }
}

pub(crate) struct Batch<'a, T> {
    pool: &'a Pool<T>,
    items: Vec<T>,
}
impl<T> Deref for Batch<'_, T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target {
        &self.items
    }
}
impl<T> DerefMut for Batch<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.items
    }
}
impl<T> Drop for Batch<'_, T> {
    fn drop(&mut self) {
        // A destructor can reenter and use the pool. No pool/Present borrow is
        // held, and only empty capacity may be returned after it finishes.
        self.items.clear();
        if self.items.capacity().saturating_mul(size_of::<T>()) > MAX_BYTES {
            return;
        }
        if let Ok(mut spare) = self.pool.0.try_borrow_mut()
            && self.items.capacity() > spare.capacity()
        {
            std::mem::swap(&mut *spare, &mut self.items);
        }
        // Discarded capacity drops after the RefMut, including during unwind.
    }
}

/// Queue and idle-spare capacities, sampled between host turns.
/// Counts slots/bytes, excludes in-flight batches, and does not measure RSS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BufferStats {
    pub commands: usize,
    pub actions: usize,
    pub listeners: usize,
    pub inputs: usize,
    pub bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn unwind_releases_payloads_but_returns_bounded_empty_storage() {
        struct Payload(Rc<Cell<usize>>);
        impl Drop for Payload {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let pool = Pool::default();
        let drops = Rc::new(Cell::new(0));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut batch = pool.take();
                batch.reserve(16);
                batch.push(Payload(drops.clone()));
                panic!("rollback");
            }))
            .is_err()
        );
        assert_eq!(drops.get(), 1);
        let batch = pool.take();
        assert!(batch.is_empty());
        assert!(batch.capacity() >= 16);
    }

    #[test]
    fn payload_drop_may_reenter_and_nested_returns_keep_one_empty_spare() {
        struct Payload(Rc<Pool<Payload>>, Rc<Cell<bool>>);
        impl Drop for Payload {
            fn drop(&mut self) {
                let mut nested = self.0.take();
                assert!(nested.is_empty());
                nested.reserve(8);
                self.1.set(true);
            }
        }
        let pool = Rc::new(Pool::default());
        let entered = Rc::new(Cell::new(false));
        let mut batch = pool.take();
        batch.reserve(32);
        batch.push(Payload(pool.clone(), entered.clone()));
        drop(batch);
        assert!(entered.get());
        assert!(pool.capacity() >= 32);
        assert!(pool.take().is_empty());
        assert_eq!(Rc::strong_count(&pool), 1);
    }

    #[test]
    fn oversized_bursts_are_not_kept_and_failed_return_does_not_panic() {
        let pool = Pool::<u8>::default();
        let mut huge = pool.take();
        huge.reserve(MAX_BYTES + 1);
        drop(huge);
        assert_eq!(pool.capacity(), 0);
        let mut batch = pool.take();
        batch.reserve(8);
        let borrow = pool.0.borrow_mut();
        drop(batch);
        drop(borrow);
        assert_eq!(pool.capacity(), 0);
    }
}
