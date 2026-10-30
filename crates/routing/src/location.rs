use crate::{Error, Result};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

/// Ordered decoded query pairs. Duplicate keys and unknown fields are retained.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query(pub Vec<(String, String)>);
impl Query {
    pub fn values<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> {
        self.0
            .iter()
            .filter(move |(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
    pub fn optional<T: std::str::FromStr>(&self, name: &str) -> Result<Option<T>> {
        let mut values = self.values(name);
        let first = values.next();
        if values.next().is_some() {
            return Err(Error::BadParams(format!("duplicate query: {name}")));
        }
        first
            .map(|value| value.parse().map_err(|_| Error::BadParams(name.to_owned())))
            .transpose()
    }
    pub fn required<T: std::str::FromStr>(&self, name: &str) -> Result<T> {
        self.optional(name)?
            .ok_or_else(|| Error::BadParams(format!("missing query: {name}")))
    }
    pub fn repeated<T: std::str::FromStr>(&self, name: &str) -> Result<Vec<T>> {
        self.values(name)
            .map(|value| value.parse().map_err(|_| Error::BadParams(name.to_owned())))
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub url_bytes: usize,
    pub segments: usize,
    pub query_pairs: usize,
    pub routes: usize,
    pub depth: usize,
    pub match_steps: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            url_bytes: 16 * 1024,
            segments: 256,
            query_pairs: 256,
            routes: 4096,
            depth: 64,
            match_steps: 4096,
        }
    }
}

/// An application URL, without origin. Path is encoded; query/fragment are decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    path: String,
    query: Query,
    fragment: Option<String>,
}
pub(crate) fn decode(value: &str) -> Result<String> {
    let bytes = value.as_bytes();
    for index in 0..bytes.len() {
        if bytes[index] == b'%'
            && (index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit())
        {
            return Err(Error::BadUrl("invalid percent encoding".into()));
        }
    }
    let decoded = percent_decode_str(value)
        .decode_utf8()
        .map(|value| value.into_owned())
        .map_err(|_| Error::BadUrl("invalid UTF-8".into()))?;
    if decoded.chars().any(char::is_control) {
        return Err(Error::BadUrl("decoded control character".into()));
    }
    Ok(decoded)
}
pub(crate) fn encode(value: &str) -> String {
    const SEGMENT: &percent_encoding::AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~');
    utf8_percent_encode(value, SEGMENT).to_string()
}
impl Location {
    pub fn parse(value: &str) -> Result<Self> {
        Self::parse_with(value, Limits::default())
    }
    pub fn parse_with(value: &str, limits: Limits) -> Result<Self> {
        if value.len() > limits.url_bytes {
            return Err(Error::OverLimit("URL bytes"));
        }
        if !value.starts_with('/')
            || value.starts_with("//")
            || value.contains('\\')
            || value.chars().any(char::is_control)
        {
            return Err(Error::BadUrl("expected root-relative URL".into()));
        }
        decode(value)?;
        let url = url::Url::parse(&format!("https://glory.invalid{value}"))
            .map_err(|error| Error::BadUrl(error.to_string()))?;
        let path = url.path().to_owned();
        if path.contains("//") {
            return Err(Error::BadUrl("empty path segment".into()));
        }
        if path.split('/').skip(1).count() > limits.segments {
            return Err(Error::OverLimit("path segments"));
        }
        let pairs = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        if pairs.len() > limits.query_pairs {
            return Err(Error::OverLimit("query pairs"));
        }
        let location = Self {
            path,
            query: Query(pairs),
            fragment: url.fragment().map(decode).transpose()?,
        };
        if location.to_string().len() > limits.url_bytes {
            return Err(Error::OverLimit("URL bytes"));
        }
        Ok(location)
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn query(&self) -> &Query {
        &self.query
    }
    pub fn fragment(&self) -> Option<&str> {
        self.fragment.as_deref()
    }
    pub fn segments(&self) -> Result<Vec<String>> {
        let path = self.path.trim_start_matches('/').trim_end_matches('/');
        if path.is_empty() {
            return Ok(Vec::new());
        }
        path.split('/').map(decode).collect()
    }
    pub fn with_query(mut self, name: impl Into<String>, value: impl ToString) -> Self {
        self.query.0.push((name.into(), value.to_string()));
        self
    }
    pub fn with_fragment(mut self, value: impl Into<String>) -> Self {
        self.fragment = Some(value.into());
        self
    }
    pub fn without_fragment(mut self) -> Self {
        self.fragment = None;
        self
    }
    pub(crate) fn with_path(&self, path: String) -> Self {
        Self {
            path,
            ..self.clone()
        }
    }
}
impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.path)?;
        if !self.query.0.is_empty() {
            let query = url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(&self.query.0)
                .finish();
            write!(f, "?{query}")?;
        }
        if let Some(fragment) = &self.fragment {
            write!(f, "#{}", encode(fragment))?;
        }
        Ok(())
    }
}

/// External opens are deliberately separate from internal history mutations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Internal(Location),
    External(String),
}
impl From<Location> for Target {
    fn from(value: Location) -> Self {
        Self::Internal(value)
    }
}
