//! Directory-relative reads stay on bounded blocking workers. Never check a
//! canonical string and then reopen through ambient filesystem authority.
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

#[cfg(unix)]
use cap_fs_ext::OpenOptionsSyncExt;
use cap_std::fs::{Dir, OpenOptions};

use super::{
    Bytes, Url,
    blocking::{Blocking, read_bytes},
};

pub(super) struct Files {
    roots: Mutex<BTreeMap<String, Arc<Dir>>>,
    blocking: Blocking,
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
            blocking: Blocking::new(workers),
        })
    }
    pub fn active(&self) -> usize {
        self.blocking.active()
    }
    pub fn peak(&self) -> usize {
        self.blocking.peak()
    }
    pub fn close(&self) {
        self.blocking.close();
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
        self.blocking
            .run(move |cancelled| read_file(&root, &path, limit, deadline, &cancelled))
            .await
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
    read_bytes(&mut file, metadata.len(), limit, deadline, cancel)
}
