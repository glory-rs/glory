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
  serve    Build/watch Web reload or native process restart
  check    Cargo check and strict Clippy for the selected application
  fmt      Run rustfmt (--check is supported)
  bundle   Build a release Web, desktop or server directory
  init     Add a private CLI package, Cargo alias and bin launchers
  completions SHELL  Print completions for bash, zsh, fish or powershell
Options:
  --platform web|desktop|server   Select a configured host
  --target TRIPLE                 Rust target (native host by default; Web uses wasm32)
  --release                      Use Cargo's release profile
  --package NAME                 Select an application in a workspace
  --bin NAME                     Select a native binary instead of the configured entry
  --features LIST                Add comma/space-separated Cargo features (repeatable)
  --no-default-features          Disable the application's default features
  --manifest-path PATH           Select the application manifest
  --cli-path PATH                Local glory-cli source for init (unpublished development)
  --build-root PATH              D:/BuildCache/<project> (macOS: /Volumes/Data/Caches/glory/...)
  --bindgen PATH                 Explicit wasm-bindgen executable, version checked
  --wasm-opt PATH                Explicit Binaryen optimizer for release Web builds
  --wasm-opt-version VERSION     Required exact optimizer version (for example 129)
  --port PORT                    Development server port (0 selects a free port)
  --no-watch                     Disable development file watching
  --offline                      Do not fetch packages or install tools
  --check                        Check formatting without editing
  --force                        Replace generated launchers during init
  --help / --version
Native run/serve accept application arguments after -- (preserved on restart).
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
    Completions,
}

#[derive(Clone, Debug)]
pub struct Args {
    pub cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub command: Command,
    pub platform: Option<String>,
    pub target: Option<String>,
    pub package: Option<String>,
    pub bin: Option<String>,
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub app_args: Vec<OsString>,
    pub shell: Option<String>,
    pub manifest: Option<PathBuf>,
    pub cli_path: Option<PathBuf>,
    pub build_root: Option<PathBuf>,
    pub bindgen: Option<PathBuf>,
    pub wasm_opt: Option<PathBuf>,
    pub wasm_opt_version: Option<String>,
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
            Some("completions") => Command::Completions,
            Some(other) => return Err(format!("unknown command {other:?}; use --help")),
        };
        let mut args = Self {
            cancel: None,
            command,
            platform: None,
            target: None,
            package: None,
            bin: None,
            features: Vec::new(),
            no_default_features: false,
            app_args: Vec::new(),
            shell: None,
            manifest: None,
            cli_path: None,
            build_root: None,
            bindgen: None,
            wasm_opt: None,
            wasm_opt_version: None,
            port: None,
            release: false,
            offline: false,
            watch: true,
            check: false,
            force: false,
        };
        if command == Command::Completions {
            let shell = next_text(&mut input, "completions")?;
            if matches!(shell.as_str(), "--help" | "-h") {
                args.command = Command::Help;
            } else if ["bash", "zsh", "fish", "powershell"].contains(&shell.as_str()) {
                args.shell = Some(shell);
            } else {
                return Err("completions needs bash, zsh, fish or powershell".into());
            }
        }
        while let Some(option) = input.next() {
            let option = option.to_str().ok_or("option names must be UTF-8")?;
            match option {
                "--platform" => args.platform = Some(next_text(&mut input, option)?),
                "--target" => args.target = Some(next_text(&mut input, option)?),
                "--package" | "-p" => args.package = Some(next_text(&mut input, option)?),
                "--bin" => args.bin = Some(next_text(&mut input, option)?),
                "--features" | "-F" => {
                    let list = next_text(&mut input, option)?;
                    let features: Vec<_> = list
                        .split(|c: char| c == ',' || c.is_whitespace())
                        .filter(|feature| !feature.is_empty())
                        .collect();
                    if features.is_empty()
                        || features.iter().any(|feature| feature.starts_with('-'))
                    {
                        return Err("--features needs Cargo feature names".into());
                    }
                    for feature in features {
                        if !args.features.iter().any(|existing| existing == feature) {
                            args.features.push(feature.to_owned());
                        }
                    }
                }
                "--no-default-features" => args.no_default_features = true,
                "--" if matches!(command, Command::Run | Command::Serve) => {
                    args.app_args.extend(input);
                    break;
                }
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
                "--wasm-opt" => {
                    args.wasm_opt = Some(PathBuf::from(
                        input.next().ok_or("--wasm-opt needs a path")?,
                    ))
                }
                "--wasm-opt-version" => {
                    let version = next_text(&mut input, option)?;
                    if version.is_empty() || !version.bytes().all(|byte| byte.is_ascii_digit()) {
                        return Err(
                            "--wasm-opt-version needs an exact numeric Binaryen version".into()
                        );
                    }
                    args.wasm_opt_version = Some(version);
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
        if args.command != Command::Help {
            if args.wasm_opt.is_some() != args.wasm_opt_version.is_some() {
                return Err("--wasm-opt and --wasm-opt-version must be supplied together".into());
            }
            if args.wasm_opt.is_some()
                && (!matches!(
                    command,
                    Command::Build | Command::Bundle | Command::Print | Command::Doctor
                ) || (!args.release
                    && command != Command::Bundle
                    && command != Command::Doctor))
            {
                return Err(
                    "wasm-opt requires a release Web build, bundle, print or doctor".into(),
                );
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
            vec!["build", "--features", " , "],
            vec!["build", "--features", "--offline"],
            vec!["build", "--", "app-argument"],
            vec!["build", "--release", "--wasm-opt", "tool"],
            vec!["build", "--wasm-opt", "tool", "--wasm-opt-version", "133"],
            vec![
                "build",
                "--release",
                "--wasm-opt",
                "tool",
                "--wasm-opt-version",
                "latest",
            ],
        ] {
            assert!(Args::parse(input.into_iter().map(OsString::from)).is_err());
        }
    }

    #[test]
    fn feature_lists_merge_and_application_options_are_not_parsed_as_cli_options() {
        let args = Args::parse(
            [
                "serve",
                "--features",
                "a,b a",
                "-F",
                "c b",
                "--no-default-features",
                "--",
                "--port",
                "foreign value",
                "--offline",
            ]
            .map(OsString::from),
        )
        .unwrap();
        assert_eq!(args.features, ["a", "b", "c"]);
        assert!(args.no_default_features);
        assert!(!args.offline);
        assert_eq!(args.app_args, ["--port", "foreign value", "--offline"]);
        assert_eq!(
            Args::parse(["completions".into(), "--help".into()])
                .unwrap()
                .command,
            Command::Help
        );
    }

    #[cfg(unix)]
    #[test]
    fn application_arguments_preserve_non_utf8_bytes() {
        use std::os::unix::ffi::OsStringExt;
        let argument = OsString::from_vec(vec![0xff]);
        let args = Args::parse(["run".into(), "--".into(), argument.clone()]).unwrap();
        assert_eq!(args.app_args, [argument]);
    }
}
