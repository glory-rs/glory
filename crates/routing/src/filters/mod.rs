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

/// Filter trait for routing requests.
pub trait Filter: fmt::Debug + 'static {
    #[doc(hidden)]
    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Self>()
    }
    #[doc(hidden)]
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    /// Create a new filter using `And`.
    #[inline]
    fn and<F>(self, other: F) -> And<Self, F>
    where
        Self: Sized,
        F: Filter,
    {
        And { first: self, second: other }
    }

    /// Create a new filter using `Or`.
    #[inline]
    fn or<F>(self, other: F) -> Or<Self, F>
    where
        Self: Sized,
        F: Filter,
    {
        Or { first: self, second: other }
    }

    /// Create a new filter using `AndThen`.
    #[inline]
    fn and_then<F>(self, fun: F) -> AndThen<Self, F>
    where
        Self: Sized,
        F: Fn(&Url, &Truck, &mut PathState) -> bool + 'static,
    {
        AndThen { filter: self, callback: fun }
    }

    /// Create a new filter using `OrElse`.
    #[inline]
    fn or_else<F>(self, fun: F) -> OrElse<Self, F>
    where
        Self: Sized,
        F: Fn(&Url, &Truck, &mut PathState) -> bool + 'static,
    {
        OrElse { filter: self, callback: fun }
    }

    /// Filter a request path.
    fn filter(&self, url: &Url, truck: &Truck, path: &mut PathState) -> bool;

    /// Relative specificity of this filter, used to rank sibling routes.
    /// Higher wins. Defaults to `0`; [`PathFilter`](crate::filters::PathFilter)
    /// overrides it from its segments so static routes outrank dynamic ones,
    /// which outrank catch-alls — independent of registration order.
    #[inline]
    fn specificity(&self) -> i32 {
        0
    }
}

/// `FnFilter` accepts a function and uses it to filter a request.
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

/// Filter requests using a `PathFilter`.
#[inline]
pub fn path(path: impl Into<String>) -> PathFilter {
    PathFilter::new(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opts() {
        fn has_one(_url: &Url, _truck: &Truck, path: &mut PathState) -> bool {
            path.segments.contains(&"one".into())
        }
        fn has_two(_url: &Url, _truck: &Truck, path: &mut PathState) -> bool {
            path.segments.contains(&"two".into())
        }

        let one_filter = FnFilter(has_one);
        let two_filter = FnFilter(has_two);
        let url = Url::parse("http://localhost/").unwrap();
        let truck = Truck::new();

        let mut path_state = PathState::new("/one");
        assert!(one_filter.filter(&url, &truck, &mut path_state));
        assert!(!two_filter.filter(&url, &truck, &mut path_state));
        assert!(one_filter.or_else(has_two).filter(&url, &truck, &mut path_state));
        assert!(one_filter.or(two_filter).filter(&url, &truck, &mut path_state));
        assert!(!one_filter.and_then(has_two).filter(&url, &truck, &mut path_state));
        assert!(!one_filter.and(two_filter).filter(&url, &truck, &mut path_state));

        let mut path_state = PathState::new("/one/two");
        assert!(one_filter.filter(&url, &truck, &mut path_state));
        assert!(two_filter.filter(&url, &truck, &mut path_state));
        assert!(one_filter.or_else(has_two).filter(&url, &truck, &mut path_state));
        assert!(one_filter.or(two_filter).filter(&url, &truck, &mut path_state));
        assert!(one_filter.and_then(has_two).filter(&url, &truck, &mut path_state));
        assert!(one_filter.and(two_filter).filter(&url, &truck, &mut path_state));

        let mut path_state = PathState::new("/two");
        assert!(!one_filter.filter(&url, &truck, &mut path_state));
        assert!(two_filter.filter(&url, &truck, &mut path_state));
        assert!(one_filter.or_else(has_two).filter(&url, &truck, &mut path_state));
        assert!(one_filter.or(two_filter).filter(&url, &truck, &mut path_state));
        assert!(!one_filter.and_then(has_two).filter(&url, &truck, &mut path_state));
        assert!(!one_filter.and(two_filter).filter(&url, &truck, &mut path_state));
    }
}
