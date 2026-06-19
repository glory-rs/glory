use crate::command::NewCommand;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use std::net::IpAddr;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Log {
    /// WASM build (wasm, wasm-opt, walrus)
    Wasm,
    /// Internal reload and csr server (hyper, salvo)
    Server,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Default)]
pub enum BuildTarget {
    #[default]
    Web,
    Desktop,
    Native,
    /// Android library build (`cdylib` via cargo-ndk). See
    /// `crates/cli/templates/mobile/README.md` for the host-project wiring.
    Android,
    /// iOS static library build (`staticlib`, macOS host required).
    Ios,
}

impl BuildTarget {
    pub fn is_mobile(&self) -> bool {
        matches!(self, BuildTarget::Android | BuildTarget::Ios)
    }
}

#[derive(Debug, Clone, Parser, PartialEq, Default)]
pub struct Opts {
    /// Build artifacts in release mode, with optimizations.
    #[arg(short, long)]
    pub release: bool,

    /// Turn on partial hot-reloading. Requires rust nightly [beta]
    #[arg(long)]
    pub hot_reload: bool,

    /// Which project to use, from a list of projects defined in a workspace
    #[arg(short, long)]
    pub project: Option<String>,

    /// The features to use when compiling all targets
    #[arg(long)]
    pub features: Vec<String>,

    /// The features to use when compiling the lib target
    #[arg(long)]
    pub lib_features: Vec<String>,

    /// The features to use when compiling the bin target
    #[arg(long)]
    pub bin_features: Vec<String>,

    /// Verbosity (none: info, errors & warnings, -v: verbose, --vv: very verbose).
    #[arg(short, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Platform build matrix to execute.
    #[arg(long, value_enum, default_value_t = BuildTarget::Web)]
    pub target: BuildTarget,
}

#[derive(Debug, Parser)]
#[command(name = "glory", version)]
pub struct Cli {
    /// Path to Cargo.toml.
    #[arg(long)]
    pub manifest_path: Option<Utf8PathBuf>,

    /// Output logs from dependencies (multiple --log accepted).
    #[arg(long)]
    pub log: Vec<Log>,

    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub fn opts(&self) -> Option<Opts> {
        use Commands::{Build, Bundle, Check, Clean, Completions, Config, Doctor, EndToEnd, Fmt, New, Run, SelfUpdate, Serve, Test};
        match &self.command {
            New(_) | Fmt(_) | Completions(_) | SelfUpdate => None,
            Build(opts) | Bundle(opts) | Check(opts) | Test(opts) | EndToEnd(opts) | Doctor(opts) => Some(opts.clone()),
            Config(opts) => Some(opts.opts.clone()),
            Serve(opts) => Some(opts.opts.clone()),
            Run(opts) => Some(opts.opts.clone()),
            Clean(opts) => Some(opts.opts.clone()),
        }
    }
}

/// Extra flags for `serve`.
#[derive(Debug, Clone, Parser, PartialEq, Default)]
pub struct ServeOpts {
    #[command(flatten)]
    pub opts: Opts,

    /// Build and serve once without watching files or live-reloading.
    #[arg(long)]
    pub no_reload: bool,

    /// Override the host address from Cargo metadata for this serve run.
    #[arg(long)]
    pub address: Option<IpAddr>,

    /// Override the site port from Cargo metadata for this serve run.
    #[arg(long)]
    pub port: Option<u16>,

    /// Explicitly open the app in the default browser. This is the default.
    #[arg(long, action = clap::ArgAction::SetTrue, conflicts_with = "no_open")]
    pub open: bool,

    /// Do not open the app in the default browser after the first build.
    #[arg(long = "no-open", action = clap::ArgAction::SetTrue)]
    pub no_open: bool,
}

impl ServeOpts {
    pub fn should_open(&self) -> bool {
        self.open || !self.no_open
    }
}

/// Extra flags for `run`.
#[derive(Debug, Clone, Parser, PartialEq, Default)]
pub struct RunOpts {
    #[command(flatten)]
    pub opts: Opts,

    /// Override the host address from Cargo metadata for this run.
    #[arg(long)]
    pub address: Option<IpAddr>,

    /// Override the site port from Cargo metadata for this run.
    #[arg(long)]
    pub port: Option<u16>,

    /// Explicitly open the app in the default browser. This is the default.
    #[arg(long, action = clap::ArgAction::SetTrue, conflicts_with = "no_open")]
    pub open: bool,

    /// Do not open the app in the default browser after the build.
    #[arg(long = "no-open", action = clap::ArgAction::SetTrue)]
    pub no_open: bool,
}

impl RunOpts {
    pub fn should_open(&self) -> bool {
        self.open || !self.no_open
    }
}

/// Extra flags for `clean`.
#[derive(Debug, Clone, Parser, PartialEq, Default)]
pub struct CleanOpts {
    #[command(flatten)]
    pub opts: Opts,

    /// Also run `cargo clean` at the workspace root.
    #[arg(long)]
    pub cargo: bool,
}

/// Flags for `config`.
#[derive(Debug, Clone, Parser, PartialEq, Default)]
pub struct ConfigOpts {
    #[command(flatten)]
    pub opts: Opts,

    /// Print the resolved project summary as JSON.
    #[arg(long)]
    pub json: bool,

    /// Print the Glory Cargo metadata schema and exit without loading a project.
    #[arg(long)]
    pub schema: bool,
}

/// Flags for `fmt` (a thin passthrough over `cargo fmt`).
#[derive(Debug, Clone, Parser, PartialEq, Default)]
pub struct FmtOpts {
    /// Run in check mode (errors if formatting is needed); maps to `cargo fmt --check`.
    #[arg(long)]
    pub check: bool,

    /// Extra arguments forwarded to `cargo fmt` after a `--` separator.
    #[arg(last = true)]
    pub args: Vec<String>,
}

/// Flags for `completions`.
#[derive(Debug, Clone, Parser, PartialEq)]
pub struct CompletionsOpts {
    /// Shell to generate completions for.
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,
}

#[derive(Debug, Subcommand, PartialEq)]
pub enum Commands {
    /// Start a hot-reloading dev server (build, serve and live-reload on change).
    Serve(ServeOpts),
    /// Build and run the app server without watching files or live-reloading.
    Run(RunOpts),
    /// Build the server (feature ssr) and the client (wasm with feature csr).
    Build(Opts),
    /// Build in release mode and collect the artifacts into a distributable `dist/` folder.
    Bundle(Opts),
    /// Remove build artifacts (front/server target dirs and the site root).
    Clean(CleanOpts),
    /// Type-check the client (wasm) and server without producing artifacts.
    Check(Opts),
    /// Validate Glory Cargo metadata or print the metadata schema.
    Config(ConfigOpts),
    /// Check local toolchains and platform prerequisites for the selected target.
    Doctor(Opts),
    /// Format the project sources (passthrough to `cargo fmt`).
    Fmt(FmtOpts),
    /// Generate shell completions to stdout.
    Completions(CompletionsOpts),
    /// Print how to update the Glory CLI.
    SelfUpdate,
    /// Run the cargo tests for app, client and server.
    Test(Opts),
    /// Start the server and end-2-end tests.
    EndToEnd(Opts),
    /// Scaffold a new Glory project from a built-in template or cargo-generate source.
    New(NewCommand),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serve_accepts_network_and_open_flags() {
        let cli = Cli::parse_from(["glory", "serve", "--address", "0.0.0.0", "--port", "9000", "--no-open"]);

        let Commands::Serve(serve) = cli.command else {
            panic!("expected serve command");
        };

        assert_eq!(serve.address, Some([0, 0, 0, 0].into()));
        assert_eq!(serve.port, Some(9000));
        assert!(!serve.should_open());
    }

    #[test]
    fn serve_opens_by_default_and_accepts_explicit_open() {
        let cli = Cli::parse_from(["glory", "serve"]);
        let Commands::Serve(default_serve) = cli.command else {
            panic!("expected serve command");
        };
        assert!(default_serve.should_open());

        let cli = Cli::parse_from(["glory", "serve", "--open"]);
        let Commands::Serve(explicit_serve) = cli.command else {
            panic!("expected serve command");
        };
        assert!(explicit_serve.should_open());
    }

    #[test]
    fn serve_rejects_conflicting_open_flags() {
        let result = Cli::try_parse_from(["glory", "serve", "--open", "--no-open"]);

        assert!(result.is_err());
    }

    #[test]
    fn run_accepts_network_and_open_flags() {
        let cli = Cli::parse_from(["glory", "run", "--address", "127.0.0.1", "--port", "8080", "--no-open"]);

        let Commands::Run(run) = cli.command else {
            panic!("expected run command");
        };

        assert_eq!(run.address, Some([127, 0, 0, 1].into()));
        assert_eq!(run.port, Some(8080));
        assert!(!run.should_open());
    }

    #[test]
    fn parses_project_free_utility_commands() {
        let cli = Cli::parse_from(["glory", "completions", "powershell"]);
        assert!(matches!(cli.command, Commands::Completions(_)));
        assert!(cli.opts().is_none());

        let cli = Cli::parse_from(["glory", "self-update"]);
        assert_eq!(cli.command, Commands::SelfUpdate);
        assert!(cli.opts().is_none());
    }
}
