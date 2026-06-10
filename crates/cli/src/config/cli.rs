use crate::command::NewCommand;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};

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
#[clap(version)]
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
        use Commands::{Build, Bundle, Check, Clean, EndToEnd, Fmt, New, Serve, Test, Watch};
        match &self.command {
            New(_) | Fmt(_) => None,
            Build(opts) | Bundle(opts) | Check(opts) | Test(opts) | EndToEnd(opts) | Watch(opts) => Some(opts.clone()),
            Serve(opts) => Some(opts.opts.clone()),
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

#[derive(Debug, Subcommand, PartialEq)]
pub enum Commands {
    /// Start a hot-reloading dev server (build, serve and live-reload on change).
    Serve(ServeOpts),
    /// Build the server (feature ssr) and the client (wasm with feature csr).
    Build(Opts),
    /// Build in release mode and collect the artifacts into a distributable `dist/` folder.
    Bundle(Opts),
    /// Remove build artifacts (front/server target dirs and the site root).
    Clean(CleanOpts),
    /// Type-check the client (wasm) and server without producing artifacts.
    Check(Opts),
    /// Format the project sources (passthrough to `cargo fmt`).
    Fmt(FmtOpts),
    /// Run the cargo tests for app, client and server.
    Test(Opts),
    /// Start the server and end-2-end tests.
    EndToEnd(Opts),
    /// Deprecated alias for `serve`; serve and automatically reload when files change.
    #[command(hide = true)]
    Watch(Opts),
    /// WIP: Start wizard for creating a new project (using cargo-generate). Ask at Glory discord before using.
    New(NewCommand),
}
