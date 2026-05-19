use std::cell::{Cell, Ref, RefCell};
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

use educe::Educe;
use indexmap::{IndexMap, IndexSet};

use super::{Revisable, RevisableId, TRACKING_STACK};
use crate::ViewId;

#[derive(Educe)]
#[educe(Debug)]
pub struct Bond<T>
where
    T: fmt::Debug + 'static,
{
    id: RevisableId,
    /// Monotonic counter bumped each time the mapper re-runs (or, when an
    /// equality gate is set, each time the output value changes).
    /// Reported as this Bond's `Revisable::version()` so downstream
    /// observers detect changes without depending on a sum-of-dep-versions
    /// heuristic.
    version: Rc<Cell<usize>>,
    gathers: Rc<RefCell<IndexMap<RevisableId, Box<dyn Revisable>>>>,
    /// Per-dependency version snapshot at the last mapper run, in the same
    /// order as `gathers`. Used to detect changes without sum collisions.
    dep_versions: Rc<RefCell<Vec<usize>>>,
    view_ids: Rc<RefCell<IndexMap<ViewId, usize>>>,
    #[educe(Debug(ignore))]
    mapper: Rc<Box<dyn Fn() -> T + 'static>>,
    /// Optional equality gate. When `Some`, a fresh mapper run that
    /// produces a value equal to the previous one does NOT bump `version`,
    /// so observers treat it as "unchanged" and skip re-rendering.
    /// `None` is the default and matches the no-gate behaviour.
    #[educe(Debug(ignore))]
    eq: Option<Rc<dyn Fn(&T, &T) -> bool + 'static>>,
    #[educe(Debug(ignore))]
    value: Rc<RefCell<T>>,
}

impl<T> Bond<T>
where
    T: fmt::Debug + 'static,
{
    pub fn new(mapper: impl Fn() -> T + 'static) -> Self {
        let (gathers, value) = crate::reflow::gather(&mapper);
        let dep_versions: Vec<usize> = gathers.values().map(|g| g.version()).collect();
        Self {
            id: RevisableId::next(),
            version: Rc::new(Cell::new(1)),
            gathers: Rc::new(RefCell::new(gathers)),
            dep_versions: Rc::new(RefCell::new(dep_versions)),
            view_ids: Default::default(),
            mapper: Rc::new(Box::new(mapper)),
            eq: None,
            value: Rc::new(RefCell::new(value)),
        }
    }

    /// Returns a new Bond with a custom equality gate. After a mapper
    /// re-run, the new and previous outputs are compared via `eq`; if
    /// they're considered equal, the Bond's reported `version` stays
    /// put and downstream observers do not re-render. Useful when the
    /// mapper's dependency set is noisier than its output.
    pub fn with_eq(mut self, eq: impl Fn(&T, &T) -> bool + 'static) -> Self {
        self.eq = Some(Rc::new(eq));
        self
    }

    /// Convenience: gate updates by `PartialEq`. Equivalent to
    /// `.with_eq(|a, b| a == b)` for any `T: PartialEq`.
    pub fn with_partial_eq(self) -> Self
    where
        T: PartialEq,
    {
        self.with_eq(|a: &T, b: &T| a == b)
    }

    /// Returns `true` when any current dependency has bumped its version
    /// since the last mapper run, or when the dependency set has changed.
    fn deps_changed(&self) -> bool {
        let gathers = self.gathers.borrow();
        let snapshot = self.dep_versions.borrow();
        if gathers.len() != snapshot.len() {
            return true;
        }
        for (gather, prev) in gathers.values().zip(snapshot.iter()) {
            if gather.version() != *prev {
                return true;
            }
        }
        false
    }

    /// Recompute the value and, if it actually changed under the
    /// configured equality gate, bump the Bond's version and refresh
    /// dependency bindings. Returns whether the version was bumped.
    fn recompute(&self) -> bool {
        let (new_gathers, new_value) = crate::reflow::gather(|| (self.mapper)());
        let new_snapshot: Vec<usize> = new_gathers.values().map(|g| g.version()).collect();

        let value_changed = match &self.eq {
            Some(eq) => {
                let prev = self.value.borrow();
                !eq(&*prev, &new_value)
            }
            None => true,
        };

        self.value.replace(new_value);
        *self.gathers.borrow_mut() = new_gathers;
        *self.dep_versions.borrow_mut() = new_snapshot;

        if value_changed {
            self.version.set(self.version.get().wrapping_add(1));
        }
        value_changed
    }

    pub fn get(&self) -> Ref<'_, T> {
        if self.deps_changed() {
            let value_changed = self.recompute();
            if value_changed {
                for view_id in self.view_ids.borrow().keys() {
                    for (_, gather) in self.gathers.borrow().deref() {
                        gather.bind_view(view_id);
                    }
                }
            }
        }

        // Always relay our dependencies up to the outer tracking layer
        // so the caller subscribes transitively, regardless of whether
        // we just re-ran or skipped via the eq gate.
        let gathers = &self.gathers;
        TRACKING_STACK.with(|tracking_items| {
            let mut tracking_items = tracking_items.borrow_mut();
            if !tracking_items.is_idle() {
                for revisable in gathers.borrow().values() {
                    tracking_items.track(revisable.clone_boxed());
                }
            }
        });

        self.value.borrow()
    }
    pub fn get_untracked(&self) -> Ref<'_, T> {
        if self.deps_changed() {
            self.recompute();
        }
        self.value.borrow()
    }

    pub fn map<M, G>(&self, mapper: M) -> Bond<G>
    where
        M: Fn(Ref<'_, T>) -> G + Clone + 'static,
        G: fmt::Debug + 'static,
    {
        let this = self.clone();
        Bond::new(move || mapper(this.get()))
    }
}

impl<T> Clone for Bond<T>
where
    T: fmt::Debug + 'static,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            version: self.version.clone(),
            gathers: self.gathers.clone(),
            dep_versions: self.dep_versions.clone(),
            view_ids: self.view_ids.clone(),
            value: self.value.clone(),
            mapper: self.mapper.clone(),
            eq: self.eq.clone(),
        }
    }
}

impl<T> Revisable for Bond<T>
where
    T: fmt::Debug + 'static,
{
    fn id(&self) -> RevisableId {
        self.id
    }
    #[cfg(not(feature = "single-app"))]
    fn holder_id(&self) -> Option<crate::HolderId> {
        self.view_ids.borrow().first().map(|(view_id, _)| view_id.holder_id())
    }

    fn version(&self) -> usize {
        self.version.get()
    }
    fn view_ids(&self) -> Rc<RefCell<IndexSet<ViewId>>> {
        Rc::new(RefCell::new(IndexSet::from_iter(self.view_ids.borrow().keys().cloned())))
    }
    fn is_revising(&self) -> bool {
        for (_, gather) in self.gathers.borrow().deref() {
            if gather.is_revising() {
                return true;
            }
        }
        false
    }
    fn bind_view(&self, view_id: &ViewId) {
        let mut view_ids = self.view_ids.borrow_mut();
        let count = view_ids.get(view_id).cloned().unwrap_or(0);
        view_ids.insert(view_id.clone(), count + 1);
        for (_, gather) in self.gathers.borrow().deref() {
            gather.bind_view(view_id);
        }
    }
    fn unbind_view(&self, view_id: &ViewId) {
        let count = self.view_ids.borrow_mut().shift_remove(view_id).unwrap_or(0);
        for (_, gather) in self.gathers.borrow().deref() {
            gather.unlace_view(view_id, count);
        }
    }
    fn unlace_view(&self, view_id: &ViewId, loose: usize) {
        let count = self.view_ids.borrow_mut().get(view_id).cloned().unwrap_or(0);
        let loose = if loose >= count {
            self.view_ids.borrow_mut().shift_remove(view_id);
            count
        } else {
            self.view_ids.borrow_mut().insert(view_id.clone(), count - loose);
            loose
        };
        for (_, gather) in self.gathers.borrow().deref() {
            gather.unlace_view(view_id, loose);
        }
    }
    fn clone_boxed(&self) -> Box<dyn Revisable> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cage;

    #[test]
    fn bond_recomputes_when_dep_revises() {
        let cage = Cage::new(1_i32);
        let cage_for_bond = cage.clone();
        let bond = Bond::new(move || *cage_for_bond.get() * 10);
        assert_eq!(*bond.get(), 10);
        let v0 = bond.version();

        cage.revise(|mut v| *v = 2);
        // get() forces recompute and reports new value
        assert_eq!(*bond.get(), 20);
        assert!(bond.version() > v0, "Bond version should bump after dep change");
    }

    #[test]
    fn cage_subscriber_count_tracks_bindings() {
        // Without a view binding, count is zero.
        let cage = Cage::new(1_i32);
        assert_eq!(cage.subscriber_count(), 0);

        // bind_view directly (the framework normally does this on a view's
        // first `.get()` inside a tracking context).
        cage.bind_view(&crate::view::ViewId::new(
            #[cfg(not(feature = "single-app"))]
            crate::HolderId::null(),
            "test".to_string(),
        ));
        assert_eq!(cage.subscriber_count(), 1);
        assert_eq!(cage.subscriber_view_ids().len(), 1);
    }

    #[test]
    fn bond_with_partial_eq_skips_version_bump_when_value_unchanged() {
        let cage = Cage::new(1_i32);
        let cage_for_bond = cage.clone();
        // Map any non-zero input to the same constant; PartialEq gate
        // should prevent version churn when the cage flips between
        // values that map to the same output.
        let bond = Bond::new(move || if *cage_for_bond.get() != 0 { 42 } else { 0 }).with_partial_eq();
        let _ = *bond.get();
        let v0 = bond.version();

        cage.revise(|mut v| *v = 7);
        let _ = *bond.get();
        assert_eq!(bond.version(), v0, "PartialEq gate should suppress version bump");

        // ...but a real change still bumps.
        cage.revise(|mut v| *v = 0);
        let _ = *bond.get();
        assert!(bond.version() > v0, "value-changing revise must still bump version");
    }
}
