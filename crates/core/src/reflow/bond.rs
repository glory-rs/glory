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
    view_ids: Rc<RefCell<IndexMap<ViewId, usize>>>,
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
        let (gathers, value) = crate::reflow::gather(mapper.clone());
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
    
    pub fn map<M, G>(&self, mapper: M) -> Bond<impl Fn() -> G + Clone + 'static, G>
    where
        M: Fn(Ref<'_, T>) -> G + Clone + 'static,
        G: fmt::Debug + 'static,
    {
        let this = self.clone();
        Bond::new(move || mapper(this.get()))
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
            *self.gathers.borrow_mut() = crate::reflow::gather(|| self.value.replace((self.mapper)())).0;
            self.version.set(new_version);
            for view_id in self.view_ids.borrow().keys() {
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
        self.view_ids.borrow().first().map(|(view_id, _)| view_id.holder_id())
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
        let mut view_ids = self.view_ids.borrow_mut();
        let count = view_ids.get(view_id).cloned().unwrap_or(0);
        view_ids.insert(view_id.clone(), count + 1);
        for (_, gather) in self.gathers.borrow().deref() {
            gather.bind_view(view_id);
        }
    }
    fn unbind_view(&self, view_id: &ViewId) {
        let count = self.view_ids.borrow_mut().remove(view_id).unwrap_or(0);
        for (_, gather) in self.gathers.borrow().deref() {
            gather.unlace_view(view_id, count);
        }
    }
    fn unlace_view(&self, view_id: &ViewId, loose: usize) {
        let count = self.view_ids.borrow_mut().get(view_id).cloned().unwrap_or(0);
        let loose = if loose >= count {
            self.view_ids.borrow_mut().remove(view_id);
            count
        } else {
            self.view_ids.borrow_mut().insert(view_id.clone(), count - loose);
            loose
        };
        for (_, gather) in self.gathers.borrow().deref() {
            gather.unlace_view(view_id, loose);
        }
    }
}
