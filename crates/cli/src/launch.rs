use std::env;
use std::net::SocketAddr;

use camino::Utf8PathBuf;
use clap::Parser;

use crate::config::{Cli, Overrides};
use crate::ext::anyhow::Result;

/// Embeddable Glory dev-tool launcher.
///
/// Instead of installing a global `glory` binary (which can drift from the
/// `glory` version your project depends on), add a tiny binary to your own
/// project that constructs a [`Glory`] and calls [`Glory::launch`]. Because it
/// is compiled against your project's pinned `glory-cli`, the tool and the
/// framework can never disagree on versions.
///
/// ```no_run
/// // src/bin/glory.rs (or an `xtask` member crate)
/// fn main() -> anyhow::Result<()> {
///     glory_cli::Glory::new()
///         .bin_package("my-app")
///         .lib_package("my-app")
///         .launch()
/// }
/// ```
///
/// Then drive it like the standalone CLI, e.g. `cargo run --bin glory -- serve`.
/// A `cargo glory ...` style alias can be added in `.cargo/config.toml`:
///
/// ```toml
/// [alias]
/// glory = "run --bin glory --"
/// ```
///
/// Builder setters mirror the `[package.metadata.glory]` keys. Anything left
/// unset falls back to the manifest metadata (or its default); anything set
/// here wins over the metadata.
#[derive(Debug, Default, Clone)]
pub struct Glory {
    overrides: Overrides,
    manifest_path: Option<Utf8PathBuf>,
}

impl Glory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Path to the `Cargo.toml` to load (defaults to `./Cargo.toml`).
    pub fn manifest_path(mut self, path: impl Into<Utf8PathBuf>) -> Self {
        self.manifest_path = Some(path.into());
        self
    }

    /// Select a single project by name in a multi-project workspace.
    pub fn project(mut self, name: impl Into<String>) -> Self {
        self.overrides.project = Some(name.into());
        self
    }

    // --- ProjectDefinition ------------------------------------------------

    pub fn name(mut self, value: impl Into<String>) -> Self {
        self.overrides.name = Some(value.into());
        self
    }
    pub fn bin_package(mut self, value: impl Into<String>) -> Self {
        self.overrides.bin_package = Some(value.into());
        self
    }
    pub fn lib_package(mut self, value: impl Into<String>) -> Self {
        self.overrides.lib_package = Some(value.into());
        self
    }

    // --- ProjectConfig scalars --------------------------------------------

    pub fn output_name(mut self, value: impl Into<String>) -> Self {
        self.overrides.output_name = Some(value.into());
        self
    }
    pub fn site_addr(mut self, value: SocketAddr) -> Self {
        self.overrides.site_addr = Some(value);
        self
    }
    pub fn site_root(mut self, value: impl Into<Utf8PathBuf>) -> Self {
        self.overrides.site_root = Some(value.into());
        self
    }
    pub fn site_pkg_dir(mut self, value: impl Into<Utf8PathBuf>) -> Self {
        self.overrides.site_pkg_dir = Some(value.into());
        self
    }
    pub fn reload_port(mut self, value: u16) -> Self {
        self.overrides.reload_port = Some(value);
        self
    }
    pub fn style_file(mut self, value: impl Into<Utf8PathBuf>) -> Self {
        self.overrides.style_file = Some(value.into());
        self
    }
    pub fn tailwind_input_file(mut self, value: impl Into<Utf8PathBuf>) -> Self {
        self.overrides.tailwind_input_file = Some(value.into());
        self
    }
    pub fn tailwind_config_file(mut self, value: impl Into<Utf8PathBuf>) -> Self {
        self.overrides.tailwind_config_file = Some(value.into());
        self
    }
    pub fn assets_dir(mut self, value: impl Into<Utf8PathBuf>) -> Self {
        self.overrides.assets_dir = Some(value.into());
        self
    }
    pub fn js_dir(mut self, value: impl Into<Utf8PathBuf>) -> Self {
        self.overrides.js_dir = Some(value.into());
        self
    }
    pub fn browser_query(mut self, value: impl Into<String>) -> Self {
        self.overrides.browser_query = Some(value.into());
        self
    }
    pub fn bin_target(mut self, value: impl Into<String>) -> Self {
        self.overrides.bin_target = Some(value.into());
        self
    }
    pub fn bin_target_triple(mut self, value: impl Into<String>) -> Self {
        self.overrides.bin_target_triple = Some(value.into());
        self
    }
    pub fn bin_target_dir(mut self, value: impl Into<String>) -> Self {
        self.overrides.bin_target_dir = Some(value.into());
        self
    }
    pub fn bin_cargo_command(mut self, value: impl Into<String>) -> Self {
        self.overrides.bin_cargo_command = Some(value.into());
        self
    }
    pub fn end2end_cmd(mut self, value: impl Into<String>) -> Self {
        self.overrides.end2end_cmd = Some(value.into());
        self
    }
    pub fn end2end_dir(mut self, value: impl Into<Utf8PathBuf>) -> Self {
        self.overrides.end2end_dir = Some(value.into());
        self
    }

    // --- ProjectConfig vecs / bools ---------------------------------------
    // `Some(vec![])` is an explicit empty override, distinct from `None`.

    pub fn features(mut self, value: Vec<String>) -> Self {
        self.overrides.features = Some(value);
        self
    }
    pub fn lib_features(mut self, value: Vec<String>) -> Self {
        self.overrides.lib_features = Some(value);
        self
    }
    pub fn bin_features(mut self, value: Vec<String>) -> Self {
        self.overrides.bin_features = Some(value);
        self
    }
    pub fn lib_default_features(mut self, value: bool) -> Self {
        self.overrides.lib_default_features = Some(value);
        self
    }
    pub fn bin_default_features(mut self, value: bool) -> Self {
        self.overrides.bin_default_features = Some(value);
        self
    }

    // --- Profiles ---------------------------------------------------------

    pub fn lib_profile_dev(mut self, value: impl Into<String>) -> Self {
        self.overrides.lib_profile_dev = Some(value.into());
        self
    }
    pub fn lib_profile_release(mut self, value: impl Into<String>) -> Self {
        self.overrides.lib_profile_release = Some(value.into());
        self
    }
    pub fn bin_profile_dev(mut self, value: impl Into<String>) -> Self {
        self.overrides.bin_profile_dev = Some(value.into());
        self
    }
    pub fn bin_profile_release(mut self, value: impl Into<String>) -> Self {
        self.overrides.bin_profile_release = Some(value.into());
        self
    }

    /// Set every override at once from a prepared [`Overrides`].
    pub fn overrides(mut self, overrides: Overrides) -> Self {
        self.overrides = overrides;
        self
    }

    // --- Entry points -----------------------------------------------------

    /// Parse CLI arguments from the process environment and run, building a
    /// dedicated Tokio runtime so downstream `fn main()` can stay synchronous.
    pub fn launch(self) -> Result<()> {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
        runtime.block_on(self.launch_async())
    }

    /// Like [`Glory::launch`] but for callers already inside a Tokio runtime.
    pub async fn launch_async(self) -> Result<()> {
        let cli = parse_argv();
        self.run(cli).await
    }

    /// Run against an explicitly provided [`Cli`], skipping argv parsing.
    /// Handy for tests and embedding in larger tools.
    pub async fn run(self, mut cli: Cli) -> Result<()> {
        if let Some(manifest_path) = self.manifest_path {
            cli.manifest_path = Some(manifest_path);
        }
        crate::run_with(cli, self.overrides).await
    }
}

/// Parse `Cli` from `env::args`, tolerating the leading `glory` token that
/// cargo inserts when invoked as the `cargo glory` subcommand / alias.
fn parse_argv() -> Cli {
    let mut args: Vec<String> = env::args().collect();
    if args.get(1).map(|a| a == "glory").unwrap_or(false) {
        args.remove(1);
    }
    Cli::parse_from(&args)
}
