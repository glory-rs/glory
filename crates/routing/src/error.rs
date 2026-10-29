use std::fmt;

pub type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Definition(String),
    BadUrl(String),
    BadParams(String),
    UnknownRoute(String),
    WrongParamType,
    WrongRuntime,
    OverLimit(&'static str),
    NoMatch,
    OutsideBase,
    Blocked,
    Denied(u16),
    RedirectLoop,
    History(String),
    Unsupported(&'static str),
    StaleNavigation,
    Busy,
    Gone,
    AsyncRequired,
    Superseded,
    HydrationMismatch,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<glory_core::AccessError> for Error {
    fn from(_: glory_core::AccessError) -> Self {
        Self::Gone
    }
}
