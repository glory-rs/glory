//! # glory-native — Blitz command-stream consumer (experimental)
//!
//! **Status: spike.** The `blitz` feature provides [`BlitzConsumer`]: it
//! applies Glory's serialized [`Command`](glory_core::renderer::Command)
//! batches to a [`blitz_dom::BaseDocument`] through `DocumentMutator` —
//! the same DOM that Blitz's vello renderer paints. This validates that
//! the command stream is a sufficient interface for a native non-webview
//! backend; windowing/painting (blitz-shell + vello) and the event return
//! path are future work, tracked in `_todos.md`.

pub use glory_core::renderer::{
    Command as NativeCommand, CommandNode as NativeNode, CommandRenderer as NativeRenderer, EventData as NativeEventPayload,
};

#[cfg(feature = "blitz")]
mod blitz_consumer;
#[cfg(feature = "blitz")]
pub use blitz_consumer::BlitzConsumer;
