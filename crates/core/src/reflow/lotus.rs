#[cfg(not(feature = "__single_holder"))]
use std::cell::Cell;
use std::cell::{Ref, RefCell};
use std::fmt::{self, Display};
use std::hash::Hash;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use indexmap::{IndexMap, IndexSet};

use super::{Bond, Cage, Revisable, RevisableId};
#[cfg(not(feature = "__single_holder"))]
use crate::HolderId;
use crate::ViewId;

#[derive(Debug)]
pub enum Lotus<T>
where
    T: fmt::Debug + 'static,
{
    #[cfg(feature = "__single_holder")]
    Bare(Rc<RefCell<T>>),
    #[cfg(not(feature = "__single_holder"))]
    Bare {
        holder_id: Rc<Cell<Option<HolderId>>>,
        data: Rc<RefCell<T>>,
    },
    Cage(Cage<T>),
    Bond(Bond<T>),
}

impl<T> Lotus<T>
where
    T: fmt::Debug + 'static,
{
    pub fn get(&self) -> Ref<'_, T> {
        match self {
            #[cfg(feature = "__single_holder")]
            Self::Bare(value) => RefCell::borrow(&**value),
            #[cfg(not(feature = "__single_holder"))]
            Self::Bare { data, .. } => RefCell::borrow(&**data),
            Self::Cage(cage) => cage.get(),
            Self::Bond(bond) => bond.get(),
        }
    }
    pub fn get_untracked(&self) -> Ref<'_, T> {
        match self {
            #[cfg(feature = "__single_holder")]
            Self::Bare(value) => RefCell::borrow(&**value),
            #[cfg(not(feature = "__single_holder"))]
            Self::Bare { data, .. } => RefCell::borrow(&**data),
            Self::Cage(cage) => cage.get_untracked(),
            Self::Bond(bond) => bond.get_untracked(),
        }
    }
    pub fn map<O>(&self, mapper: impl Fn(&T) -> O + 'static) -> Lotus<O>
    where
        O: fmt::Debug + 'static,
    {
        match self {
            #[cfg(feature = "__single_holder")]
            Self::Bare(value) => Lotus::Bare(Rc::new(RefCell::new(mapper(&value.borrow())))),
            #[cfg(not(feature = "__single_holder"))]
            Self::Bare { data, holder_id } => Lotus::Bare {
                data: Rc::new(RefCell::new(mapper(&data.borrow()))),
                holder_id: holder_id.clone(),
            },
            _ => {
                let this = self.clone();
                let bond = Bond::new(move || mapper(&this.get()));
                Lotus::Bond(bond)
            }
        }
    }
}
impl<T> Clone for Lotus<T>
where
    T: fmt::Debug + 'static,
{
    fn clone(&self) -> Self {
        match self {
            #[cfg(feature = "__single_holder")]
            Self::Bare(rc) => Self::Bare(rc.clone()),
            #[cfg(not(feature = "__single_holder"))]
            Self::Bare { data, holder_id } => Self::Bare {
                data: data.clone(),
                holder_id: holder_id.clone(),
            },
            Self::Cage(cage) => Self::Cage(cage.clone()),
            Self::Bond(bond) => Self::Bond(bond.clone()),
        }
    }
}

impl<T> Revisable for Lotus<T>
where
    T: fmt::Debug + 'static,
{
    fn id(&self) -> RevisableId {
        match self {
            #[cfg(feature = "__single_holder")]
            Self::Bare(_) => RevisableId(0),
            #[cfg(not(feature = "__single_holder"))]
            Self::Bare { .. } => RevisableId(0),
            Self::Cage(cage) => cage.id(),
            Self::Bond(bond) => bond.id(),
        }
    }
    #[cfg(not(feature = "__single_holder"))]
    fn holder_id(&self) -> Option<HolderId> {
        match self {
            Self::Bare { holder_id, .. } => (&*holder_id.clone()).clone().into_inner(),
            Self::Cage(cage) => cage.holder_id(),
            Self::Bond(bond) => bond.holder_id(),
        }
    }
    fn version(&self) -> usize {
        match self {
            #[cfg(feature = "__single_holder")]
            Self::Bare(_) => 0,
            #[cfg(not(feature = "__single_holder"))]
            Self::Bare { .. } => 0,
            Self::Cage(cage) => cage.version(),
            Self::Bond(bond) => bond.version(),
        }
    }
    fn view_ids(&self) -> Rc<RefCell<IndexSet<ViewId>>> {
        match self {
            #[cfg(feature = "__single_holder")]
            Self::Bare(_) => Rc::new(RefCell::new(IndexSet::new())),
            #[cfg(not(feature = "__single_holder"))]
            Self::Bare { .. } => Rc::new(RefCell::new(IndexSet::new())),
            Self::Cage(cage) => cage.view_ids(),
            Self::Bond(bond) => bond.view_ids(),
        }
    }
    fn bind_view(&self, view_id: &ViewId) {
        match self {
            #[cfg(feature = "__single_holder")]
            Self::Bare(value_) => {}
            #[cfg(not(feature = "__single_holder"))]
            Self::Bare { holder_id, .. } => {
                holder_id.set(Some(view_id.holder_id()));
            }
            Self::Cage(cage) => cage.bind_view(view_id),
            Self::Bond(bond) => bond.bind_view(view_id),
        }
    }
    fn unbind_view(&self, view_id: &ViewId) {
        match self {
            #[cfg(feature = "__single_holder")]
            Self::Bare(_) => {}
            #[cfg(not(feature = "__single_holder"))]
            Self::Bare { .. } => {}
            Self::Cage(cage) => cage.unbind_view(view_id),
            Self::Bond(bond) => bond.unbind_view(view_id),
        }
    }
    fn unlace_view(&self, view_id: &ViewId, loose: usize) {
        match self {
            #[cfg(feature = "__single_holder")]
            Self::Bare(_) => {}
            #[cfg(not(feature = "__single_holder"))]
            Self::Bare { .. } => {}
            Self::Cage(cage) => cage.unlace_view(view_id, loose),
            Self::Bond(bond) => bond.unlace_view(view_id, loose),
        }
    }
    fn is_revising(&self) -> bool {
        match self {
            #[cfg(feature = "__single_holder")]
            Self::Bare(_) => false,
            #[cfg(not(feature = "__single_holder"))]
            Self::Bare { .. } => false,
            Self::Cage(cage) => cage.is_revising(),
            Self::Bond(bond) => bond.is_revising(),
        }
    }
    fn clone_boxed(&self) -> Box<dyn Revisable> {
        match self {
            #[cfg(feature = "__single_holder")]
            Self::Bare(rc) => Box::new(Self::Bare(rc.clone())),
            #[cfg(not(feature = "__single_holder"))]
            Self::Bare { data, holder_id } => Box::new(Self::Bare {
                data: data.clone(),
                holder_id: holder_id.clone(),
            }),
            Self::Cage(cage) => Box::new(Self::Cage(cage.clone())),
            Self::Bond(bond) => Box::new(Self::Bond(bond.clone())),
        }
    }
}

impl<T> From<Cage<T>> for Lotus<T>
where
    T: fmt::Debug + 'static,
{
    fn from(value: Cage<T>) -> Self {
        Self::Cage(value)
    }
}

impl<T> From<Bond<T>> for Lotus<T>
where
    T: fmt::Debug + 'static,
{
    fn from(value: Bond<T>) -> Self {
        Self::Bond(value)
    }
}

impl<'a> From<&'a str> for Lotus<String> {
    #[cfg(feature = "__single_holder")]
    fn from(value: &'a str) -> Self {
        Self::Bare(Rc::new(RefCell::new(value.to_owned())))
    }
    #[cfg(not(feature = "__single_holder"))]
    fn from(value: &'a str) -> Self {
        Self::Bare {
            data: Rc::new(RefCell::new(value.to_owned())),
            holder_id: Default::default(),
        }
    }
}
impl<'a> From<&'a String> for Lotus<String> {
    #[cfg(feature = "__single_holder")]
    fn from(value: &'a String) -> Self {
        Self::Bare(Rc::new(RefCell::new(value.to_owned())))
    }
    #[cfg(not(feature = "__single_holder"))]
    fn from(value: &'a String) -> Self {
        Self::Bare {
            data: Rc::new(RefCell::new(value.to_owned())),
            holder_id: Default::default(),
        }
    }
}

impl<'a> From<&'a str> for Lotus<Option<String>> {
    #[cfg(feature = "__single_holder")]
    fn from(value: &'a str) -> Self {
        Self::Bare(Rc::new(RefCell::new(Some(value.to_owned()))))
    }
    #[cfg(not(feature = "__single_holder"))]
    fn from(value: &'a str) -> Self {
        Self::Bare {
            data: Rc::new(RefCell::new(Some(value.to_owned()))),
            holder_id: Default::default(),
        }
    }
}
impl<'a> From<&'a String> for Lotus<Option<String>> {
    #[cfg(feature = "__single_holder")]
    fn from(value: &'a String) -> Self {
        Self::Bare(Rc::new(RefCell::new(Some(value.to_owned()))))
    }
    #[cfg(not(feature = "__single_holder"))]
    fn from(value: &'a String) -> Self {
        Self::Bare {
            data: Rc::new(RefCell::new(Some(value.to_owned()))),
            holder_id: Default::default(),
        }
    }
}

impl<T> From<T> for Lotus<T>
where
    T: fmt::Debug + 'static,
{
    #[cfg(feature = "__single_holder")]
    fn from(value: T) -> Self {
        Self::Bare(Rc::new(RefCell::new(value)))
    }
    #[cfg(not(feature = "__single_holder"))]
    fn from(value: T) -> Self {
        Self::Bare {
            data: Rc::new(RefCell::new(value)),
            holder_id: Default::default(),
        }
    }
}

impl<T> From<T> for Lotus<Option<T>>
where
    T: fmt::Debug + 'static,
{
    #[cfg(feature = "__single_holder")]
    fn from(value: T) -> Self {
        Self::Bare(Rc::new(RefCell::new(Some(value))))
    }
    #[cfg(not(feature = "__single_holder"))]
    fn from(value: T) -> Self {
        Self::Bare {
            data: Rc::new(RefCell::new(Some(value))),
            holder_id: Default::default(),
        }
    }
}

impl<T> From<Lotus<T>> for Lotus<Option<T>>
where
    T: fmt::Debug + Clone + 'static,
{
    fn from(value: Lotus<T>) -> Self {
        value.map(|v| Some(v.clone()))
    }
}

impl<T> Default for Lotus<T>
where
    T: Default + fmt::Debug + 'static,
{
    #[cfg(feature = "__single_holder")]
    fn default() -> Self {
        Self::Bare(Default::default())
    }
    #[cfg(not(feature = "__single_holder"))]
    fn default() -> Self {
        Self::Bare {
            data: Default::default(),
            holder_id: Default::default(),
        }
    }
}
