use crate::compile::{front_cargo_process, server_cargo_process};
use crate::config::{Config, Project};
use crate::ext::anyhow::{Context, Result, anyhow};
use crate::logger::GRAY;

pub async fn check_all(conf: &Config) -> Result<()> {
    let mut first_failed_project = None;

    for proj in &conf.projects {
        if !check_proj(proj).await? && first_failed_project.is_none() {
            first_failed_project = Some(proj);
        }
    }

    if let Some(proj) = first_failed_project {
        Err(anyhow!("Check failed for {}", proj.name))
    } else {
        Ok(())
    }
}

/// Type-check the client (wasm lib) and the server (bin) without producing
/// artifacts. Returns true when both checks succeed.
pub async fn check_proj(proj: &Project) -> Result<bool> {
    let mut front_ok = true;
    if proj.builds_front() {
        let (envs, line, mut proc) = front_cargo_process("check", true, proj).dot()?;
        let status = proc.wait().await.dot()?;
        log::debug!("Cargo envs: {}", GRAY.paint(envs));
        log::info!("Cargo front check finished {}", GRAY.paint(line));
        front_ok = status.success();
    }

    let mut server_ok = true;
    if proj.builds_server() {
        let (envs, line, mut proc) = server_cargo_process("check", proj).dot()?;
        let status = proc.wait().await.dot()?;
        log::debug!("Cargo envs: {}", GRAY.paint(envs));
        log::info!("Cargo server check finished {}", GRAY.paint(line));
        server_ok = status.success();
    }

    Ok(front_ok && server_ok)
}
