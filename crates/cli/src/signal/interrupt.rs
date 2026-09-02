use std::sync::PoisonError;

use once_cell::sync::Lazy;
use tokio::{
    signal,
    sync::{RwLock, broadcast},
    task::JoinHandle,
};

use crate::compile::{Change, ChangeSet};

static ANY_INTERRUPT: Lazy<broadcast::Sender<()>> = Lazy::new(|| broadcast::channel(10).0);
static SHUTDOWN: Lazy<broadcast::Sender<()>> = Lazy::new(|| broadcast::channel(1).0);

static SHUTDOWN_REQUESTED: Lazy<RwLock<bool>> = Lazy::new(|| RwLock::new(false));

/// Deliberately a **std** lock, not `tokio::sync::RwLock`: the guard is never
/// held across an `.await`, and the senders below are called both from the
/// notify watcher's own thread and from inside the Tokio runtime (the serve
/// keyboard-controls task). `tokio::sync::RwLock::blocking_write` panics when
/// called on a runtime worker thread, which used to make `r` + Enter kill the
/// controls task on the first press.
static SOURCE_CHANGES: Lazy<std::sync::RwLock<ChangeSet>> = Lazy::new(|| std::sync::RwLock::new(ChangeSet::default()));

pub struct Interrupt {}

impl Interrupt {
    pub async fn is_shutdown_requested() -> bool {
        *SHUTDOWN_REQUESTED.read().await
    }

    pub fn subscribe_any() -> broadcast::Receiver<()> {
        ANY_INTERRUPT.subscribe()
    }

    pub fn subscribe_shutdown() -> broadcast::Receiver<()> {
        SHUTDOWN.subscribe()
    }

    pub fn get_source_changes() -> ChangeSet {
        SOURCE_CHANGES.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Drop the changes a build has consumed.
    ///
    /// `consumed_revision` is the [`ChangeSet::revision`] of the snapshot the
    /// build actually ran on. If the live revision has moved on, an edit landed
    /// while the build was running: the set is left intact so the next loop
    /// iteration rebuilds with it. Returns whether the set was cleared.
    pub fn clear_source_changes(consumed_revision: u64) -> bool {
        let mut ch = SOURCE_CHANGES.write().unwrap_or_else(PoisonError::into_inner);
        if ch.revision() != consumed_revision {
            log::trace!(
                "Interrupt source changes kept: revision moved {consumed_revision} -> {} during the build",
                ch.revision()
            );
            return false;
        }
        ch.clear();
        log::trace!("Interrupt source changed cleared");
        true
    }

    pub fn send_all_changed() {
        let mut ch = SOURCE_CHANGES.write().unwrap_or_else(PoisonError::into_inner);
        ch.add_all();
        drop(ch);
        Self::send_any()
    }

    pub fn send(changes: &[Change]) {
        let mut ch = SOURCE_CHANGES.write().unwrap_or_else(PoisonError::into_inner);
        let mut did_change = false;
        for change in changes {
            did_change |= ch.add(change.clone());
        }
        drop(ch);

        if !did_change {
            log::trace!("Interrupt change already pending; signalling anyway so a running build restarts");
        }
        // Signal unconditionally: a repeat of an already-pending change still
        // has to interrupt an in-flight build, otherwise that edit is only
        // visible after the *next* unrelated save.
        Self::send_any();
    }

    fn send_any() {
        if let Err(e) = ANY_INTERRUPT.send(()) {
            log::error!("Interrupt error could not send due to: {e}");
        } else {
            log::trace!("Interrupt send done");
        }
    }

    pub async fn request_shutdown() {
        {
            *SHUTDOWN_REQUESTED.write().await = true;
        }
        _ = SHUTDOWN.send(());
        _ = ANY_INTERRUPT.send(());
    }

    pub fn run_ctrl_c_monitor() -> JoinHandle<()> {
        tokio::spawn(async move {
            signal::ctrl_c().await.expect("failed to listen for event");
            log::info!("Glory ctrl-c received");
            Interrupt::request_shutdown().await;
        })
    }
}
