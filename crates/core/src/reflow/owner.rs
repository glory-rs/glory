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
}
