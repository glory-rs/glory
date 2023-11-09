use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

pub use ::url::ParseError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Url(url::Url);
impl Url {
    /// Parse an absolute URL from a string.
    #[inline]
    pub fn parse(input: &str) -> Result<Url, ParseError> {
        url::Url::parse(input).map(Url)
    }

    /// Return the serialization of this URL.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0.as_str()
    }

    /// Return the origin of this URL (<https://url.spec.whatwg.org/#origin>)
    #[inline]
    pub fn origin(&self) -> String {
        self.0.origin().unicode_serialization()
    }

    /// Return the scheme of this URL, lower-cased, as an ASCII string without the ':' delimiter.
    #[inline]
    pub fn scheme(&self) -> String {
        self.0.scheme().into()
    }

    /// Return the username for this URL (typically the empty string)
    /// as a percent-encoded ASCII string.
    pub fn username(&self) -> String {
        self.0.username().into()
    }

    /// Return the password for this URL, if any, as a percent-encoded ASCII string.
    pub fn password(&self) -> Option<String> {
        self.0.password().map(|v| v.into())
    }

    /// Return the authority of this URL as an ASCII string.
    pub fn authority(&self) -> String {
        self.0.authority().into()
    }

    /// Return the string representation of the host (domain or IP address) for this URL, if any.
    ///
    /// Non-ASCII domains are punycode-encoded per IDNA if this is the host
    /// of a special URL, or percent encoded for non-special URLs.
    /// IPv6 addresses are given between `[` and `]` brackets.
    pub fn host(&self) -> Option<String> {
        self.0.host_str().map(|v| v.into())
    }

    /// Return the port number for this URL, if any.
    ///
    /// Note that default port numbers are never reflected by the serialization,
    /// use the `port_or_known_default()` method if you want a default port number returned.
    #[inline]
    pub fn port(&self) -> Option<u16> {
        self.0.port()
    }

    /// Return the path for this URL, as a percent-encoded ASCII string.
    /// For cannot-be-a-base URLs, this is an arbitrary string that doesn’t start with '/'.
    /// For other URLs, this starts with a '/' slash
    /// and continues with slash-separated path segments.
    pub fn path(&self) -> String {
        self.0.path().into()
    }

    /// Return this URL’s query string, if any, as a percent-encoded ASCII string.
    pub fn query(&self) -> Option<String> {
        self.0.query().map(|v| v.into())
    }

    /// Return this URL’s fragment identifier, if any.
    ///
    /// A fragment is the part of the URL after the `#` symbol.
    /// The fragment is optional and, if present, contains a fragment identifier
    /// that identifies a secondary resource, such as a section heading
    /// of a document.
    ///
    /// In HTML, the fragment identifier is usually the id attribute of a an element
    /// that is scrolled to on load. Browsers typically will not send the fragment portion
    /// of a URL to the server.
    ///
    /// **Note:** the parser did *not* percent-encode this component,
    /// but the input may have been percent-encoded already.
    pub fn fragment(&self) -> Option<String> {
        self.0.fragment().map(|v| v.into())
    }

    /// Change this URL’s fragment identifier.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("https://example.com/data.csv")?;
    /// assert_eq!(url.as_str(), "https://example.com/data.csv");

    /// url.set_fragment(Some("cell=4,1-6,2"));
    /// assert_eq!(url.as_str(), "https://example.com/data.csv#cell=4,1-6,2");
    /// assert_eq!(url.fragment(), Some("cell=4,1-6,2"));
    ///
    /// url.set_fragment(None);
    /// assert_eq!(url.as_str(), "https://example.com/data.csv");
    /// assert!(url.fragment().is_none());
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn set_fragment(&mut self, fragment: Option<&str>) {
        self.0.set_fragment(fragment)
    }

    /// Change this URL’s query string.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use url::Url;
    /// # use url::ParseError;
    ///
    /// # fn run() -> Result<(), ParseError> {
    /// let mut url = Url::parse("https://example.com/products")?;
    /// assert_eq!(url.as_str(), "https://example.com/products");
    ///
    /// url.set_query(Some("page=2"));
    /// assert_eq!(url.as_str(), "https://example.com/products?page=2");
    /// assert_eq!(url.query(), Some("page=2"));
    /// # Ok(())
    /// # }
    /// # run().unwrap();
    /// ```
    pub fn set_query(&mut self, query: Option<&str>) {
        self.0.set_query(query)
    }

    /// Change this URL’s path.
    pub fn set_path(&mut self, path: &str) {
        self.0.set_path(path)
    }

    /// Change this URL’s port number.
    ///
    /// Note that default port numbers are not reflected in the serialization.
    ///
    /// If this URL is cannot-be-a-base, does not have a host, or has the `file` scheme;
    /// do nothing and return `Err`.
    #[allow(clippy::result_unit_err)]
    pub fn set_port(&mut self, port: Option<u16>) -> Result<(), ()> {
        self.0.set_port(port)
    }

    /// Change this URL’s host.
    ///
    /// Removing the host (calling this with `None`)
    /// will also remove any username, password, and port number.
    pub fn set_host(&mut self, host: Option<&str>) -> Result<(), ParseError> {
        self.0.set_host(host)
    }

    /// Change this URL’s password.
    ///
    /// If this URL is cannot-be-a-base or does not have a host, do nothing and return `Err`.
    #[allow(clippy::result_unit_err)]
    pub fn set_password(&mut self, password: Option<&str>) -> Result<(), ()> {
        self.0.set_password(password)
    }

    /// Change this URL’s username.
    ///
    /// If this URL is cannot-be-a-base or does not have a host, do nothing and return `Err`.
    #[allow(clippy::result_unit_err)]
    pub fn set_username(&mut self, username: &str) -> Result<(), ()> {
        self.0.set_username(username)
    }

    /// Change this URL’s scheme.
    ///
    /// Do nothing and return `Err` under the following circumstances:
    ///
    /// * If the new scheme is not in `[a-zA-Z][a-zA-Z0-9+.-]+`
    /// * If this URL is cannot-be-a-base and the new scheme is one of
    ///   `http`, `https`, `ws`, `wss` or `ftp`
    /// * If either the old or new scheme is `http`, `https`, `ws`,
    ///   `wss` or `ftp` and the other is not one of these
    /// * If the new scheme is `file` and this URL includes credentials
    ///   or has a non-null port
    /// * If this URL's scheme is `file` and its host is empty or null
    ///
    /// See also [the URL specification's section on legal scheme state
    /// overrides](https://url.spec.whatwg.org/#scheme-state).
    #[allow(clippy::result_unit_err, clippy::suspicious_operation_groupings)]
    pub fn set_scheme(&mut self, scheme: &str) -> Result<(), ()> {
        self.0.set_scheme(scheme)
    }
}

/// Parse a string as an URL, without a base URL or encoding override.
impl FromStr for Url {
    type Err = ParseError;

    #[inline]
    fn from_str(input: &str) -> Result<Url, ParseError> {
        Url::parse(input)
    }
}

impl<'a> TryFrom<&'a str> for Url {
    type Error = ParseError;

    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        Url::parse(s)
    }
}

/// String conversion.
impl From<Url> for String {
    fn from(value: Url) -> String {
        value.0.into()
    }
}

impl Display for Url {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
