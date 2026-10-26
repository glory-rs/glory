use std::ffi::OsString;
use std::path::PathBuf;

use crate::Result;

pub const HELP: &str = "Glory project tools
Usage: cargo glory <command> [options]
Commands:
  doctor   Verify the project, framework version, Rust and tool paths
  print    Print the resolved build plan as JSON
  build    Build Web, desktop or server artifacts
  run      Build and run (Web starts the static server without watching)
  serve    Build, watch and reload a Web app
  check    Cargo check and strict Clippy for the selected application
  fmt      Run rustfmt (--check is supported)
  bundle   Build a release Web directory or server executable
  init     Add a private CLI package, Cargo alias and bin launchers
Options:
  --platform web|desktop|server   Select a configured host
  --release                      Use Cargo's release profile
  --package NAME                 Select an application in a workspace
  --manifest-path PATH           Select the application manifest
  --cli-path PATH                Local glory-cli source for init (unpublished development)
  --build-root PATH              D:/BuildCache/<project> (or verified /mnt/d/...)
  --bindgen PATH                 Explicit wasm-bindgen executable, version checked
  --port PORT                    Development server port (0 selects a free port)
  --no-watch                     Disable development file watching
  --offline                      Do not fetch packages or install tools
  --check                        Check formatting without editing
  --force                        Replace generated launchers during init
  --help / --version
Use bin/glory.ps1 (Windows) or bin/glory to set project-local caches before Cargo.
Ordinary commands preserve Cargo.lock; update project dependencies explicitly.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    Doctor,
    Print,
    Build,
    Bundle,
    Run,
    Serve,
    Check,
    Fmt,
    Init,
}

#[derive(Clone, Debug)]
pub struct Args {
    pub cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub command: Command,
    pub platform: Option<String>,
    pub package: Option<String>,
    pub manifest: Option<PathBuf>,
    pub cli_path: Option<PathBuf>,
    pub build_root: Option<PathBuf>,
    pub bindgen: Option<PathBuf>,
    pub port: Option<u16>,
    pub release: bool,
    pub offline: bool,
    pub watch: bool,
    pub check: bool,
    pub force: bool,
}

impl Args {
    pub fn parse(input: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut input = input.into_iter();
        let command = match input.next().as_deref().and_then(|value| value.to_str()) {
            None | Some("help" | "--help" | "-h") => Command::Help,
            Some("--version" | "-V") => Command::Version,
            Some("doctor") => Command::Doctor,
            Some("print") => Command::Print,
            Some("build") => Command::Build,
            Some("bundle") => Command::Bundle,
            Some("run") => Command::Run,
            Some("serve") => Command::Serve,
            Some("check") => Command::Check,
            Some("fmt") => Command::Fmt,
            Some("init") => Command::Init,
            Some(other) => return Err(format!("unknown command {other:?}; use --help")),
        };
        let mut args = Self {
            cancel: None,
            command,
            platform: None,
            package: None,
            manifest: None,
            cli_path: None,
            build_root: None,
            bindgen: None,
            port: None,
            release: false,
            offline: false,
            watch: true,
            check: false,
            force: false,
        };
        while let Some(option) = input.next() {
            let option = option.to_str().ok_or("option names must be UTF-8")?;
            match option {
                "--platform" => args.platform = Some(next_text(&mut input, option)?),
                "--package" | "-p" => args.package = Some(next_text(&mut input, option)?),
                "--manifest-path" => {
                    args.manifest = Some(PathBuf::from(
                        input.next().ok_or("--manifest-path needs a path")?,
                    ))
                }
                "--cli-path" => {
                    args.cli_path = Some(PathBuf::from(
                        input.next().ok_or("--cli-path needs a path")?,
                    ))
                }
                "--build-root" => {
                    args.build_root = Some(PathBuf::from(
                        input.next().ok_or("--build-root needs a path")?,
                    ))
                }
                "--bindgen" => {
                    args.bindgen =
                        Some(PathBuf::from(input.next().ok_or("--bindgen needs a path")?))
                }
                "--port" => {
                    args.port = Some(
                        next_text(&mut input, option)?
                            .parse()
                            .map_err(|_| "--port must be 0..65535")?,
                    )
                }
                "--release" => args.release = true,
                "--offline" => args.offline = true,
                "--no-watch" => args.watch = false,
                "--check" => args.check = true,
                "--force" => args.force = true,
                "--help" | "-h" => args.command = Command::Help,
                other => return Err(format!("unknown option {other:?}")),
            }
        }
        Ok(args)
    }
}

fn next_text(input: &mut impl Iterator<Item = OsString>, option: &str) -> Result<String> {
    input
        .next()
        .ok_or_else(|| format!("{option} needs a value"))?
        .into_string()
        .map_err(|_| format!("{option} needs a UTF-8 value"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_mistyped_commands_and_values_without_running_cargo() {
        for input in [
            vec!["build", "--platfrom", "web"],
            vec!["serve", "--port", "65536"],
            vec!["build", "--platform"],
            vec!["self-update"],
        ] {
            assert!(Args::parse(input.into_iter().map(OsString::from)).is_err());
        }
    }
}
