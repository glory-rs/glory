#[cfg(test)]
mod tests;

mod assets;
mod bin_package;
mod cli;
mod dotenvs;
mod end2end;
mod lib_package;
mod overrides;
mod profile;
mod project;
mod style;
mod tailwind;

use std::{fmt::Debug, sync::Arc};

pub use self::cli::{BuildTarget, Cli, Commands, Log, Opts};
pub use self::overrides::Overrides;
use crate::ext::{
    MetadataExt,
    anyhow::{Context, Result},
};
use anyhow::bail;
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::Metadata;
pub use profile::Profile;
pub use project::{Project, ProjectConfig};
pub use style::StyleConfig;
pub use tailwind::TailwindConfig;

pub struct Config {
    /// absolute path to the working dir
    pub working_dir: Utf8PathBuf,
    pub projects: Vec<Arc<Project>>,
    pub cli: Opts,
    pub watch: bool,
}

impl Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("projects", &self.projects)
            .field("cli", &self.cli)
            .field("watch", &self.watch)
            .finish_non_exhaustive()
    }
}

impl Config {
    pub fn load(cli: Opts, cwd: &Utf8Path, manifest_path: &Utf8Path, watch: bool) -> Result<Self> {
        Self::load_with(cli, cwd, manifest_path, watch, &Overrides::default())
    }

    /// Like [`Config::load`] but additionally overlays the programmatic
    /// [`Overrides`] supplied by the embeddable [`crate::Glory`] builder.
    pub fn load_with(cli: Opts, cwd: &Utf8Path, manifest_path: &Utf8Path, watch: bool, overrides: &Overrides) -> Result<Self> {
        let metadata = Metadata::load_cleaned(manifest_path)?;

        let mut projects = Project::resolve(&cli, cwd, &metadata, watch, overrides).dot()?;

        if projects.is_empty() {
            bail!(
                "Please define glory projects in the workspace Cargo.toml sections [[workspace.metadata.glory]], or supply bin_package()/lib_package() to the Glory builder."
            )
        }

        // A builder-supplied project name takes precedence over `--project`.
        if let Some(proj_name) = overrides.project.as_ref().or(cli.project.as_ref()) {
            if let Some(proj) = projects.iter().find(|p| p.name == *proj_name) {
                projects = vec![proj.clone()];
            } else {
                bail!(
                    r#"The specified project "{proj_name}" not found. Available projects: {}"#,
                    names(&projects)
                )
            }
        }

        Ok(Self {
            working_dir: metadata.workspace_root,
            projects,
            cli,
            watch,
        })
    }

    #[cfg(test)]
    pub fn test_load(cli: Opts, cwd: &str, manifest_path: &str, watch: bool) -> Self {
        use crate::ext::PathBufExt;

        let manifest_path = resolve_test_path(manifest_path);
        let mut cwd = resolve_test_path(cwd);
        cwd.clean_windows_path();
        Self::load(cli, &cwd, &manifest_path, watch).unwrap()
    }

    pub fn current_project(&self) -> Result<Arc<Project>> {
        if self.projects.len() == 1 {
            Ok(self.projects[0].clone())
        } else {
            bail!(
                "There are several projects available ({}). Please select one of them with the command line parameter --project",
                names(&self.projects)
            );
        }
    }
}

#[cfg(test)]
fn resolve_test_path(path: &str) -> Utf8PathBuf {
    let path = Utf8PathBuf::from(path);
    if path.is_absolute() {
        return path.canonicalize_utf8().unwrap();
    }
    if let Ok(path) = path.canonicalize_utf8() {
        return path;
    }

    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("glory-cli crate should live under crates/cli");
    workspace_root.join(path).canonicalize_utf8().unwrap()
}

fn names(projects: &[Arc<Project>]) -> String {
    projects.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(", ")
}
