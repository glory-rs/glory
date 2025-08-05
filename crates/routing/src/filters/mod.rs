//! filter module
//!
//! This module provides filters for routing requests based on various criteria
//! such as uri scheme, hostname, port, path, and HTTP method.

mod opts;
mod path;

use std::fmt::{self, Formatter};

use glory_core::Truck;

use crate::PathState;
use crate::url::Url;

use self::opts::*;

pub use path::*;

/// Fiter trait for filter request.
pub trait Filter: fmt::Debug + 'static {
    #[doc(hidden)]
    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Self>()
    }
    #[doc(hidden)]
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    /// Create a new filter use `And` filter.
    #[inline]
    fn and<F>(self, other: F) -> And<Self, F>
    where
        Self: Sized,
        F: Filter,
    {
        And { first: self, second: other }
    }

    /// Create a new filter use `Or` filter.
    #[inline]
    fn or<F>(self, other: F) -> Or<Self, F>
    where
        Self: Sized,
        F: Filter,
    {
        Or { first: self, second: other }
    }

    /// Create a new filter use `AndThen` filter.
    #[inline]
    fn and_then<F>(self, fun: F) -> AndThen<Self, F>
    where
        Self: Sized,
        F: Fn(&Url, &Truck, &mut PathState) -> bool + 'static,
    {
        AndThen { filter: self, callback: fun }
    }

    /// Create a new filter use `OrElse` filter.
    #[inline]
    fn or_else<F>(self, fun: F) -> OrElse<Self, F>
    where
        Self: Sized,
        F: Fn(&Url, &Truck, &mut PathState) -> bool + 'static,
    {
        OrElse { filter: self, callback: fun }
    }

    /// Filter `Request` and returns false or true.
    fn filter(&self, url: &Url, truck: &Truck, path: &mut PathState) -> bool;
}

/// `FnFilter` accepts a function as it's param, use this function to filter request.
#[derive(Copy, Clone)]
#[allow(missing_debug_implementations)]
pub struct FnFilter<F>(pub F);

impl<F> Filter for FnFilter<F>
where
    F: Fn(&Url, &Truck, &mut PathState) -> bool + 'static,
{
    #[inline]
    fn filter(&self, url: &Url, truck: &Truck, path: &mut PathState) -> bool {
        self.0(url, truck, path)
    }
}

impl<F> fmt::Debug for FnFilter<F> {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "fn:fn")
    }
}

/// Filter request use `PathFilter`.
#[inline]
pub fn path(path: impl Into<String>) -> PathFilter {
    PathFilter::new(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_methods() {
        assert!(get() == MethodFilter(Method::GET));
        assert!(head() == MethodFilter(Method::HEAD));
        assert!(options() == MethodFilter(Method::OPTIONS));
        assert!(post() == MethodFilter(Method::POST));
        assert!(patch() == MethodFilter(Method::PATCH));
        assert!(put() == MethodFilter(Method::PUT));
        assert!(delete() == MethodFilter(Method::DELETE));
    }

    #[test]
    fn test_opts() {
        fn has_one(_req: &mut Request, path: &mut PathState) -> bool {
            path.parts.contains(&"one".into())
        }
        fn has_two(_req: &mut Request, path: &mut PathState) -> bool {
            path.parts.contains(&"two".into())
        }

        let one_filter = FnFilter(has_one);
        let two_filter = FnFilter(has_two);

        let mut req = Request::default();
        let mut path_state = PathState::new("http://localhost/one");
        assert!(one_filter.filter(&mut req, &mut path_state));
        assert!(!two_filter.filter(&mut req, &mut path_state));
        assert!(one_filter.or_else(has_two).filter(&mut req, &mut path_state));
        assert!(one_filter.or(two_filter).filter(&mut req, &mut path_state));
        assert!(!one_filter.and_then(has_two).filter(&mut req, &mut path_state));
        assert!(!one_filter.and(two_filter).filter(&mut req, &mut path_state));

        let mut path_state = PathState::new("http://localhost/one/two");
        assert!(one_filter.filter(&mut req, &mut path_state));
        assert!(two_filter.filter(&mut req, &mut path_state));
        assert!(one_filter.or_else(has_two).filter(&mut req, &mut path_state));
        assert!(one_filter.or(two_filter).filter(&mut req, &mut path_state));
        assert!(one_filter.and_then(has_two).filter(&mut req, &mut path_state));
        assert!(one_filter.and(two_filter).filter(&mut req, &mut path_state));

        let mut path_state = PathState::new("http://localhost/two");
        assert!(!one_filter.filter(&mut req, &mut path_state));
        assert!(two_filter.filter(&mut req, &mut path_state));
        assert!(one_filter.or_else(has_two).filter(&mut req, &mut path_state));
        assert!(one_filter.or(two_filter).filter(&mut req, &mut path_state));
        assert!(!one_filter.and_then(has_two).filter(&mut req, &mut path_state));
        assert!(!one_filter.and(two_filter).filter(&mut req, &mut path_state));
    }
}
