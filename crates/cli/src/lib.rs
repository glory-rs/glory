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
use once_cell::sync::Lazy;
use signal::Interrupt;
use std::{env, path::PathBuf};
use tokio::sync::Mutex;

static RUN_CWD_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

/// Run with arguments parsed from the standalone binary (no programmatic
/// overrides). Kept for the legacy `glory` binary and external callers.
pub async fn run(args: Cli) -> Result<()> {
    run_with(args, Overrides::default()).await
}

/// Run with parsed arguments plus programmatic [`Overrides`] from the
/// embeddable [`Glory`] builder.
pub async fn run_with(args: Cli, overrides: Overrides) -> Result<()> {
    use Commands::{Build, Bundle, Check, Clean, Completions, Config as ConfigCommand, Doctor, EndToEnd, Fmt, New, Run, SelfUpdate, Serve, Test};

    let verbose = args.opts().map(|o| o.verbose).unwrap_or(0);
    logger::setup(verbose, &args.log);

    let _cwd_lock = RUN_CWD_LOCK.lock().await;

    // Commands that don't need the project metadata loaded.
    match &args.command {
        New(new) => return new.run().await,
        Fmt(fmt) => return command::fmt(fmt.check, &fmt.args).await,
        Completions(opts) => return command::completions(opts.shell),
        SelfUpdate => return command::self_update(),
        Doctor(opts) => return command::doctor(opts).await,
        ConfigCommand(opts) if opts.schema => return command::config_schema().await,
        _ => {}
    }

    let mut cwd = Utf8PathBuf::from_path_buf(env::current_dir().unwrap()).unwrap();
    cwd.clean_windows_path();

    let mut manifest_path = args
        .manifest_path
        .to_owned()
        .unwrap_or_else(|| Utf8PathBuf::from("Cargo.toml"))
        .resolve_home_dir()
        .context(format!("manifest_path: {:?}", &args.manifest_path))?;
    if manifest_path.is_relative() {
        manifest_path = cwd.join(manifest_path);
    }
    manifest_path.clean_windows_path();

    let opts = args.opts().unwrap();
    let mut overrides = overrides;
    match &args.command {
        Serve(serve) => {
            overrides.site_address = serve.address;
            overrides.site_port = serve.port;
        }
        Run(run) => {
            overrides.site_address = run.address;
            overrides.site_port = run.port;
        }
        _ => {}
    }

    let watch = matches!(&args.command, Serve(serve) if !serve.no_reload);
    let config = Config::load_with(opts, &cwd, &manifest_path, watch, &overrides).dot()?;
    let _cwd_guard = CurrentDirGuard::enter(&config.working_dir)?;
    log::debug!("Path working dir {}", GRAY.paint(config.working_dir.as_str()));

    let _monitor = Interrupt::run_ctrl_c_monitor();
    match args.command {
        New(_) | Fmt(_) | Completions(_) | SelfUpdate | Doctor(_) => unreachable!("handled before metadata load"),
        Serve(serve) if serve.no_reload => command::serve(&config.current_project()?, serve.should_open()).await,
        Serve(serve) => command::watch(&config.current_project()?, serve.should_open()).await,
        Run(run) => command::serve(&config.current_project()?, run.should_open()).await,
        Build(_) => command::build_all(&config).await,
        Bundle(_) => command::bundle_all(&config).await,
        Clean(clean) => command::clean_all(&config, clean.cargo).await,
        Check(_) => command::check_all(&config).await,
        ConfigCommand(opts) => command::config_validate(&config, opts.json).await,
        Test(_) => command::test_all(&config).await,
        EndToEnd(_) => command::end2end_all(&config).await,
    }
}

struct CurrentDirGuard {
    original: PathBuf,
}

impl CurrentDirGuard {
    fn enter(path: &camino::Utf8Path) -> Result<Self> {
        let original = env::current_dir().dot()?;
        env::set_current_dir(path).dot()?;
        Ok(Self { original })
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        if let Err(error) = env::set_current_dir(&self.original) {
            log::error!("Failed to restore current working directory to {:?}: {error}", self.original);
        }
    }
}

#[cfg(test)]
mod cwd_tests {
    use super::*;
    use crate::config::{ConfigOpts, Opts};

    #[tokio::test]
    async fn run_restores_current_dir_after_project_command() {
        let before = env::current_dir().unwrap();
        let workspace_root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("glory-cli crate should live under crates/cli")
            .to_path_buf();

        run(Cli {
            manifest_path: Some(workspace_root.join("examples/project/Cargo.toml")),
            log: Vec::new(),
            command: Commands::Config(ConfigOpts {
                opts: Opts::default(),
                json: true,
                schema: false,
            }),
        })
        .await
        .unwrap();

        assert_eq!(env::current_dir().unwrap(), before);
    }
}
