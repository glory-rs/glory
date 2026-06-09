use std::sync::Arc;

use camino::Utf8PathBuf;
use tokio::process::Command;

use crate::compile::FRONT_TARGET_DIR;
use crate::config::{Config, Project};
use crate::ext::anyhow::{Context, Result, bail};
use crate::ext::fs;
use crate::logger::GRAY;

/// Default server build target dir (mirrors `BinPackage::resolve`, which puts
/// the server binary under `<target>/server` unless `bin-target-dir` is set).
const SERVER_TARGET_DIR: &str = "target/server";

pub async fn clean_all(conf: &Config, cargo: bool) -> Result<()> {
    for proj in &conf.projects {
        clean_proj(proj).await?;
    }

    if cargo {
        cargo_clean().await?;
    }
    Ok(())
}

/// Remove the front/server target dirs and the site root for a project.
pub async fn clean_proj(proj: &Arc<Project>) -> Result<()> {
    let mut dirs: Vec<Utf8PathBuf> = vec![
        Utf8PathBuf::from(FRONT_TARGET_DIR),
        proj.bin
            .target_dir
            .clone()
            .map(Utf8PathBuf::from)
            .unwrap_or_else(|| Utf8PathBuf::from(SERVER_TARGET_DIR)),
        proj.site.root_dir.clone(),
    ];
    dirs.dedup();

    for dir in dirs {
        if dir.exists() {
            log::info!("Glory cleaning {}", GRAY.paint(dir.as_str()));
            fs::remove_dir_all(&dir).await.dot()?;
        }
    }
    Ok(())
}

async fn cargo_clean() -> Result<()> {
    log::info!("Glory running {}", GRAY.paint("cargo clean"));
    let status = Command::new("cargo").arg("clean").status().await.dot()?;
    if !status.success() {
        bail!("`cargo clean` failed");
    }
    Ok(())
}
