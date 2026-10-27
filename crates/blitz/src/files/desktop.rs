//! Native chooser plus stable opened-file grants. The private worker owns
//! non-abortable OS work after the caller withdraws its result future.
use super::worker::Worker;
use super::*;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::sync::oneshot;
pub(super) struct Desktop {
    worker: Arc<Worker>,
    picker: Arc<AtomicBool>,
    reads: Arc<AtomicUsize>,
}
struct PickPermit(Arc<AtomicBool>);
impl Drop for PickPermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
struct ReadPermit(Arc<AtomicUsize>);
impl Drop for ReadPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
impl Desktop {
    pub fn new() -> Result<Self, FileRejection> {
        Ok(Self {
            worker: Arc::new(Worker::new()?),
            picker: Arc::new(AtomicBool::new(false)),
            reads: Arc::new(AtomicUsize::new(0)),
        })
    }
}
struct Opened {
    file: Arc<Mutex<File>>,
    worker: Arc<Worker>,
    reads: Arc<AtomicUsize>,
}
impl FileReader for Opened {
    fn read(&self, offset: u64, length: u32) -> Reply<Vec<u8>> {
        if length > glory_render::MAX_FILE_CHUNK_BYTES
            || offset
                .checked_add(u64::from(length))
                .is_none_or(|end| end > glory_render::MAX_FILE_BYTES)
        {
            return Box::pin(std::future::ready(Err(FileRejection::Bytes)));
        }
        let file = self.file.clone();
        let worker = self.worker.clone();
        let reads = self.reads.clone();
        Box::pin(async move {
            reads
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                    (v < 4).then_some(v + 1)
                })
                .map_err(|_| FileRejection::Busy)?;
            let permit = ReadPermit(reads);
            let (sender, receiver) = oneshot::channel();
            worker.send(Box::pin(async move {
                // The blocking closure, not its JoinHandle, owns admission.
                let result = tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    let mut file = file.lock().map_err(|_| FileRejection::Failed)?;
                    let mut bytes = vec![0; length as usize];
                    file.seek(SeekFrom::Start(offset))
                        .and_then(|_| file.read_exact(&mut bytes))
                        .map_err(|_| FileRejection::Failed)?;
                    Ok(bytes)
                })
                .await
                .unwrap_or(Err(FileRejection::Failed));
                let _ = sender.send(result);
            }))?;
            receiver.await.map_err(|_| FileRejection::Unavailable)?
        })
    }
}
fn open(
    path: PathBuf,
    worker: Arc<Worker>,
    reads: Arc<AtomicUsize>,
) -> Result<SelectedFile, FileRejection> {
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .filter(|v| !v.is_empty() && v.len() <= glory_render::MAX_FILE_NAME_BYTES)
        .ok_or(FileRejection::Metadata)?
        .to_owned();
    if !path
        .metadata()
        .map_err(|_| FileRejection::Metadata)?
        .is_file()
    {
        return Err(FileRejection::Bytes);
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(&path).map_err(|_| FileRejection::Failed)?;
    let metadata = file.metadata().map_err(|_| FileRejection::Metadata)?;
    if !metadata.is_file() || metadata.len() > glory_render::MAX_FILE_BYTES {
        return Err(FileRejection::Bytes);
    }
    let media_type = mime_guess::from_path(&path)
        .first_raw()
        .unwrap_or("")
        .to_owned();
    Ok(SelectedFile {
        name,
        media_type,
        size: metadata.len(),
        reader: Arc::new(Opened {
            file: Arc::new(Mutex::new(file)),
            worker,
            reads,
        }),
    })
}
impl FilePicker for Desktop {
    fn pick(&self, configuration: FileConfig) -> Reply<Option<Vec<SelectedFile>>> {
        let worker = self.worker.clone();
        let picker = self.picker.clone();
        let reads = self.reads.clone();
        Box::pin(async move {
            configuration
                .validate()
                .map_err(|_| FileRejection::Failed)?;
            if picker.swap(true, Ordering::AcqRel) {
                return Err(FileRejection::Busy);
            }
            let permit = PickPermit(picker);
            let (sender, receiver) = oneshot::channel();
            let opener = worker.clone();
            worker.send(Box::pin(async move {
                let _permit = permit;
                let mut dialog = rfd::AsyncFileDialog::new();
                // The declaration combines filters with OR. A wildcard or MIME
                // without known extensions must not hide its allowed files.
                let mut extensions = Vec::new();
                let mut broad = false;
                for filter in &configuration.filters {
                    match filter {
                        glory_render::FileFilter::Extension(extension) => {
                            extensions.push(&extension[1..])
                        }
                        glory_render::FileFilter::Mime(media) => {
                            match mime_guess::get_mime_extensions_str(media) {
                                Some(known) => extensions.extend_from_slice(known),
                                None => broad = true,
                            }
                        }
                    }
                }
                extensions.sort_unstable();
                extensions.dedup();
                if !broad && !extensions.is_empty() {
                    dialog = dialog.add_filter("Allowed files", &extensions);
                }
                let chosen = if configuration.multiple {
                    dialog.pick_files().await
                } else {
                    dialog.pick_file().await.map(|file| vec![file])
                };
                let result = match chosen {
                    None => Ok(None),
                    Some(files) if files.len() > configuration.max_files => {
                        Err(FileRejection::Count)
                    }
                    Some(files) => {
                        // Full paths exist only in this explicitly selected OS grant.
                        // Never derive one from FileInfo's display name.
                        let paths = files
                            .into_iter()
                            .map(|f| f.path().to_owned())
                            .collect::<Vec<_>>();
                        tokio::task::spawn_blocking(move || {
                            paths
                                .into_iter()
                                .map(|p| open(p, opener.clone(), reads.clone()))
                                .collect::<Result<Vec<_>, _>>()
                                .map(Some)
                        })
                        .await
                        .unwrap_or(Err(FileRejection::Failed))
                    }
                };
                let _ = sender.send(result);
            }))?;
            receiver.await.map_err(|_| FileRejection::Unavailable)?
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        task::{Context, Poll, Wake, Waker},
        time::{Duration, Instant},
    };
    struct Thread(std::thread::Thread);
    impl Wake for Thread {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    fn await_reply<T>(mut reply: Reply<T>) -> Result<T, FileRejection> {
        let waker = Waker::from(Arc::new(Thread(std::thread::current())));
        let mut cx = Context::from_waker(&waker);
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            if let Poll::Ready(result) = reply.as_mut().poll(&mut cx) {
                return result;
            }
            assert!(Instant::now() < end, "owned file worker did not complete");
            std::thread::park_timeout(Duration::from_millis(10));
        }
    }
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "glory-file-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn opened_file_identity_survives_path_replacement_and_short_reads_return_no_partial_body() {
        let dir = Directory::new();
        let original = dir.0.join("source-中文.txt");
        let moved = dir.0.join("moved.txt");
        std::fs::write(&original, b"original").unwrap();
        let driver = Desktop::new().unwrap();
        let selected = open(
            original.clone(),
            driver.worker.clone(),
            driver.reads.clone(),
        )
        .unwrap();
        assert_eq!(
            (
                selected.name.as_str(),
                selected.size,
                selected.media_type.as_str()
            ),
            ("source-中文.txt", 8, "text/plain")
        );
        std::fs::rename(&original, &moved).unwrap();
        std::fs::write(&original, b"replaced").unwrap();
        assert_eq!(
            await_reply(selected.reader.read(0, 8)).unwrap(),
            b"original"
        );
        assert_eq!(await_reply(selected.reader.read(3, 3)).unwrap(), b"gin");
        assert!(
            await_reply(
                selected
                    .reader
                    .read(0, glory_render::MAX_FILE_CHUNK_BYTES + 1)
            )
            .is_err()
        );
        assert_eq!(
            await_reply(selected.reader.read(8, 0)).unwrap(),
            Vec::<u8>::new()
        );
        std::fs::OpenOptions::new()
            .write(true)
            .open(&moved)
            .unwrap()
            .set_len(3)
            .unwrap();
        assert_eq!(
            await_reply(selected.reader.read(0, 8)),
            Err(FileRejection::Failed)
        );
        assert_eq!(await_reply(selected.reader.read(0, 3)).unwrap(), b"ori");
        assert_eq!(driver.reads.load(Ordering::Acquire), 0);
        assert!(open(dir.0.clone(), driver.worker.clone(), driver.reads.clone()).is_err());
    }
    #[test]
    fn dropping_receivers_cannot_refund_four_blocking_file_operations() {
        let dir = Directory::new();
        let path = dir.0.join("bytes.bin");
        std::fs::write(&path, [7u8]).unwrap();
        let driver = Desktop::new().unwrap();
        let reader = Opened {
            file: Arc::new(Mutex::new(File::open(path).unwrap())),
            worker: driver.worker.clone(),
            reads: driver.reads.clone(),
        };
        let locked = reader.file.lock().unwrap();
        let waker = Waker::from(Arc::new(Thread(std::thread::current())));
        let mut cx = Context::from_waker(&waker);
        for _ in 0..4 {
            let mut reply = reader.read(0, 1);
            assert!(reply.as_mut().poll(&mut cx).is_pending());
            drop(reply);
        }
        assert_eq!(driver.reads.load(Ordering::Acquire), 4);
        assert_eq!(await_reply(reader.read(0, 1)), Err(FileRejection::Busy));
        drop(locked);
        let end = Instant::now() + Duration::from_secs(10);
        while driver.reads.load(Ordering::Acquire) != 0 {
            assert!(Instant::now() < end);
            std::thread::park_timeout(Duration::from_millis(10));
        }
        assert_eq!(await_reply(reader.read(0, 1)).unwrap(), [7]);
    }
}
