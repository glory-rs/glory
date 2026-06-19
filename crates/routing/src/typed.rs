use std::fmt;
use std::str::FromStr;
use std::{borrow::Cow, collections::BTreeMap};

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

/// Parsed query parameters for typed route helpers.
pub type RouteQuery = BTreeMap<String, Vec<String>>;

/// Parse a raw URL query string into a stable map of decoded values.
pub fn parse_route_query(query: Option<&str>) -> RouteQuery {
    let Some(query) = query else {
        return RouteQuery::new();
    };
    let query = query.trim_start_matches('?');
    form_urlencoded::parse(query.as_bytes()).fold(RouteQuery::new(), |mut out, (key, value)| {
        out.entry(key.into_owned()).or_default().push(value.into_owned());
        out
    })
}

/// Percent-encode key/value pairs for a route query string.
pub fn encode_route_query<I, K, V>(pairs: I) -> String
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: fmt::Display,
{
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(key.as_ref(), &value.to_string());
    }
    serializer.finish()
}

/// Append a single encoded query parameter to a URL string.
pub fn append_route_query_param(url: &mut String, key: &str, value: impl fmt::Display) {
    if url.contains('?') {
        if !url.ends_with('?') && !url.ends_with('&') {
            url.push('&');
        }
    } else {
        url.push('?');
    }
    url.push_str(&encode_route_query([(key, value)]));
}

/// Parse a full query map into a typed application query struct.
pub trait FromRouteQuery: Sized {
    fn from_route_query(query: &RouteQuery) -> Result<Self, RouteParamError>;
}

impl FromRouteQuery for RouteQuery {
    fn from_route_query(query: &RouteQuery) -> Result<Self, RouteParamError> {
        Ok(query.clone())
    }
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

/// Read and parse the first value for a required query parameter.
pub fn required_query_param<T>(query: &RouteQuery, name: &str) -> Result<T, RouteParamError>
where
    T: FromRouteParam,
{
    let value = first_query_value(query, name)?;
    T::from_route_param(value).map_err(|err| err.with_name(name))
}

/// Read and parse the first value for an optional query parameter.
pub fn optional_query_param<T>(query: &RouteQuery, name: &str) -> Result<Option<T>, RouteParamError>
where
    T: FromRouteParam,
{
    let Some(values) = query.get(name) else {
        return Ok(None);
    };
    let Some(value) = values.first() else {
        return Ok(None);
    };
    T::from_route_param(value).map(Some).map_err(|err| err.with_name(name))
}

/// Read and parse a query parameter, falling back to a caller-provided default.
pub fn query_param_or<T>(query: &RouteQuery, name: &str, default: T) -> Result<T, RouteParamError>
where
    T: FromRouteParam,
{
    optional_query_param(query, name).map(|value| value.unwrap_or(default))
}

/// Read and parse every value for a repeated query parameter.
pub fn repeated_query_param<T>(query: &RouteQuery, name: &str) -> Result<Vec<T>, RouteParamError>
where
    T: FromRouteParam,
{
    query
        .get(name)
        .map(|values| {
            values
                .iter()
                .map(|value| T::from_route_param(value).map_err(|err| err.with_name(name)))
                .collect()
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

fn first_query_value<'a>(query: &'a RouteQuery, name: &str) -> Result<&'a str, RouteParamError> {
    query
        .get(name)
        .and_then(|values| values.first())
        .map(String::as_str)
        .ok_or_else(|| RouteParamError::Missing { name: name.to_owned() })
}

/// Encode path segments for use in a catch-all route tail.
pub fn encode_catch_all<I, V>(segments: I) -> String
where
    I: IntoIterator<Item = V>,
    V: fmt::Display,
{
    segments
        .into_iter()
        .map(|segment| encode_route_param(segment))
        .collect::<Vec<_>>()
        .join("/")
}

/// Split and decode a catch-all path tail into individual route segments.
pub fn split_catch_all(value: &str) -> Vec<String> {
    value
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(decode_route_param)
        .collect()
}

/// Split a catch-all path tail and parse each segment as a concrete Rust type.
pub fn parse_catch_all<T>(value: &str) -> Result<Vec<T>, RouteParamError>
where
    T: FromRouteParam,
{
    split_catch_all(value).into_iter().map(|segment| T::from_route_param(&segment)).collect()
}

/// Borrow or allocate a query suffix suitable for appending after a path.
pub fn query_suffix(query: &str) -> Cow<'_, str> {
    if query.is_empty() {
        Cow::Borrowed("")
    } else if query.starts_with('?') {
        Cow::Borrowed(query)
    } else {
        Cow::Owned(format!("?{query}"))
    }
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
        Search { query: SearchQuery },
        Files { path: Vec<String> },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SearchQuery {
        q: String,
        page: u32,
        tags: Vec<String>,
    }

    impl FromRouteQuery for SearchQuery {
        fn from_route_query(query: &RouteQuery) -> Result<Self, RouteParamError> {
            Ok(Self {
                q: required_query_param(query, "q")?,
                page: query_param_or(query, "page", 1)?,
                tags: repeated_query_param(query, "tag")?,
            })
        }
    }

    impl Routable for AppRoute {
        fn to_url(&self) -> String {
            match self {
                Self::Home => "/".to_owned(),
                Self::User { id } => format!("/users/{}", encode_route_param(id)),
                Self::Search { query } => {
                    let mut url = "/search".to_owned();
                    append_route_query_param(&mut url, "q", &query.q);
                    append_route_query_param(&mut url, "page", query.page);
                    for tag in &query.tags {
                        append_route_query_param(&mut url, "tag", tag);
                    }
                    url
                }
                Self::Files { path } => format!("/files/{}", encode_catch_all(path)),
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
                [prefix] if prefix == "search" => Some(Self::Search {
                    query: SearchQuery::from_route_query(&parse_route_query(url.query().as_deref())).ok()?,
                }),
                [prefix, rest @ ..] if prefix == "files" => Some(Self::Files { path: rest.to_vec() }),
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
        assert_eq!(encode_route_param("hello/world"), "hello%2Fworld");
        assert_eq!(decode_route_param("hello%2Fworld"), "hello/world");
    }

    #[test]
    fn typed_route_round_trips_query_params() {
        let route = AppRoute::Search {
            query: SearchQuery {
                q: "hello world".to_owned(),
                page: 2,
                tags: vec!["rust".to_owned(), "ui".to_owned()],
            },
        };
        assert_eq!(route.to_url(), "/search?q=hello+world&page=2&tag=rust&tag=ui");
        assert_eq!(AppRoute::from_url("/search?q=hello+world&page=2&tag=rust&tag=ui"), Some(route));
        assert_eq!(
            AppRoute::from_url("/search?q=hello+world"),
            Some(AppRoute::Search {
                query: SearchQuery {
                    q: "hello world".to_owned(),
                    page: 1,
                    tags: Vec::new(),
                },
            })
        );
        assert_eq!(AppRoute::from_url("/search?page=not-a-number&q=x"), None);
    }

    #[test]
    fn catch_all_helpers_round_trip_segments() {
        let path = vec!["docs".to_owned(), "hello/world".to_owned(), "42".to_owned()];
        let encoded = encode_catch_all(&path);
        assert_eq!(encoded, "docs/hello%2Fworld/42");
        assert_eq!(split_catch_all(&encoded), path);
        assert_eq!(parse_catch_all::<u32>("1/2/3").unwrap(), vec![1, 2, 3]);

        let route = AppRoute::Files { path };
        assert_eq!(route.to_url(), "/files/docs/hello%2Fworld/42");
        assert_eq!(AppRoute::from_url("/files/docs/hello%2Fworld/42"), Some(route));
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

    #[test]
    fn query_helpers_report_missing_and_invalid_values() {
        let query = parse_route_query(Some("?page=2&tag=rust&tag=ui&name=hello+world"));
        assert_eq!(required_query_param::<u32>(&query, "page").unwrap(), 2);
        assert_eq!(optional_query_param::<String>(&query, "name").unwrap(), Some("hello world".to_owned()));
        assert_eq!(optional_query_param::<u32>(&query, "missing").unwrap(), None);
        assert_eq!(query_param_or::<u32>(&query, "missing", 10).unwrap(), 10);
        assert_eq!(
            repeated_query_param::<String>(&query, "tag").unwrap(),
            vec!["rust".to_owned(), "ui".to_owned()]
        );
        assert_eq!(query_suffix("page=1").as_ref(), "?page=1");
        assert_eq!(query_suffix("?page=1").as_ref(), "?page=1");
        assert_eq!(query_suffix("").as_ref(), "");

        assert!(matches!(
            required_query_param::<u32>(&query, "missing"),
            Err(RouteParamError::Missing { .. })
        ));

        let invalid = parse_route_query(Some("page=nope"));
        assert!(matches!(
            required_query_param::<u32>(&invalid, "page"),
            Err(RouteParamError::Invalid { name: Some(_), .. })
        ));
    }
}
