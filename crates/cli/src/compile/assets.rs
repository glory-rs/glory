use std::sync::Arc;

use super::ChangeSet;
use crate::config::Project;
use crate::ext::anyhow::{Context, Result};
use crate::service::notify::Watched;
use crate::service::site::SourcedSiteFile;
use crate::signal::{Outcome, Product};
use crate::{ext::PathExt, fs, logger::GRAY};
use camino::{Utf8Path, Utf8PathBuf};
use tokio::task::JoinHandle;

pub async fn assets(proj: &Arc<Project>, changes: &ChangeSet, first_sync: bool) -> JoinHandle<Result<Outcome<Product>>> {
    let changes = changes.clone();

    let proj = proj.clone();
    tokio::spawn(async move {
        let Some(assets) = &proj.assets else {
            return Ok(Outcome::Success(Product::None));
        };
        let dest_root = &proj.site.root_dir;

        let change = if first_sync {
            log::trace!("Assets starting full resync");
            resync(&assets.dir, dest_root).await?;
            true
        } else {
            let mut changed = false;
            for watched in changes.asset_iter() {
                log::trace!("Assets processing {watched:?}");
                // A single asset event must never take down `glory serve`.
                // Editors save atomically (write temp -> rename -> remove
                // temp) and watchers replay directory events, so the mirrored
                // path is routinely already gone, or is a directory, by the
                // time we act on it. Log and carry on; a later rescan
                // reconciles the tree.
                match update_asset(&proj, watched.clone(), &assets.dir, dest_root, &[]).await {
                    Ok(change) => changed |= change,
                    Err(error) => log::warn!("Assets could not apply {watched:?}: {error:?}"),
                }
            }
            changed
        };
        if change {
            log::debug!("Assets finished (with changes)");
            Ok(Outcome::Success(Product::Assets))
        } else {
            log::debug!("Assets finished (no changes)");
            Ok(Outcome::Success(Product::None))
        }
    })
}

async fn update_asset(proj: &Project, watched: Watched, src_root: &Utf8Path, dest_root: &Utf8Path, reserved: &[Utf8PathBuf]) -> Result<bool> {
    if let Some(path) = watched.path()
        && reserved.contains(path)
    {
        log::warn!("Assets reserved filename for Glory. Please remove {path:?}");
        return Ok(false);
    }
    Ok(match watched {
        Watched::Create(f) => {
            let to = f.rebase(src_root, dest_root)?;
            if f.is_dir() {
                fs::copy_dir_all(f, to).await?;
            } else {
                fs::copy(&f, &to).await?;
            }
            true
        }
        Watched::Remove(f) => {
            let path = f.rebase(src_root, dest_root)?;
            if path.is_dir() {
                fs::remove_dir_all(&path).await.context(format!("remove dir recursively {path:?}"))?;
            } else if path.exists() {
                fs::remove_file(&path).await.context(format!("remove file {path:?}"))?;
            } else {
                // Never mirrored (e.g. an editor's temp file that was renamed
                // away before we saw it), so there is nothing to remove.
                log::trace!("Assets remove skipped, not mirrored: {path:?}");
            }
            false
        }
        Watched::Rename(from, to) => {
            let from = from.rebase(src_root, dest_root)?;
            let to = to.rebase(src_root, dest_root)?;
            fs::rename(&from, &to).await.context(format!("rename {from:?} to {to:?}"))?;
            true
        }
        Watched::Write(f) => {
            // Windows reports `Modify(Any)` for the watched directory itself;
            // reading it as a file fails with EISDIR.
            if f.is_dir() {
                log::trace!("Assets write skipped, path is a directory: {f:?}");
                return Ok(false);
            }
            if !f.exists() {
                log::trace!("Assets write skipped, source is gone: {f:?}");
                return Ok(false);
            }
            let file = SourcedSiteFile {
                source: f.clone(),
                dest: f.rebase(src_root, dest_root)?,
                site: f.unbase(src_root)?,
            };
            proj.site.updated(&file).await?
        }
        Watched::Rescan => {
            resync(src_root, dest_root).await?;
            true
        }
    })
}

pub fn reserved(src: &Utf8Path) -> Vec<Utf8PathBuf> {
    vec![src.join("index.html"), src.join("pkg")]
}

// pub async fn update(config: &Config) -> Result<()> {
//     if let Some(src) = &config.glory.assets_dir {
//         let dest = DEST.to_canoncial_dir().dot()?;
//         let src = src.to_canoncial_dir().dot()?;

//         resync(&src, &dest)
//             .await
//             .context(format!("Could not synchronize {src:?} with {dest:?}"))?;
//     }
//     Ok(())
// }

async fn resync(src: &Utf8Path, dest: &Utf8Path) -> Result<()> {
    clean_dest(dest).await.context(format!("Cleaning {dest:?}"))?;
    let reserved = reserved(src);
    mirror(src, dest, &reserved).await.context(format!("Mirroring {src:?} -> {dest:?}"))
}

async fn clean_dest(dest: &Utf8Path) -> Result<()> {
    let mut entries = fs::read_dir(dest).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if entry.file_type().await?.is_dir() {
            if entry.file_name() != "pkg" {
                log::debug!("Assets removing folder {}", GRAY.paint(path.to_string_lossy()));
                fs::remove_dir_all(path).await?;
            }
        } else if entry.file_name() != "index.html" {
            log::debug!("Assets removing file {}", GRAY.paint(path.to_string_lossy()));
            fs::remove_file(path).await?;
        }
    }
    Ok(())
}

async fn mirror(src_root: &Utf8Path, dest_root: &Utf8Path, reserved: &[Utf8PathBuf]) -> Result<()> {
    let mut entries = src_root.read_dir_utf8()?;
    while let Some(Ok(entry)) = entries.next() {
        let from = entry.path().to_path_buf();
        let to = from.rebase(src_root, dest_root)?;
        if reserved.contains(&from) {
            log::warn!("");
            continue;
        }

        if entry.file_type()?.is_dir() {
            log::debug!("Assets copy folder {} -> {}", GRAY.paint(from.as_str()), GRAY.paint(to.as_str()));
            fs::copy_dir_all(from, to).await?;
        } else {
            log::debug!("Assets copy file {} -> {}", GRAY.paint(from.as_str()), GRAY.paint(to.as_str()));
            fs::copy(from, to).await?;
        }
    }
    Ok(())
}
