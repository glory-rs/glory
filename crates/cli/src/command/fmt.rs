use tokio::process::Command;

use crate::ext::anyhow::{Context, Result, bail};
use crate::logger::GRAY;

/// Format the project sources. Glory uses a builder-pattern widget API (no rsx
/// macro), so this is a thin passthrough over `cargo fmt --all`.
pub async fn fmt(check: bool, extra: &[String]) -> Result<()> {
    let mut args = vec!["fmt".to_string(), "--all".to_string()];
    if check || !extra.is_empty() {
        args.push("--".to_string());
        if check {
            args.push("--check".to_string());
        }
        args.extend(extra.iter().cloned());
    }

    let line = format!("cargo {}", args.join(" "));
    log::info!("Glory running {}", GRAY.paint(line.as_str()));

    let status = Command::new("cargo").args(&args).status().await.dot()?;
    if !status.success() {
        bail!("`{line}` failed");
    }
    Ok(())
}
