//! A bounded queue of admitted I/O work. Active work is drained on shutdown;
//! OS calls own their physical permits independently of receiver futures.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
pub(super) type Work = Pin<Box<dyn Future<Output = ()> + Send>>;
#[derive(Default)]
pub(super) struct Failures {
    panicked: AtomicU64,
    cancelled: AtomicU64,
}
impl Failures {
    pub fn counts(&self) -> (u64, u64) {
        (
            self.panicked.load(Ordering::Relaxed),
            self.cancelled.load(Ordering::Relaxed),
        )
    }
    fn record(&self, result: Result<(), tokio::task::JoinError>) {
        if let Err(error) = result {
            let count = if error.is_panic() {
                &self.panicked
            } else if error.is_cancelled() {
                &self.cancelled
            } else {
                return;
            };
            let _ = count.try_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                Some(count.saturating_add(1))
            });
        }
    }
    pub fn blocking<T>(
        &self,
        result: Result<Result<T, FileRejection>, tokio::task::JoinError>,
    ) -> Result<T, FileRejection> {
        match result {
            Ok(result) => result,
            Err(error) => {
                self.record(Err(error));
                Err(FileRejection::Failed)
            }
        }
    }
}
pub(super) struct Worker {
    sender: mpsc::Sender<Work>,
    failures: Arc<Failures>,
}
impl Worker {
    pub fn new() -> Result<Self, FileRejection> {
        let (sender, mut receiver) = mpsc::channel::<Work>(8);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .map_err(|_| FileRejection::Unavailable)?;
        let failures = Arc::new(Failures::default());
        let outcomes = failures.clone();
        std::thread::Builder::new()
            .name("glory-files".into())
            .spawn(move || {
                runtime.block_on(async move {
                    let mut tasks = tokio::task::JoinSet::new();
                    loop {
                        tokio::select! {
                            work = receiver.recv(), if tasks.len() < 8 => match work {
                                Some(work) => { tasks.spawn(work); }
                                None => break,
                            },
                            result = tasks.join_next(), if !tasks.is_empty() => {
                                if let Some(result) = result { outcomes.record(result); }
                            }
                        }
                    }
                    while let Some(result) = tasks.join_next().await {
                        outcomes.record(result);
                    }
                })
            })
            .map_err(|_| FileRejection::Unavailable)?;
        Ok(Self { sender, failures })
    }
    pub fn failures(&self) -> Arc<Failures> {
        self.failures.clone()
    }
    pub fn send(&self, work: Work) -> Result<(), FileRejection> {
        self.sender.try_send(work).map_err(|_| FileRejection::Busy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reaped_join_failures_distinguish_panic_cancel_and_success() {
        let failures = Failures::default();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut tasks = tokio::task::JoinSet::new();
            tasks.spawn(async {
                panic!("owned file worker panic probe");
            });
            tasks.spawn(std::future::pending::<()>()).abort();
            tasks.spawn(async {});
            while let Some(result) = tasks.join_next().await {
                failures.record(result);
            }
        });
        assert_eq!(failures.counts(), (1, 1));
    }

    #[test]
    fn blocking_task_panics_are_counted_before_mapping_to_file_rejection() {
        let failures = Failures::default();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            let failed = tokio::task::spawn_blocking(|| -> Result<(), FileRejection> {
                panic!("owned blocking file worker panic probe");
            })
            .await;
            assert_eq!(failures.blocking(failed), Err(FileRejection::Failed));
            let succeeded = tokio::task::spawn_blocking(|| Ok::<_, FileRejection>(42)).await;
            assert_eq!(failures.blocking(succeeded), Ok(42));
        });
        assert_eq!(failures.counts(), (1, 0));
    }
}
