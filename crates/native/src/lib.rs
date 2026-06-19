//! # glory-native — Blitz command-stream consumer (experimental)
//!
//! **Status: spike.** The `blitz` feature provides [`BlitzConsumer`]: it
//! applies Glory's serialized [`Command`](glory_core::renderer::Command)
//! batches to a [`blitz_dom::BaseDocument`] through `DocumentMutator` —
//! the same DOM that Blitz's vello renderer paints. This validates that
//! the command stream is a sufficient interface for a native non-webview
//! backend. The `shell` feature adds a small Glory wrapper around
//! `blitz-shell` + vello lifecycle handling and routes Blitz click/input events
//! back into Glory's `CommandHolder`.

pub use glory_core::renderer::{
    Command as NativeCommand, CommandNode as NativeNode, CommandRenderer as NativeRenderer, EventData as NativeEventPayload,
};

#[cfg(feature = "blitz")]
mod blitz_consumer;
#[cfg(feature = "blitz")]
pub use blitz_consumer::BlitzConsumer;
#[cfg(feature = "shell")]
pub use blitz_consumer::{GloryBlitzApplication, GloryBlitzWindowConfig, GloryBlitzWindowId, launch_blitz_window, launch_blitz_window_with_config};
