//! Windowed list primitives. Scrolling only accesses the visible window and
//! overscan, never an iterator over the whole data set. This is distinct from
//! keyed `each`, which deliberately mounts its complete sequence.
//!
//! The public List supports fixed or measured rows, bounded height metadata,
//! keyed anchoring and segmented physical scrolling through any Render host.
//! At most one focused row stays mounted outside the window until blur. Data
//! edits relocate it through the optional indexed `locate` callback; without
//! that callback only its old index and the current window can be searched.

mod cache;
mod layout;
mod measure;
mod navigation;
mod scroll;
mod widget;
pub use widget::{List, ListStats, list};

pub use cache::{Cache, ReconcileStats, RowHost};
pub use layout::{Anchor, Heights, Layout, Window};
pub use scroll::{ScrollSpace, ScrollUpdate};

/// Keep logical pixel coordinates below the point where f64 spacing exceeds
/// useful subpixel precision. Physical surfaces have a much smaller host limit.
pub const MAX_EXTENT: f64 = 281_474_976_710_656.0;

/// Default maximum rows in one render window, including overscan. One focused
/// row outside the window may additionally remain mounted.
pub const DEFAULT_ROW_LIMIT: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidSize,
    OutOfBounds,
    Allocation,
    DuplicateKey,
    InvalidRange,
    InvalidLookup,
    InvalidAnchor,
    RowLimit { requested: usize, limit: usize },
}

fn check_rows(requested: usize, limit: usize) -> Result<(), Error> {
    if requested > limit {
        Err(Error::RowLimit { requested, limit })
    } else {
        Ok(())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
fn nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}
fn valid_extent(value: f64) -> bool {
    nonnegative(value) && value <= MAX_EXTENT
}
