use std::sync::Arc;

use crate::{
    compile::{self},
    config::Project,
    ext::anyhow::Context,
    service,
    signal::{Interrupt, Outcome, Product, ProductSet, ReloadSignal, ServerRestart},
};
use anyhow::Result;
use glory_hot_reload::{HotFunctions, ViewMacros};
use tokio::try_join;

use super::build::build_proj;

pub async fn watch(proj: &Arc<Project>) -> Result<()> {
    // even if the build fails, we continue
    build_proj(proj).await?;

    // but if ctrl-c is pressed, we stop
    if Interrupt::is_shutdown_requested().await {
        return Ok(());
    }

    let hot_reload_sources = if proj.hot_reload {
        // build initial set of view macros for patching
        if proj.builds_front() {
            let view_macros = ViewMacros::new();
            view_macros.update_from_paths(&proj.lib.src_paths)?;
            let hot_functions = HotFunctions::new();
            hot_functions.update_from_paths(&proj.lib.src_paths)?;
            Some((view_macros, hot_functions))
        } else {
            None
        }
    } else {
        None
    };

    let _watch = service::notify::spawn(proj).await?;
    if let Some((view_macros, hot_functions)) = hot_reload_sources {
        let _patch = service::patch::spawn(proj, &view_macros, &hot_functions).await?;
    }

    service::serve::spawn(proj).await;
    service::reload::spawn(proj).await;

    let res = run_loop(proj).await;
    if res.is_err() {
        Interrupt::request_shutdown().await;
    }
    res
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

        let outcomes = vec![serve, front, assets];

        let failed = outcomes.iter().any(|outcome| *outcome == Outcome::Failed);
        let interrupted = outcomes.iter().any(|outcome| *outcome == Outcome::Stopped);

        if failed {
            log::warn!("Build failed");
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
            } else if set.contains_any(&[Product::Front, Product::Assets]) {
                ReloadSignal::send_full();
                log::info!("Watch updated {set}")
            }
            Interrupt::clear_source_changes().await;
        }
    }
}
