//! Explicit file capabilities. Display names are never reopened as paths.
use glory_render::{FileConfig, FileRejection};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};
pub type Reply<T> = Pin<Box<dyn Future<Output = Result<T, FileRejection>> + Send + 'static>>;

/// Constructors must return promptly. OS work starts only when polled; foreign
/// completions wake futures and never enter UI/business state. Dropping a future
/// withdraws delivery, not a native operation's actual admission.
pub trait FileReader: Send + Sync + 'static {
    fn read(&self, offset: u64, length: u32) -> Reply<Vec<u8>>;
}
pub struct SelectedFile {
    pub name: String,
    pub media_type: String,
    pub size: u64,
    pub reader: Arc<dyn FileReader>,
}
/// Opens only paths delivered by an accepted native file drop. Hosts validate
/// the finite URI/path list before invoking this driver, outside UI borrows.
/// As with readers, dropping the receipt must not refund an active OS permit.
pub trait FileDrop: Send + Sync + 'static {
    fn open(&self, paths: Vec<std::path::PathBuf>) -> Reply<Vec<SelectedFile>>;
}
pub trait FilePicker: Send + Sync + 'static {
    fn pick(&self, configuration: FileConfig) -> Reply<Option<Vec<SelectedFile>>>;
}
#[derive(Clone)]
pub struct Files {
    pub(crate) driver: Arc<dyn FilePicker>,
    pub(crate) dropper: Option<Arc<dyn FileDrop>>,
    pub(crate) shared: Arc<Shared>,
    pub(crate) timeout: Duration,
}
impl Files {
    pub fn new(driver: Arc<dyn FilePicker>) -> Self {
        Self {
            driver,
            dropper: None,
            shared: Arc::new(Shared::default()),
            timeout: Duration::from_secs(120),
        }
    }
    pub fn with_timeout(
        driver: Arc<dyn FilePicker>,
        timeout: Duration,
    ) -> Result<Self, FileRejection> {
        if timeout.is_zero() || timeout > Duration::from_secs(600) {
            return Err(FileRejection::Failed);
        }
        let mut files = Self::new(driver);
        files.timeout = timeout;
        Ok(files)
    }
    /// Explicitly connect native drop grants to the application's opener.
    pub fn with_drop_driver(mut self, driver: Arc<dyn FileDrop>) -> Self {
        self.dropper = Some(driver);
        self
    }
    pub fn stats(&self) -> FileStats {
        self.shared.0.lock().expect("file budget").stats
    }
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub fn desktop() -> Result<Self, FileRejection> {
        let driver = Arc::new(desktop::Desktop::new()?);
        Ok(Self::new(driver.clone()).with_drop_driver(driver))
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileStats {
    pub picker: bool,
    /// Current host-owned native drop receipt/opener slot. Raw platform
    /// allocation and the driver's actual OS permits are separate budgets.
    pub dropping: bool,
    pub reads: usize,
    pub references: usize,
    pub metadata_bytes: usize,
    /// Charged declared file sizes, not allocated memory.
    pub selected_bytes: u64,
    /// Encoded preview bodies, including pending and cancelled decoder leases.
    /// Decoder allocations are bounded separately by document resource limits.
    pub preview_bytes: usize,
}
#[derive(Default)]
pub(crate) struct Shared(Mutex<Budget>);
#[derive(Default)]
struct Budget {
    stats: FileStats,
}
pub(crate) struct Permit {
    shared: Arc<Shared>,
    kind: Operation,
}
enum Operation {
    Picker,
    Read,
    #[cfg(all(
        feature = "shell",
        any(target_os = "windows", target_os = "linux", target_os = "macos")
    ))]
    Drop,
}
impl Shared {
    pub(crate) fn preview(self: &Arc<Self>, bytes: usize) -> Result<PreviewCharge, FileRejection> {
        let mut budget = self.0.lock().expect("file budget");
        if bytes > 32 * 1024 * 1024 - budget.stats.preview_bytes {
            return Err(FileRejection::Busy);
        }
        budget.stats.preview_bytes += bytes;
        Ok(PreviewCharge {
            shared: self.clone(),
            bytes,
        })
    }
    #[cfg(all(
        feature = "shell",
        any(target_os = "windows", target_os = "linux", target_os = "macos")
    ))]
    pub(crate) fn drop_permit(self: &Arc<Self>) -> Result<Permit, FileRejection> {
        let mut budget = self.0.lock().expect("file budget");
        if budget.stats.dropping {
            return Err(FileRejection::Busy);
        }
        budget.stats.dropping = true;
        Ok(Permit {
            shared: self.clone(),
            kind: Operation::Drop,
        })
    }
    pub(crate) fn permit(self: &Arc<Self>, picker: bool) -> Result<Permit, FileRejection> {
        let mut budget = self.0.lock().expect("file budget");
        if picker {
            if budget.stats.picker {
                return Err(FileRejection::Busy);
            }
            budget.stats.picker = true;
        } else {
            if budget.stats.reads >= glory_render::MAX_FILE_READS {
                return Err(FileRejection::Busy);
            }
            budget.stats.reads += 1;
        }
        Ok(Permit {
            shared: self.clone(),
            kind: if picker {
                Operation::Picker
            } else {
                Operation::Read
            },
        })
    }
    pub(crate) fn select(
        self: &Arc<Self>,
        previous: Option<&mut Lease>,
        cost: Cost,
    ) -> Result<Lease, FileRejection> {
        let old = previous.as_ref().map_or(Cost::default(), |p| p.cost);
        let mut budget = self.0.lock().expect("file budget");
        let stats = &mut budget.stats;
        let references = stats.references - old.references + cost.references;
        let metadata = stats.metadata_bytes - old.metadata + cost.metadata;
        let bytes = stats
            .selected_bytes
            .checked_sub(old.bytes)
            .and_then(|v| v.checked_add(cost.bytes))
            .ok_or(FileRejection::Bytes)?;
        if references > 128 || metadata > 128 * 1024 || bytes > glory_render::MAX_FILE_BYTES {
            return Err(FileRejection::Busy);
        }
        stats.references = references;
        stats.metadata_bytes = metadata;
        stats.selected_bytes = bytes;
        if let Some(old) = previous {
            old.cost = Cost::default();
        }
        Ok(Lease {
            shared: self.clone(),
            cost,
        })
    }
}
pub(crate) struct PreviewCharge {
    shared: Arc<Shared>,
    bytes: usize,
}
impl Drop for PreviewCharge {
    fn drop(&mut self) {
        self.shared
            .0
            .lock()
            .expect("file budget")
            .stats
            .preview_bytes -= self.bytes;
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut budget = self.shared.0.lock().expect("file budget");
        match self.kind {
            Operation::Picker => budget.stats.picker = false,
            #[cfg(all(
                feature = "shell",
                any(target_os = "windows", target_os = "linux", target_os = "macos")
            ))]
            Operation::Drop => budget.stats.dropping = false,
            Operation::Read => budget.stats.reads -= 1,
        }
    }
}
#[derive(Clone, Copy, Default)]
pub(crate) struct Cost {
    pub references: usize,
    pub metadata: usize,
    pub bytes: u64,
}
pub(crate) struct Lease {
    shared: Arc<Shared>,
    cost: Cost,
}
impl Drop for Lease {
    fn drop(&mut self) {
        let mut budget = self.shared.0.lock().expect("file budget");
        budget.stats.references -= self.cost.references;
        budget.stats.metadata_bytes -= self.cost.metadata;
        budget.stats.selected_bytes -= self.cost.bytes;
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
mod desktop;
pub(crate) mod preview;
mod worker;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_replacement_is_transactional_and_preview_bodies_share_one_finite_budget() {
        let shared = Arc::new(Shared::default());
        let mut first = shared
            .select(
                None,
                Cost {
                    references: 128,
                    metadata: 128 * 1024,
                    bytes: glory_render::MAX_FILE_BYTES,
                },
            )
            .unwrap();
        assert!(
            shared
                .select(
                    None,
                    Cost {
                        references: 1,
                        metadata: 1,
                        bytes: 1
                    }
                )
                .is_err()
        );
        assert!(
            shared
                .select(
                    Some(&mut first),
                    Cost {
                        references: 129,
                        metadata: 1,
                        bytes: 1
                    }
                )
                .is_err()
        );
        assert_eq!(shared.0.lock().unwrap().stats.references, 128);
        let second = shared
            .select(
                Some(&mut first),
                Cost {
                    references: 1,
                    metadata: 3,
                    bytes: 1,
                },
            )
            .unwrap();
        drop(first);
        assert_eq!(shared.0.lock().unwrap().stats.references, 1);
        let mut previews = (0..4)
            .map(|_| shared.preview(8 * 1024 * 1024).unwrap())
            .collect::<Vec<_>>();
        assert!(shared.preview(1).is_err());
        previews.pop();
        let last = shared.preview(8 * 1024 * 1024).unwrap();
        assert!(shared.preview(1).is_err());
        drop((second, previews, last));
        assert_eq!(shared.0.lock().unwrap().stats, FileStats::default());
    }
}
