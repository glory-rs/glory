#[cfg(all(test, feature = "full_tests"))]
mod tests;

mod command;
pub mod compile;
pub mod config;
pub mod ext;
mod launch;
mod logger;
pub mod service;
pub mod signal;

pub use launch::Glory;

use crate::config::{Commands, Overrides};
use crate::ext::PathBufExt;
use crate::ext::anyhow::{Context, Result};
use crate::logger::GRAY;
use camino::Utf8PathBuf;
use config::{Cli, Config};
#[allow(unused_imports)]
use ext::fs;
use signal::Interrupt;
use std::env;

/// Run with arguments parsed from the standalone binary (no programmatic
/// overrides). Kept for the legacy `glory` binary and external callers.
pub async fn run(args: Cli) -> Result<()> {
    run_with(args, Overrides::default()).await
}

/// Run with parsed arguments plus programmatic [`Overrides`] from the
/// embeddable [`Glory`] builder.
pub async fn run_with(args: Cli, overrides: Overrides) -> Result<()> {
    use Commands::{Build, Bundle, Check, Clean, EndToEnd, Fmt, New, Serve, Test, Watch};

    let verbose = args.opts().map(|o| o.verbose).unwrap_or(0);
    logger::setup(verbose, &args.log);

    // Commands that don't need the project metadata loaded.
    match &args.command {
        New(new) => return new.run().await,
        Fmt(fmt) => return command::fmt(fmt.check, &fmt.args).await,
        _ => {}
    }

    let manifest_path = args
        .manifest_path
        .to_owned()
        .unwrap_or_else(|| Utf8PathBuf::from("Cargo.toml"))
        .resolve_home_dir()
        .context(format!("manifest_path: {:?}", &args.manifest_path))?;
    let mut cwd = Utf8PathBuf::from_path_buf(env::current_dir().unwrap()).unwrap();
    cwd.clean_windows_path();

    let opts = args.opts().unwrap();

    // "watch" means reload-enabled: the hot-reload dev server (`serve` unless
    // `--no-reload`) and the deprecated `watch` alias.
    let watch = match &args.command {
        Serve(serve) => !serve.no_reload,
        Watch(_) => true,
        _ => false,
    };
    let config = Config::load_with(opts, &cwd, &manifest_path, watch, &overrides).dot()?;
    env::set_current_dir(&config.working_dir).dot()?;
    log::debug!("Path working dir {}", GRAY.paint(config.working_dir.as_str()));

    let _monitor = Interrupt::run_ctrl_c_monitor();
    match args.command {
        New(_) | Fmt(_) => unreachable!("handled before metadata load"),
        Serve(serve) if serve.no_reload => command::serve(&config.current_project()?).await,
        Serve(_) | Watch(_) => command::watch(&config.current_project()?).await,
        Build(_) => command::build_all(&config).await,
        Bundle(_) => command::bundle_all(&config).await,
        Clean(clean) => command::clean_all(&config, clean.cargo).await,
        Check(_) => command::check_all(&config).await,
        Test(_) => command::test_all(&config).await,
        EndToEnd(_) => command::end2end_all(&config).await,
    }
}
