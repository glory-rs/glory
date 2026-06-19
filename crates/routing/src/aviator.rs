use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationError {
    Parse(crate::url::ParseError),
    NotEnabled(&'static str),
    BrowserLocationUnavailable,
    BrowserHistoryUnavailable,
}

impl fmt::Display for NavigationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(err) => write!(f, "navigation URL parse failed: {err}"),
            Self::NotEnabled(message) => write!(f, "navigation backend is not enabled: {message}"),
            Self::BrowserLocationUnavailable => write!(f, "browser location is unavailable"),
            Self::BrowserHistoryUnavailable => write!(f, "browser history is unavailable"),
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

    fn back(&self) -> Result<bool, NavigationError> {
        Err(NavigationError::NotEnabled("back navigation is not supported by this aviator"))
    }

    fn forward(&self) -> Result<bool, NavigationError> {
        Err(NavigationError::NotEnabled("forward navigation is not supported by this aviator"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StaticAviator;

    impl Aviator for StaticAviator {
        fn goto(&self, _url: &str) -> Result<(), NavigationError> {
            Ok(())
        }
    }

    #[test]
    fn default_history_navigation_reports_not_enabled() {
        assert!(matches!(StaticAviator.back(), Err(NavigationError::NotEnabled(_))));
        assert!(matches!(StaticAviator.forward(), Err(NavigationError::NotEnabled(_))));
    }
}
