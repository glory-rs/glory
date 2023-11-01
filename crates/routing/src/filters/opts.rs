use std::fmt::{self, Formatter};

use glory_core::Truck;
use crate::url::Url;

use crate::{Filter, PathState};

#[derive(Clone, Copy, Debug)]
pub struct Or<T, U> {
    pub(super) first: T,
    pub(super) second: U,
}

impl<T, U> Filter for Or<T, U>
where
    T: Filter,
    U: Filter,
{
    #[inline]
    fn filter(&self, url: &Url, truck: &Truck, state: &mut PathState) -> bool {
        if self.first.filter(url, truck, state) {
            true
        } else {
            self.second.filter(url, truck, state)
        }
    }
}

#[derive(Clone, Copy)]
pub struct OrElse<T, F> {
    pub(super) filter: T,
    pub(super) callback: F,
}

impl<T, F> Filter for OrElse<T, F>
where
    T: Filter,
    F: Fn(&Truck, &mut PathState) -> bool + 'static,
{
    #[inline]
    fn filter(&self, url: &Url, truck: &Truck, state: &mut PathState) -> bool {
        if self.filter.filter(url, truck, state) {
            true
        } else {
            (self.callback)(truck, state)
        }
    }
}

impl<T, F> fmt::Debug for OrElse<T, F> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "opt:or_else")
    }
}

#[derive(Clone, Copy, Debug)]
pub struct And<T, U> {
    pub(super) first: T,
    pub(super) second: U,
}

impl<T, U> Filter for And<T, U>
where
    T: Filter,
    U: Filter,
{
    #[inline]
    fn filter(&self, url: &Url, truck: &Truck, state: &mut PathState) -> bool {
        if !self.first.filter(url, truck, state) {
            false
        } else {
            self.second.filter(url, truck, state)
        }
    }
}

#[derive(Clone, Copy)]
pub struct AndThen<T, F> {
    pub(super) filter: T,
    pub(super) callback: F,
}

impl<T, F> Filter for AndThen<T, F>
where
    T: Filter,
    F: Fn(&Truck, &mut PathState) -> bool + 'static,
{
    #[inline]
    fn filter(&self, url: &Url, truck: &Truck, state: &mut PathState) -> bool {
        if !self.filter.filter(url, truck, state) {
            false
        } else {
            (self.callback)(truck, state)
        }
    }
}

impl<T, F> fmt::Debug for AndThen<T, F> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "opt:and_then")
    }
}
