use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use super::Cage;

type Invalidator = Box<dyn Fn() + 'static>;

#[derive(Clone, Default)]
pub struct Owner {
    inner: Rc<OwnerInner>,
}

#[derive(Default)]
struct OwnerInner {
    invalidators: RefCell<Vec<Invalidator>>,
}

impl Owner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn own_cage<T>(&self, cage: Cage<T>) -> Cage<T>
    where
        T: fmt::Debug + 'static,
    {
        self.inner.invalidators.borrow_mut().push(Box::new(move || cage.invalidate()));
        cage
    }

    pub fn cage<T>(&self, value: T) -> Cage<T>
    where
        T: fmt::Debug + 'static,
    {
        self.own_cage(Cage::new(value))
    }

    /// Number of cages this owner will invalidate when it drops. Useful for
    /// debugging reclamation (e.g. asserting a scope released what it owned).
    pub fn owned_count(&self) -> usize {
        self.inner.invalidators.borrow().len()
    }
}

impl Drop for OwnerInner {
    fn drop(&mut self) {
        for invalidate in self.invalidators.get_mut().drain(..) {
            invalidate();
        }
    }
}

impl fmt::Debug for Owner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Owner")
            .field("owned_signals", &self.inner.invalidators.borrow().len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_drop_invalidates_owned_cage() {
        let cage = {
            let owner = Owner::new();
            owner.cage(1_i32)
        };

        assert!(cage.try_get_untracked().is_err());
    }

    #[test]
    fn owner_invalidates_all_owned_cages_and_tracks_count() {
        let owner = Owner::new();
        let a = owner.cage(1_i32);
        let b = owner.cage(2_i32);
        let c = owner.own_cage(Cage::new(3_i32));
        assert_eq!(owner.owned_count(), 3);

        // Live before drop.
        assert!(a.try_get_untracked().is_ok());
        drop(owner);

        // Every owned cage is reclaimed when the owner drops.
        assert!(a.try_get_untracked().is_err());
        assert!(b.try_get_untracked().is_err());
        assert!(c.try_get_untracked().is_err());
    }
}
