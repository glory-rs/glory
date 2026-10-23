//! Opt-in, explicitly named resource serialization. Create one Resources per
//! request/app; clones share that context, independent contexts share nothing.
//! No ambient registry, HTTP, HTML or browser dependencies.
mod registry;
mod snapshot;

pub use registry::Resources;
pub use snapshot::{Limits, Snapshot, VERSION};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Access(glory_core::AccessError),
    InvalidName,
    InvalidSchema,
    Duplicate(String),
    Schema {
        name: String,
        expected: String,
        found: String,
    },
    Version(u32),
    Json(String),
    Payload(&'static str),
    Limit(&'static str),
    WrongRuntime,
    Busy(String),
}
impl From<glory_core::AccessError> for Error {
    fn from(error: glory_core::AccessError) -> Self {
        Self::Access(error)
    }
}
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error.to_string())
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

fn names(name: &str, schema: &str) -> Result<(), Error> {
    if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err(Error::InvalidName);
    }
    if schema.is_empty() || schema.len() > 128 || schema.chars().any(char::is_control) {
        return Err(Error::InvalidSchema);
    }
    Ok(())
}
