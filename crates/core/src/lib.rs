//! The platform-independent, owned incremental runtime for Glory.
//!
//! Reactive values belong to a Runtime and an Owner. Reads track only their
//! immediate consumer; creating a component does not implicitly subscribe it.
//! Platform adapters supply execution services and consume semantic mutations.

mod arena;
/// Ready-only local tasks for native event-loop and streaming hosts. Futures
/// stay on their creating thread; wakers may be used from worker threads.
#[cfg(not(target_arch = "wasm32"))]
pub mod executor;
mod resource;
mod runtime;

pub use glory_render::RuntimeId;
pub use resource::{Resource, ResourceSeed, ResourceState};
pub use runtime::{
    AccessError, BindingId, Bond, Cage, Host, HostError, InstanceId, LocalFuture, Lotus, Owner,
    Runtime, RuntimeStats, Scope, Task, TaskToken, untracked,
};
