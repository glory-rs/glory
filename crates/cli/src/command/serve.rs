use std::{process::Command, sync::Arc};

use crate::config::Project;
use crate::ext::anyhow::{Context, Result};
use crate::service::serve;

pub async fn serve(proj: &Arc<Project>, should_open: bool) -> Result<()> {
    if !super::build::build_proj(proj).await.dot()? {
        return Ok(());
    }
    let server = serve::spawn(proj).await;
    open_site(proj, should_open);
    server.await??;
    Ok(())
}

pub(super) fn open_site(proj: &Project, should_open: bool) {
    if !should_open {
        return;
    }

    let url = proj.site.url();
    if let Err(error) = open_browser(&url) {
        log::warn!("Failed to open {url} in the default browser: {error}");
    }
}

fn open_browser(url: &str) -> Result<()> {
    let mut command = browser_command(url);
    command.spawn().with_context(|| format!("open browser for {url}"))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn browser_command(url: &str) -> Command {
    let mut command = Command::new("cmd");
    command.args(["/C", "start", "", url]);
    command
}

#[cfg(target_os = "macos")]
fn browser_command(url: &str) -> Command {
    let mut command = Command::new("open");
    command.arg(url);
    command
}

#[cfg(all(unix, not(target_os = "macos")))]
fn browser_command(url: &str) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(url);
    command
}
