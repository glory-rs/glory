use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use crate::{Aviator, NavigationError};

/// A typed route that can round-trip to and from the URL form used by
/// [`Aviator::goto`].
///
/// This trait is intentionally small: applications can implement it for an
/// enum today, while a future derive macro can generate the same contract.
pub trait Routable: Sized {
    /// Serialize this route to a browser/server navigation URL, usually a
    /// root-relative path such as `/users/42?page=1`.
    fn to_url(&self) -> String;

    /// Parse a URL back into this route type.
    fn from_url(url: &str) -> Option<Self>;
}

/// Convenience methods for navigating with a [`Routable`] value.
pub trait AviatorExt: Aviator {
    /// Navigate to a typed route using the active history backend.
    fn goto_route<R>(&self, route: &R) -> Result<(), NavigationError>
    where
        R: Routable,
    {
        let url = route.to_url();
        self.goto(&url)
    }
}

impl<A> AviatorExt for A where A: Aviator + ?Sized {}

/// Percent-encode a value for use in a single path segment.
pub fn encode_route_param(value: impl fmt::Display) -> String {
    glory_core::web::escape(&value.to_string())
}

/// Percent-decode a raw path segment before parsing it as a typed parameter.
pub fn decode_route_param(value: &str) -> String {
    glory_core::web::unescape(value)
}

/// Error returned when a path parameter cannot be read as the requested type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteParamError {
    Missing {
        name: String,
    },
    Invalid {
        name: Option<String>,
        value: String,
        expected: &'static str,
    },
}

impl RouteParamError {
    fn invalid(value: impl Into<String>, expected: &'static str) -> Self {
        Self::Invalid {
            name: None,
            value: value.into(),
            expected,
        }
    }

    fn with_name(self, name: &str) -> Self {
        match self {
            Self::Invalid { value, expected, .. } => Self::Invalid {
                name: Some(name.to_owned()),
                value,
                expected,
            },
            other => other,
        }
    }
}

impl fmt::Display for RouteParamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { name } => write!(f, "missing route parameter `{name}`"),
            Self::Invalid {
                name: Some(name),
                value,
                expected,
            } => write!(f, "route parameter `{name}` with value `{value}` is not a valid {expected}"),
            Self::Invalid { name: None, value, expected } => write!(f, "route parameter value `{value}` is not a valid {expected}"),
        }
    }
}

impl std::error::Error for RouteParamError {}

/// Parse a decoded path parameter as a concrete Rust type.
pub trait FromRouteParam: Sized {
    fn from_route_param(value: &str) -> Result<Self, RouteParamError>;
}

impl<T> FromRouteParam for T
where
    T: FromStr,
{
    fn from_route_param(value: &str) -> Result<Self, RouteParamError> {
        value
            .parse::<T>()
            .map_err(|_| RouteParamError::invalid(value, std::any::type_name::<T>()))
    }
}

/// Parse a decoded parameter value.
pub fn parse_route_param<T>(value: &str) -> Result<T, RouteParamError>
where
    T: FromRouteParam,
{
    T::from_route_param(value)
}

/// Read and parse a named parameter from a router `PathState` parameter map.
pub fn required_route_param<T>(params: &BTreeMap<String, String>, name: &str) -> Result<T, RouteParamError>
where
    T: FromRouteParam,
{
    let value = params.get(name).ok_or_else(|| RouteParamError::Missing { name: name.to_owned() })?;
    T::from_route_param(value).map_err(|err| err.with_name(name))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum AppRoute {
        Home,
        User { id: u64 },
        Search { query: String },
    }

    impl Routable for AppRoute {
        fn to_url(&self) -> String {
            match self {
                Self::Home => "/".to_owned(),
                Self::User { id } => format!("/users/{}", encode_route_param(id)),
                Self::Search { query } => format!("/search/{}", encode_route_param(query)),
            }
        }

        fn from_url(url: &str) -> Option<Self> {
            let url = crate::url::Url::parse(url).ok()?;
            let path = url.path();
            let segments = path
                .trim_matches('/')
                .split('/')
                .filter(|segment| !segment.is_empty())
                .map(decode_route_param)
                .collect::<Vec<_>>();
            match &segments[..] {
                [] => Some(Self::Home),
                [prefix, id] if prefix == "users" => Some(Self::User {
                    id: parse_route_param(id).ok()?,
                }),
                [prefix, query] if prefix == "search" => Some(Self::Search { query: query.clone() }),
                _ => None,
            }
        }
    }

    struct RecordingAviator {
        last_url: RefCell<Option<String>>,
    }

    impl RecordingAviator {
        fn new() -> Self {
            Self {
                last_url: RefCell::new(None),
            }
        }
    }

    impl Aviator for RecordingAviator {
        fn goto(&self, url: &str) -> Result<(), NavigationError> {
            *self.last_url.borrow_mut() = Some(url.to_owned());
            Ok(())
        }
    }

    #[test]
    fn typed_route_round_trips_path_params() {
        let route = AppRoute::User { id: 42 };
        assert_eq!(route.to_url(), "/users/42");
        assert_eq!(AppRoute::from_url("/users/42"), Some(route));
        assert_eq!(AppRoute::from_url("/users/not-a-number"), None);
    }

    #[test]
    fn typed_route_encodes_single_path_segment() {
        let route = AppRoute::Search {
            query: "hello/world".to_owned(),
        };
        assert_eq!(route.to_url(), "/search/hello%2Fworld");
        assert_eq!(AppRoute::from_url("/search/hello%2Fworld"), Some(route));
    }

    #[test]
    fn aviator_ext_navigates_with_typed_route() {
        let aviator = RecordingAviator::new();
        aviator.goto_route(&AppRoute::User { id: 7 }).unwrap();
        assert_eq!(aviator.last_url.borrow().as_deref(), Some("/users/7"));
    }

    #[test]
    fn required_route_param_reports_missing_and_invalid_values() {
        let mut params = BTreeMap::new();
        params.insert("id".to_owned(), "12".to_owned());

        assert_eq!(required_route_param::<u64>(&params, "id").unwrap(), 12);
        assert!(matches!(
            required_route_param::<u64>(&params, "missing"),
            Err(RouteParamError::Missing { .. })
        ));

        params.insert("id".to_owned(), "abc".to_owned());
        assert!(matches!(
            required_route_param::<u64>(&params, "id"),
            Err(RouteParamError::Invalid { name: Some(_), .. })
        ));
    }
}
