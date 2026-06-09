use std::sync::Arc;

use camino::Utf8PathBuf;

use crate::config::{BuildTarget, Config, Project};
use crate::ext::anyhow::{Context, Result, anyhow, bail};
use crate::ext::fs;
use crate::logger::GRAY;

/// Output folder collecting the distributable artifacts.
const DIST_DIR: &str = "dist";

pub async fn bundle_all(conf: &Config) -> Result<()> {
    for proj in &conf.projects {
        bundle_proj(proj).await?;
    }
    Ok(())
}

/// Build the project and collect the artifacts into `dist/<name>/`.
///
/// For the web target this gathers the generated site (JS/WASM/CSS/assets)
/// plus the server binary. Desktop/native bundling is not implemented yet.
pub async fn bundle_proj(proj: &Arc<Project>) -> Result<()> {
    if proj.target != BuildTarget::Web {
        bail!(
            "bundle for the {:?} target is not yet implemented; only the web target is supported",
            proj.target
        );
    }

    if !proj.release {
        log::warn!("Bundling a debug build. Pass --release for an optimized distributable.");
    }

    if !super::build::build_proj(proj).await.dot()? {
        bail!("Build failed; nothing to bundle");
    }

    let dist = Utf8PathBuf::from(DIST_DIR).join(&proj.name);
    if dist.exists() {
        fs::remove_dir_all(&dist).await.dot()?;
    }
    fs::create_dir_all(&dist).await.dot()?;

    // Static site (JS/WASM/CSS/assets).
    let public = dist.join("public");
    fs::copy_dir_all(&proj.site.root_dir, &public).await.dot()?;

    // Server binary.
    if proj.builds_server() && proj.bin.exe_file.exists() {
        let file_name = proj
            .bin
            .exe_file
            .file_name()
            .ok_or_else(|| anyhow!("server exe path has no file name: {}", proj.bin.exe_file))?;
        fs::copy(&proj.bin.exe_file, dist.join(file_name)).await.dot()?;
    }

    log::info!("Glory bundled {} into {}", proj.name, GRAY.paint(dist.as_str()));
    Ok(())
}
