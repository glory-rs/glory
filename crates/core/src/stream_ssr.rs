//! Streaming-SSR coordination shared between the server holder (the
//! orchestrator) and [`Suspense`](crate::widgets::Suspense), which tags its
//! rendered region so a resolved body can be streamed in as an out-of-order
//! patch.
//!
//! This API only exists for server-side streaming SSR (non-wasm, `web-ssr`,
//! multi-holder); the module is empty everywhere else, so non-server builds
//! never pay for it. `Suspense` reaches it only from inside matching
//! `#[cfg]` blocks.

#[cfg(all(not(feature = "single-app"), feature = "web-ssr", not(target_arch = "wasm32")))]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;

    use crate::scope::SuspenseBoundary;

    /// A Suspense region registered during a streaming mount.
    #[derive(Clone)]
    pub(crate) struct BoundaryRegistration {
        /// Stable id shared by the `<template data-glory-placeholder>` marker
        /// and its later `data-glory-placeholder-patch`, and stored on the
        /// wrapper node as `data-glory-suspense`.
        pub placeholder_id: String,
        /// SSR node id of the wrapper whose children are the boundary region.
        pub wrapper_id: u64,
        /// Boundary handle, used to read pending state at flush time.
        pub boundary: SuspenseBoundary,
    }

    thread_local! {
        static STREAMING: Cell<bool> = const { Cell::new(false) };
        static BOUNDARIES: RefCell<Vec<BoundaryRegistration>> = const { RefCell::new(Vec::new()) };
        static NEXT_ID: Cell<u64> = const { Cell::new(0) };
        /// Resolved `resource_hydratable_in` values (token → JSON) captured
        /// during a server render so the client can adopt them and skip the
        /// refetch. Armed for every server mount, not just streaming.
        static RESOURCE_DATA: RefCell<Option<BTreeMap<String, String>>> = const { RefCell::new(None) };
    }

    /// True while a streaming mount is in progress on this thread.
    pub(crate) fn is_streaming() -> bool {
        STREAMING.with(Cell::get)
    }

    /// Arms streaming mode: clears any prior registrations and resets the
    /// placeholder-id counter so ids are deterministic per render.
    pub(crate) fn begin() {
        STREAMING.with(|flag| flag.set(true));
        BOUNDARIES.with(|b| b.borrow_mut().clear());
        NEXT_ID.with(|n| n.set(0));
    }

    /// Disarms streaming mode and hands back the registered boundaries.
    pub(crate) fn finish() -> Vec<BoundaryRegistration> {
        STREAMING.with(|flag| flag.set(false));
        BOUNDARIES.with(|b| std::mem::take(&mut *b.borrow_mut()))
    }

    /// Allocates the next deterministic placeholder id for a boundary.
    pub(crate) fn next_placeholder_id() -> String {
        NEXT_ID.with(|n| {
            let value = n.get();
            n.set(value + 1);
            format!("gly-suspense-{value}")
        })
    }

    /// Records a boundary so the holder can stream its resolved body.
    pub(crate) fn register_boundary(registration: BoundaryRegistration) {
        BOUNDARIES.with(|b| b.borrow_mut().push(registration));
    }

    /// Arms resolved-resource-value capture for a server render. Cleared and
    /// reset so a previous (un-rendered) holder never leaks data into this one.
    pub(crate) fn arm_resource_capture() {
        RESOURCE_DATA.with(|data| *data.borrow_mut() = Some(BTreeMap::new()));
    }

    /// Records a resolved `resource_hydratable_in` value (already serialized to
    /// JSON) under its stable token. No-op when capture is not armed.
    pub(crate) fn record_resource_json(token: &str, json: String) {
        RESOURCE_DATA.with(|data| {
            if let Some(map) = data.borrow_mut().as_mut() {
                map.insert(token.to_owned(), json);
            }
        });
    }

    /// Disarms capture and returns the collected token → JSON map.
    pub(crate) fn take_resource_data() -> BTreeMap<String, String> {
        RESOURCE_DATA.with(|data| data.borrow_mut().take().unwrap_or_default())
    }
}

#[cfg(all(not(feature = "single-app"), feature = "web-ssr", not(target_arch = "wasm32")))]
pub(crate) use imp::*;
