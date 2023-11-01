use std::fmt::{self, Formatter, Display};use std::str::FromStr;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParseError {
    EmptyHost,
    IdnaError,
    InvalidPort,
    InvalidIpv4Address,
    InvalidIpv6Address,
    InvalidDomainCharacter,
    RelativeUrlWithoutBase,
    RelativeUrlWithCannotBeABaseBase,
    SetHostOnCannotBeABaseUrl,
    Overflow,
}
impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::EmptyHost => write!(f, "Empty host"),
            ParseError::IdnaError => write!(f, "Idna error"),
            ParseError::InvalidPort => write!(f, "Invalid port"),
            ParseError::InvalidIpv4Address => write!(f, "Invalid ipv4 address"),
            ParseError::InvalidIpv6Address => write!(f, "Invalid ipv6 address"),
            ParseError::InvalidDomainCharacter => write!(f, "Invalid domain character"),
            ParseError::RelativeUrlWithoutBase => write!(f, "Relative url without base"),
            ParseError::RelativeUrlWithCannotBeABaseBase => write!(f, "Relative url with cannot be a base base"),
            ParseError::SetHostOnCannotBeABaseUrl => write!(f, "Set host on cannot be a base url"),
            ParseError::Overflow => write!(f, "Overflow"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url(web_sys::Url);
impl Url {
    /// Parse an absolute URL from a string.
    #[inline]
    pub fn parse(input: &str) -> Result<Url, ParseError> {
        web_sys::Url::new(input)
            .map(Url)
            .map_err(|_| ParseError::RelativeUrlWithoutBase)
    }

    /// Return the serialization of this URL.
    #[inline]
    pub fn to_string(&self) -> String {
        self.0.as_string().unwrap_or_default()
    }

    /// Return the origin of this URL (<https://url.spec.whatwg.org/#origin>)
    #[inline]
    pub fn origin(&self) -> String {
        self.0.origin()
    }

    /// Return the scheme of this URL, lower-cased, as an ASCII string without the ':' delimiter.
    #[inline]
    pub fn scheme(&self) -> String {
        self.0.protocol()
    }

    /// Return the username for this URL (typically the empty string)
    /// as a percent-encoded ASCII string.
    pub fn username(&self) -> String {
        self.0.username()
    }

    /// Return the password for this URL, if any, as a percent-encoded ASCII string.
    pub fn password(&self) -> Option<String> {
       let password = self.0.password();
       if password.is_empty() {
           None
       } else {
           Some(password)
       }
    }

    /// Return the authority of this URL as an ASCII string.
    pub fn authority(&self) -> String {
        let username = self.username();
        let password = self.password();
        let host = self.host().unwrap_or_default();
        if username.is_empty() && password.is_none() {
            host
        } else if password.is_none() {
            format!("{}@{}", username, host)
        } else {
            format!("{}:{}@{}", username, password.unwrap(), host)
        }
    }

    /// Return the string representation of the host (domain or IP address) for this URL, if any.
    ///
    /// Non-ASCII domains are punycode-encoded per IDNA if this is the host
    /// of a special URL, or percent encoded for non-special URLs.
    /// IPv6 addresses are given between `[` and `]` brackets.
    pub fn host(&self) -> Option<String> {
        let host = self.0.host();
        if host.is_empty() {
            None
        } else {
            Some(host)
        }
    }

    /// Return the port number for this URL, if any.
    #[inline]
    pub fn port(&self) -> Option<u16> {
        self.0.port().parse().ok()
    }
    
    /// Return the path for this URL, as a percent-encoded ASCII string.
    /// For cannot-be-a-base URLs, this is an arbitrary string that doesn’t start with '/'.
    /// For other URLs, this starts with a '/' slash
    /// and continues with slash-separated path segments.
    pub fn path(&self) -> String {
        self.0.pathname()
    }

    /// Return this URL’s query string, if any, as a percent-encoded ASCII string.
    pub fn query(&self) -> Option<String> {
        let search = self.0.search();
        if search.is_empty() {
            None
        } else {
            Some(search.trim_start_matches("?").to_owned())
        }
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
        let hash = self.0.hash();
        if hash.is_empty() {
            None
        } else {
            Some(hash.trim_start_matches("#").to_owned())
        }
    }

    /// Change this URL’s fragment identifier.
    /// ```rust
    pub fn set_fragment(&mut self, fragment: Option<&str>) {
        let fragment = fragment.unwrap_or_default();
        if fragment.is_empty() {
            self.0.set_hash("");
        } else {
            self.0.set_hash(format!("#{}", fragment).as_str());
        }
    }

    /// Change this URL’s query string.
    pub fn set_query(&mut self, query:&str) {
        self.0.set_search(query)
    }

    /// Change this URL’s path.
    pub fn set_path(&mut self, path: &str) {
        self.0.set_pathname(path)
    }

    /// Change this URL’s port number.
    ///
    /// Note that default port numbers are not reflected in the serialization.
    ///
    /// If this URL is cannot-be-a-base, does not have a host, or has the `file` scheme;
    /// do nothing and return `Err`.
    #[allow(clippy::result_unit_err)]
    pub fn set_port(&mut self, port: u16) -> Result<(), ()> {
        self.0.set_port(port.to_string().as_str());
        Ok(())
    }

    /// Change this URL’s host.
    ///
    /// Removing the host (calling this with `None`)
    /// will also remove any username, password, and port number.
    pub fn set_host(&mut self, host: Option<&str>) -> Result<(), ParseError> {
        self.0.set_host(host.unwrap_or_default());
        Ok(())
    }

    /// Change this URL’s password.
    #[allow(clippy::result_unit_err)]
    pub fn set_password(&mut self, password: Option<&str>) -> Result<(), ()> {
        self.0.set_password(password.unwrap_or_default());
        Ok(())
    }

    /// Change this URL’s username.
    ///
    /// If this URL is cannot-be-a-base or does not have a host, do nothing and return `Err`.
    #[allow(clippy::result_unit_err)]
    pub fn set_username(&mut self, username: Option<&str>) -> Result<(), ()> {
        self.0.set_username(username.unwrap_or_default());
        Ok(())
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
        self.0.set_protocol(scheme);
        Ok(())
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
        value.0.to_string().into()
    }
}

impl Display for Url {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_string())
    }
}