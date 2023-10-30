use once_cell::sync::Lazy;
use tokio::{
    signal,
    sync::{broadcast, RwLock},
    task::JoinHandle,
};

use crate::compile::{Change, ChangeSet};

static ANY_INTERRUPT: Lazy<broadcast::Sender<()>> = Lazy::new(|| broadcast::channel(10).0);
static SHUTDOWN: Lazy<broadcast::Sender<()>> = Lazy::new(|| broadcast::channel(1).0);

static SHUTDOWN_REQUESTED: Lazy<RwLock<bool>> = Lazy::new(|| RwLock::new(false));
static SOURCE_CHANGES: Lazy<RwLock<ChangeSet>> = Lazy::new(|| RwLock::new(ChangeSet::default()));

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

    pub async fn get_source_changes() -> ChangeSet {
        SOURCE_CHANGES.read().await.clone()
    }

    pub async fn clear_source_changes() {
        let mut ch = SOURCE_CHANGES.write().await;
        ch.clear();
        log::trace!("Interrupt source changed cleared");
    }

    pub fn send_all_changed() {
        let mut ch = SOURCE_CHANGES.blocking_write();
        *ch = ChangeSet::all_changes();
        drop(ch);
        Self::send_any()
    }

    pub fn send(changes: &[Change]) {
        let mut ch = SOURCE_CHANGES.blocking_write();
        let mut did_change = false;
        for change in changes {
            did_change |= ch.add(change.clone());
        }
        drop(ch);

        if did_change {
            Self::send_any();
        } else {
            log::trace!("Interrupt no change");
        }
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
