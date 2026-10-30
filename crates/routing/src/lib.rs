//! Renderer-independent URL matching, owned navigation and nested Widget views.
//! Browser/OS events are explicit inputs; no global router or implicit I/O.
mod bootstrap;
mod definition;
mod error;
mod history;
#[cfg(feature = "widgets")]
mod link;
mod location;
pub mod native;
mod params;
mod request;
mod router;
#[cfg(feature = "widgets")]
mod view;

pub use bootstrap::Bootstrap;
pub use definition::{
    Match, MatchPlan, RouteDef, RouteId, RouteKey, Routes, RoutesBuilder, SlashPolicy,
};
pub use error::{Error, Result};
pub use history::{Entry, EntryKey, History, MemoryHistory, ScrollMemory, Traversal};
#[cfg(feature = "widgets")]
pub use link::{link, link_with};
pub use location::{Limits, Location, Query, Target};
pub use params::{Params, RouteParams};
pub use request::{RequestHistory, RequestLocation, RequestResolution};
pub use router::{
    Decision, GuardContext, GuardFuture, Navigation, NavigationKind, PreparedNavigation, Router,
    RouterHandle, Snapshot,
};
#[cfg(feature = "widgets")]
pub use view::{Outlet, RouteContext, RouteView, RouteViews};
