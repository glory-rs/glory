use std::cell::{Cell, Ref, RefCell};
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

use educe::Educe;
use indexmap::{IndexMap, IndexSet};

use super::{Record, Revisable, RevisableId, Signal, TRACKING_STACK};
use crate::ViewId;

#[derive(Educe)]
#[educe(Debug)]
pub struct Bond<F, T>
where
    F: Fn() -> T + Clone + 'static,
    T: fmt::Debug + 'static,
{
    id: RevisableId,
    version: Rc<Cell<usize>>,
    gathers: Rc<RefCell<IndexMap<RevisableId, Box<dyn Signal>>>>,
    view_ids: Rc<RefCell<IndexSet<ViewId>>>,
    #[educe(Debug(ignore))]
    mapper: F,
    #[educe(Debug(ignore))]
    value: Rc<RefCell<T>>,
}

impl<F, T> Bond<F, T>
where
    F: Fn() -> T + Clone + 'static,
    T: fmt::Debug + 'static,
{
    pub fn new(mapper: F) -> Self {
        TRACKING_STACK.with(|tracking_stack| tracking_stack.borrow_mut().push_layer());
        let value = (mapper)();
        let gathers = TRACKING_STACK.with(|tracking_stack| tracking_stack.borrow_mut().pop_layer().unwrap());
        let version = gathers.values().map(|g| g.version()).sum();
        Self {
            id: RevisableId::next(),
            version: Rc::new(Cell::new(version)),
            gathers: Rc::new(RefCell::new(gathers)),
            view_ids: Default::default(),
            mapper,
            value: Rc::new(RefCell::new(value)),
        }
    }
}

impl<F, T> Clone for Bond<F, T>
where
    F: Fn() -> T + Clone + 'static,
    T: fmt::Debug + 'static,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            version: self.version.clone(),
            gathers: self.gathers.clone(),
            view_ids: self.view_ids.clone(),
            value: Rc::new(RefCell::new((self.mapper)())),
            mapper: self.mapper.clone(),
        }
    }
}

impl<F, T> Record<T> for Bond<F, T>
where
    F: Fn() -> T + Clone + 'static,
    T: fmt::Debug + 'static,
{
    fn get(&self) -> Ref<'_, T> {
        let new_version = self.gathers.borrow().values().map(|g| g.version()).sum();
        if self.version() != new_version {
            TRACKING_STACK.with(|tracking_items| tracking_items.borrow_mut().push_layer());
            self.value.replace((self.mapper)());
            let gathers = TRACKING_STACK.with(|tracking_items| tracking_items.borrow_mut().pop_layer().unwrap());
            *self.gathers.borrow_mut() = gathers;
            self.version.set(new_version);
            for view_id in self.view_ids.borrow().iter() {
                for (_, gather) in self.gathers.borrow().deref() {
                    gather.bind_view(view_id);
                }
            }
        } else {
            let gathers = &self.gathers;
            TRACKING_STACK.with(|tracking_items| {
                let mut tracking_items = tracking_items.borrow_mut();
                if !tracking_items.is_idle() {
                    for singal in gathers.borrow().values() {
                        tracking_items.track(singal.clone_boxed());
                    }
                }
            });
        }

        self.value.borrow()
    }
    fn get_untracked(&self) -> Ref<'_, T> {
        let new_version = self.gathers.borrow().values().map(|g| g.version()).sum();
        if self.version() != new_version {
            self.value.replace((self.mapper)());
            self.version.set(new_version);
        }
        self.value.borrow()
    }
}

impl<F, T> Revisable for Bond<F, T>
where
    F: Fn() -> T + Clone + 'static,
    T: fmt::Debug + 'static,
{
    fn id(&self) -> RevisableId {
        self.id
    }
    #[cfg(not(feature = "__single_holder"))]
    fn holder_id(&self) -> Option<crate::HolderId> {
        self.view_ids.borrow().first().map(|view_id| view_id.holder_id())
    }
    fn version(&self) -> usize {
        self.version.get()
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
        self.view_ids.borrow_mut().insert(view_id.clone());
        for (_, gather) in self.gathers.borrow().deref() {
            gather.bind_view(view_id);
        }
    }
}
