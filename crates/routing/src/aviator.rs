use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationError {
    Parse(crate::url::ParseError),
    NotEnabled(&'static str),
    BrowserLocationUnavailable,
}

impl fmt::Display for NavigationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "navigation URL parse failed: {err}"),
            Self::NotEnabled(message) => write!(f, "navigation backend is not enabled: {message}"),
            Self::BrowserLocationUnavailable => write!(f, "browser location is unavailable"),
        }
    }
}

impl std::error::Error for NavigationError {}

impl From<crate::url::ParseError> for NavigationError {
    fn from(value: crate::url::ParseError) -> Self {
        Self::Parse(value)
    }
}

pub trait Aviator {
    fn goto(&self, url: &str) -> Result<(), NavigationError>;
}
