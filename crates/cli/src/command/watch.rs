use std::sync::Arc;

use crate::{
    compile::{self},
    config::Project,
    ext::anyhow::Context,
    logger, service,
    signal::{Interrupt, Outcome, Product, ProductSet, ReloadSignal, ServerRestart},
};
use anyhow::Result;
use glory_hot_reload::HotReloadFunctions;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    task::JoinHandle,
    try_join,
};

use super::build::build_proj;

const CONTROLS_HELP: &str = "Serve controls: r + Enter rebuild, v + Enter verbose, / + Enter help, Ctrl-C stop";

pub async fn watch(proj: &Arc<Project>, should_open: bool) -> Result<()> {
    // even if the build fails, we continue
    build_proj(proj).await?;

    // but if ctrl-c is pressed, we stop
    if Interrupt::is_shutdown_requested().await {
        return Ok(());
    }

    let hot_reload_sources = if proj.hot_reload {
        if proj.builds_front() {
            let hot_functions = HotReloadFunctions::new();
            hot_functions.update_from_paths(&proj.lib.src_paths)?;
            Some(hot_functions)
        } else {
            None
        }
    } else {
        None
    };

    let _watch = service::notify::spawn(proj).await?;
    if let Some(hot_functions) = hot_reload_sources {
        let _patch = service::patch::spawn(proj, &hot_functions).await?;
    }

    service::serve::spawn(proj).await;
    service::reload::spawn(proj).await;
    super::serve::open_site(proj, should_open);
    let _controls = spawn_controls();

    let res = run_loop(proj).await;
    if res.is_err() {
        Interrupt::request_shutdown().await;
    }
    res
}

fn spawn_controls() -> JoinHandle<()> {
    log::info!("{CONTROLS_HELP}");
    tokio::spawn(async move {
        let mut shutdown = Interrupt::subscribe_shutdown();
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        loop {
            tokio::select! {
                _ = shutdown.recv() => break,
                line = lines.next_line() => {
                    match line {
                        Ok(Some(line)) => handle_control_line(&line),
                        Ok(None) => break,
                        Err(error) => {
                            log::warn!("Serve controls stopped reading stdin: {error}");
                            break;
                        }
                    }
                }
            }
        }
    })
}

fn handle_control_line(line: &str) {
    match ServeControl::parse(line) {
        ServeControl::Rebuild => {
            log::info!("Serve manual rebuild requested");
            Interrupt::send_all_changed();
        }
        ServeControl::ToggleVerbose => {
            let level = logger::toggle_verbose();
            log::info!("Serve log level set to {level}");
        }
        ServeControl::Help => log::info!("{CONTROLS_HELP}"),
        ServeControl::Ignore => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServeControl {
    Rebuild,
    ToggleVerbose,
    Help,
    Ignore,
}

impl ServeControl {
    fn parse(line: &str) -> Self {
        match line.trim().to_ascii_lowercase().as_str() {
            "r" | "rebuild" => Self::Rebuild,
            "v" | "verbose" => Self::ToggleVerbose,
            "/" | "?" | "h" | "help" => Self::Help,
            "" => Self::Ignore,
            _ => Self::Ignore,
        }
    }
}

pub async fn run_loop(proj: &Arc<Project>) -> Result<()> {
    let mut int = Interrupt::subscribe_any();
    loop {
        log::debug!("Watch waiting for changes");
        int.recv().await.dot()?;

        if Interrupt::is_shutdown_requested().await {
            log::debug!("Shutting down");
            return Ok(());
        }

        let changes = Interrupt::get_source_changes().await;

        // spawn separate style-update process
        tokio::spawn({
            let changes = changes.to_owned();
            let proj = Arc::clone(proj);
            async move {
                let style = compile::style(&proj, &changes).await;
                if let Ok(Ok(Outcome::Success(Product::Style(_)))) = style.await {
                    ReloadSignal::send_style();
                    log::info!("Watch updated style");
                    Interrupt::clear_source_changes().await;
                }
            }
        });

        let server_hdl = if proj.builds_server() {
            Some(compile::server(proj, &changes).await)
        } else {
            None
        };
        let front_hdl = if proj.builds_front() {
            Some(compile::front(proj, &changes).await)
        } else {
            None
        };
        let mobile_hdl = if proj.builds_mobile() {
            Some(compile::mobile(proj, &changes).await)
        } else {
            None
        };
        let assets_hdl = compile::assets(proj, &changes, false).await;

        let (serve, front, assets) = match (server_hdl, front_hdl) {
            (Some(server_hdl), Some(front_hdl)) => {
                let (serve, front, assets) = try_join!(server_hdl, front_hdl, assets_hdl)?;
                (serve?, front?, assets?)
            }
            (Some(server_hdl), None) => {
                let (serve, assets) = try_join!(server_hdl, assets_hdl)?;
                (serve?, Outcome::Success(Product::None), assets?)
            }
            (None, Some(front_hdl)) => {
                let (front, assets) = try_join!(front_hdl, assets_hdl)?;
                (Outcome::Success(Product::None), front?, assets?)
            }
            (None, None) => {
                let assets = assets_hdl.await?;
                (Outcome::Success(Product::None), Outcome::Success(Product::None), assets?)
            }
        };

        let mobile = match mobile_hdl {
            Some(mobile_hdl) => mobile_hdl.await??,
            None => Outcome::Success(Product::None),
        };

        let outcomes = vec![serve, front, assets, mobile];

        let failed = outcomes.contains(&Outcome::Failed);
        let interrupted = outcomes.contains(&Outcome::Stopped);

        if failed {
            log::warn!("Build failed");
            // Surface the failure in the browser as a dismissible overlay; the
            // full diagnostics remain in the terminal.
            ReloadSignal::send_build_error("Build failed. See the terminal running `glory serve` for the full compiler output.");
            Interrupt::clear_source_changes().await;
        } else if interrupted {
            log::info!("Build interrupted. Restarting.");
        } else {
            let set = ProductSet::from(outcomes);

            if set.is_empty() {
                log::trace!("Build step done with no changes");
            } else {
                log::trace!("Build step done with changes: {set}");
            }

            if set.only_style() {
                ReloadSignal::send_style();
                log::info!("Watch updated style")
            } else if set.contains(&Product::Server) {
                // send product change, then the server will send the reload once it has restarted
                ServerRestart::send();
                log::info!("Watch updated {set}. Server restarting")
            } else if set.contains(&Product::Mobile) {
                if set.contains_any(&[Product::Front, Product::Assets]) {
                    ReloadSignal::send_full();
                    log::info!(
                        "Watch rebuilt {set}. Connected mobile webviews reloaded static assets; reinstall/relaunch the app to load Rust code changes"
                    )
                } else {
                    log::info!("Watch rebuilt {set}. Reinstall/relaunch the mobile app to load Rust code changes")
                }
            } else if set.contains_any(&[Product::Front, Product::Assets]) {
                ReloadSignal::send_full();
                log::info!("Watch updated {set}")
            }
            Interrupt::clear_source_changes().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_serve_control_lines() {
        assert_eq!(ServeControl::parse("r"), ServeControl::Rebuild);
        assert_eq!(ServeControl::parse(" rebuild "), ServeControl::Rebuild);
        assert_eq!(ServeControl::parse("v"), ServeControl::ToggleVerbose);
        assert_eq!(ServeControl::parse("/"), ServeControl::Help);
        assert_eq!(ServeControl::parse("?"), ServeControl::Help);
        assert_eq!(ServeControl::parse(""), ServeControl::Ignore);
        assert_eq!(ServeControl::parse("unknown"), ServeControl::Ignore);
    }
}
