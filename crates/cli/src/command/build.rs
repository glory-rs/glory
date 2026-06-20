use std::sync::Arc;
use std::time::Instant;

use crate::{
    compile,
    compile::{BuildProgress, BuildStage, ChangeSet, render_progress},
    config::{Config, Project},
    ext::{
        anyhow::{Context, Result, anyhow},
        fs,
    },
};

pub async fn build_all(conf: &Config) -> Result<()> {
    let mut first_failed_project = None;

    for proj in &conf.projects {
        if !build_proj(proj).await? && first_failed_project.is_none() {
            first_failed_project = Some(proj);
        }
    }

    if let Some(proj) = first_failed_project {
        Err(anyhow!("Failed to build {}", proj.name))
    } else {
        Ok(())
    }
}

/// Build the project. Returns true if the build was successful
pub async fn build_proj(proj: &Arc<Project>) -> Result<bool> {
    if proj.site.root_dir.exists() {
        fs::rm_dir_content(&proj.site.root_dir).await.dot()?;
    }
    let changes = ChangeSet::all_changes();
    let mut progress = BuildProgress::new();

    if proj.builds_front() {
        if !run_stage(&mut progress, BuildStage::Compiling, compile::front(proj, &changes).await).await? {
            return Ok(false);
        }
        // wasm-opt only runs on release builds inside `front`; reflect that.
        if proj.release {
            progress.finish(BuildStage::WasmOpt);
        }
    }
    if !run_stage(&mut progress, BuildStage::Assets, compile::assets(proj, &changes, true).await).await? {
        return Ok(false);
    }
    if !run_stage(&mut progress, BuildStage::Styling, compile::style(proj, &changes).await).await? {
        return Ok(false);
    }
    if proj.builds_server() && !run_stage(&mut progress, BuildStage::Server, compile::server(proj, &changes).await).await? {
        return Ok(false);
    }
    if proj.builds_mobile() && !run_stage(&mut progress, BuildStage::Mobile, compile::mobile(proj, &changes).await).await? {
        return Ok(false);
    }

    log::info!("Build progress: {}", render_progress(&progress));
    Ok(true)
}

/// Drive a single build stage through the [`BuildProgress`] model: mark it
/// running, await the compile task, then record done (with elapsed ms) or
/// failed. Returns `Ok(false)` when the stage did not succeed.
async fn run_stage<T>(
    progress: &mut BuildProgress,
    stage: BuildStage,
    task: tokio::task::JoinHandle<Result<crate::signal::Outcome<T>>>,
) -> Result<bool> {
    progress.start(stage);
    let started = Instant::now();
    let outcome = task.await??;
    if outcome.is_success() {
        progress.finish_with(stage, started.elapsed().as_millis() as u64);
        Ok(true)
    } else {
        progress.fail(stage, format!("{stage:?} did not succeed"));
        log::info!("Build progress: {}", render_progress(progress));
        Ok(false)
    }
}
