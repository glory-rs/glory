//! Bounded blocking I/O shared by mounted files and Android APK assets.
use super::Bytes;
use std::io::Read;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Instant;
use tokio::sync::Semaphore;

struct Counts {
    active: AtomicUsize,
    peak: AtomicUsize,
}
pub(super) struct Blocking {
    slots: Arc<Semaphore>,
    counts: Arc<Counts>,
}
impl Blocking {
    pub fn new(workers: usize) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(workers)),
            counts: Arc::new(Counts {
                active: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
            }),
        }
    }
    pub fn active(&self) -> usize {
        self.counts.active.load(Ordering::Acquire)
    }
    pub fn peak(&self) -> usize {
        self.counts.peak.load(Ordering::Acquire)
    }
    pub fn close(&self) {
        self.slots.close();
    }
    pub async fn run<T: Send + 'static>(
        &self,
        operation: impl FnOnce(Arc<AtomicBool>) -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        let permit = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "asset blocking workers closed")?;
        let cancel = ReadCancel(Arc::new(AtomicBool::new(false)));
        let cancelled = cancel.0.clone();
        let counts = self.counts.clone();
        let result = tokio::task::spawn_blocking(move || {
            // The closure, not its cancellable JoinHandle, owns admission.
            // A stuck OS call therefore cannot create more blocking reads
            // through repeated scroll/cancel/timeout cycles.
            let _permit = permit;
            let _active = Active::new(counts);
            operation(cancelled)
        })
        .await
        .map_err(|_| "asset blocking worker")?;
        drop(cancel);
        result
    }
}

struct ReadCancel(Arc<AtomicBool>);
impl Drop for ReadCancel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
struct Active(Arc<Counts>);
impl Active {
    fn new(counts: Arc<Counts>) -> Self {
        let active = counts.active.fetch_add(1, Ordering::AcqRel) + 1;
        counts.peak.fetch_max(active, Ordering::AcqRel);
        Self(counts)
    }
}
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::AcqRel);
    }
}
pub(super) fn read_bytes(
    mut source: impl Read,
    length: u64,
    limit: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Bytes, String> {
    let check = || {
        if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
            Err("asset read cancelled/deadline".to_owned())
        } else {
            Ok(())
        }
    };
    check()?;
    if length > limit as u64 {
        return Err("asset body budget".into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length as usize)
        .map_err(|_| "asset body allocation")?;
    let mut chunk = [0u8; 64 * 1024];
    loop {
        check()?;
        // Read one extra byte after the limit to detect files that grew after
        // metadata. Never allocate that byte into the bounded result buffer.
        let wanted = chunk
            .len()
            .min(limit.saturating_sub(bytes.len()).saturating_add(1));
        let read = match source.read(&mut chunk[..wanted]) {
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err("asset read".into()),
        };
        if read == 0 {
            check()?;
            return Ok(bytes.into());
        }
        if read > limit.saturating_sub(bytes.len()) {
            return Err("asset body budget".into());
        }
        let needed = bytes.len() + read;
        if needed > bytes.capacity() {
            let capacity = needed.max(bytes.capacity().saturating_mul(2)).min(limit);
            bytes
                .try_reserve_exact(capacity - bytes.len())
                .map_err(|_| "asset body allocation")?;
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn streaming_reads_enforce_declared_and_actual_sizes_before_returning_bytes() {
        let deadline = Instant::now() + Duration::from_secs(5);
        let cancel = AtomicBool::new(false);
        let data = vec![7u8; 140_000];
        assert_eq!(
            read_bytes(&data[..], data.len() as u64, data.len(), deadline, &cancel).unwrap(),
            data[..]
        );
        // Metadata may be stale. The extra byte detects growth without being
        // appended to the result; an empty body with a zero budget is valid.
        assert!(
            read_bytes(&data[..], 1, 100_000, deadline, &cancel)
                .unwrap_err()
                .contains("budget")
        );
        assert!(read_bytes(&b"x"[..], 0, 0, deadline, &cancel).is_err());
        assert!(
            read_bytes(&b""[..], 0, 0, deadline, &cancel)
                .unwrap()
                .is_empty()
        );
        struct MustNotRead;
        impl Read for MustNotRead {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                panic!("read after admission refusal")
            }
        }
        assert!(read_bytes(MustNotRead, 11, 10, deadline, &cancel).is_err());
        assert!(read_bytes(MustNotRead, 0, 10, Instant::now(), &cancel).is_err());
        cancel.store(true, Ordering::Release);
        assert!(read_bytes(MustNotRead, 0, 10, deadline, &cancel).is_err());
    }

    #[test]
    fn cancellation_after_an_os_read_prevents_more_reads_and_publication() {
        struct CancelAfterRead<'a> {
            cancel: &'a AtomicBool,
            reads: usize,
        }
        impl Read for CancelAfterRead<'_> {
            fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
                assert_eq!(self.reads, 0, "must stop between chunks");
                self.reads += 1;
                bytes[0] = 7;
                self.cancel.store(true, Ordering::Release);
                Ok(1)
            }
        }
        let cancel = AtomicBool::new(false);
        assert!(
            read_bytes(
                CancelAfterRead {
                    cancel: &cancel,
                    reads: 0
                },
                1,
                10,
                Instant::now() + Duration::from_secs(5),
                &cancel
            )
            .unwrap_err()
            .contains("cancelled")
        );
    }

    #[test]
    fn cancelled_waiters_do_not_refund_a_still_running_blocking_read() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let files = Arc::new(Blocking::new(1));
            let (started_tx, started_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let worker = files.clone();
            let first = tokio::spawn(async move {
                worker
                    .run(move |cancel| {
                        let _ = started_tx.send(());
                        release_rx
                            .recv_timeout(Duration::from_secs(5))
                            .map_err(|_| "fixture gate")?;
                        assert!(cancel.load(Ordering::Acquire));
                        Ok(())
                    })
                    .await
            });
            started_rx.await.unwrap();
            first.abort();
            assert!(first.await.unwrap_err().is_cancelled());
            assert_eq!(files.active(), 1);
            let entered = Arc::new(AtomicUsize::new(0));
            for _ in 0..200 {
                let worker = files.clone();
                let entered = entered.clone();
                let task = tokio::spawn(async move {
                    worker
                        .run(move |_| Ok(entered.fetch_add(1, Ordering::AcqRel)))
                        .await
                });
                tokio::task::yield_now().await;
                task.abort();
                assert!(task.await.unwrap_err().is_cancelled());
            }
            assert_eq!(entered.load(Ordering::Acquire), 0);
            assert_eq!(files.peak(), 1);
            let worker = files.clone();
            let waiting = tokio::spawn(async move { worker.run(|_| Ok(())).await });
            tokio::task::yield_now().await;
            files.close();
            assert!(waiting.await.unwrap().unwrap_err().contains("closed"));
            release_tx.send(()).unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                while files.active() != 0 {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            })
            .await
            .unwrap();
        });
    }
}
