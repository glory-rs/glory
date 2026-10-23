//! Directory-relative reads stay on bounded blocking workers. Never check a
//! canonical string and then reopen through ambient filesystem authority.
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Instant;

#[cfg(unix)]
use cap_fs_ext::OpenOptionsSyncExt;
use cap_std::fs::{Dir, OpenOptions};
use tokio::sync::Semaphore;

use super::{Bytes, Url};

struct Counts {
    active: AtomicUsize,
    peak: AtomicUsize,
}
pub(super) struct Files {
    roots: Mutex<BTreeMap<String, Arc<Dir>>>,
    slots: Arc<Semaphore>,
    counts: Arc<Counts>,
}
impl Files {
    pub fn new(mounts: &BTreeMap<String, PathBuf>, workers: usize) -> io::Result<Self> {
        let mut roots = BTreeMap::new();
        for (name, path) in mounts {
            if name.is_empty()
                || name.len() > 32
                || name == "app"
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                || name.starts_with('-')
                || name.ends_with('-')
                || !path.is_absolute()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "asset mount name/root",
                ));
            }
            roots.insert(
                name.clone(),
                Arc::new(Dir::open_ambient_dir(path, cap_std::ambient_authority())?),
            );
        }
        Ok(Self {
            roots: Mutex::new(roots),
            slots: Arc::new(Semaphore::new(workers)),
            counts: Arc::new(Counts {
                active: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
            }),
        })
    }
    pub fn active(&self) -> usize {
        self.counts.active.load(Ordering::Acquire)
    }
    pub fn peak(&self) -> usize {
        self.counts.peak.load(Ordering::Acquire)
    }
    pub fn close(&self) {
        self.slots.close();
        let roots = std::mem::take(&mut *self.roots.lock().expect("asset file roots"));
        // Active operations own their individual handle clones. Inspection of
        // a closed Loader must not keep every idle mounted directory locked.
        drop(roots);
    }
    pub async fn read(&self, url: &Url, limit: usize, deadline: Instant) -> Result<Bytes, String> {
        let root = self
            .roots
            .lock()
            .expect("asset file roots")
            .get(url.host_str().ok_or("asset mount host")?)
            .cloned()
            .ok_or("asset mount missing")?;
        let path = relative_path(url)?;
        self.blocking(move |cancelled| read_file(&root, &path, limit, deadline, &cancelled))
            .await
    }
    async fn blocking<T: Send + 'static>(
        &self,
        operation: impl FnOnce(Arc<AtomicBool>) -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        let permit = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "asset files closed")?;
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
        .map_err(|_| "asset file worker")?;
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
fn relative_path(url: &Url) -> Result<PathBuf, String> {
    let mut path = PathBuf::new();
    for raw in url
        .path()
        .strip_prefix('/')
        .ok_or("asset file path")?
        .split('/')
    {
        let part = percent_encoding::percent_decode_str(raw)
            .decode_utf8()
            .map_err(|_| "asset file path UTF-8")?;
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if part.is_empty()
            || matches!(part.as_ref(), "." | "..")
            || part.ends_with(['.', ' '])
            || part.chars().any(|c| {
                c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
            })
            || matches!(
                stem.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            )
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            return Err("asset file path component".into());
        }
        path.push(part.as_ref());
    }
    Ok(path)
}
fn read_file(
    root: &Dir,
    path: &Path,
    limit: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Bytes, String> {
    let check = || {
        if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
            Err("asset file cancelled/deadline".to_owned())
        } else {
            Ok(())
        }
    };
    check()?;
    // Non-regular entries (directories, devices, sockets/FIFOs) are not assets.
    // Check again on the actual opened handle to cover replacement races.
    if !root
        .metadata(path)
        .map_err(|_| "asset file metadata")?
        .is_file()
    {
        return Err("asset file is not regular".into());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.nonblock(true);
    let mut file = root
        .open_with(path, &options)
        .map_err(|_| "asset file open")?;
    let metadata = file.metadata().map_err(|_| "asset file metadata")?;
    if !metadata.is_file() {
        return Err("asset file is not regular".into());
    }
    if metadata.len() > limit as u64 {
        return Err("asset body budget".into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(metadata.len() as usize)
        .map_err(|_| "asset body allocation")?;
    let mut chunk = [0u8; 64 * 1024];
    loop {
        check()?;
        // Read one extra byte after the limit to detect files that grew after
        // metadata. Never allocate that byte into the bounded result buffer.
        let wanted = chunk
            .len()
            .min(limit.saturating_sub(bytes.len()).saturating_add(1));
        let read = file
            .read(&mut chunk[..wanted])
            .map_err(|_| "asset file read")?;
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
    fn cancelled_waiters_do_not_refund_a_still_running_blocking_read() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let files = Arc::new(Files::new(&BTreeMap::new(), 1).unwrap());
            let (started_tx, started_rx) = tokio::sync::oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let worker = files.clone();
            let first = tokio::spawn(async move {
                worker
                    .blocking(move |cancel| {
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
                        .blocking(move |_| Ok(entered.fetch_add(1, Ordering::AcqRel)))
                        .await
                });
                tokio::task::yield_now().await;
                task.abort();
                assert!(task.await.unwrap_err().is_cancelled());
            }
            assert_eq!(entered.load(Ordering::Acquire), 0);
            assert_eq!(files.peak(), 1);
            let worker = files.clone();
            let waiting = tokio::spawn(async move { worker.blocking(|_| Ok(())).await });
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
