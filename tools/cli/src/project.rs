use crate::Result;
use crate::args::Args;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Web,
    Desktop,
    Server,
}

#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    pub default_platform: Option<String>,
    pub web: Host,
    pub desktop: Host,
    pub server: Host,
}

#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Host {
    pub bin: Option<String>,
    pub target: Option<String>,
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub index: Option<PathBuf>,
    pub assets: Vec<PathBuf>,
    pub port: Option<u16>,
    pub bundle_identifier: Option<String>,
}

#[derive(Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub cargo_home: PathBuf,
    pub target: PathBuf,
    pub temp: PathBuf,
}

impl Paths {
    pub fn new(manifest: &Path, requested: Option<&Path>) -> Result<Self> {
        let digest = sha256(manifest.as_os_str().to_string_lossy().as_bytes());
        let default = if cfg!(windows) {
            PathBuf::from("D:/BuildCache/glory-projects")
        } else if cfg!(target_os = "macos") {
            PathBuf::from("/Volumes/Data/Caches/glory/projects")
        } else {
            PathBuf::from("/mnt/d/BuildCache/glory-projects")
        }
        .join(&digest[..16]);
        let root = requested
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os("GLORY_BUILD_ROOT").map(PathBuf::from))
            .unwrap_or(default);
        validate_build_root(&root)?;
        std::fs::create_dir_all(&root).map_err(|e| format!("create {}: {e}", root.display()))?;
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        validate_build_root(&root)?;
        // Python build generators (including Stylo) join OUT_DIR with `..`.
        // Windows verbatim paths disable that normalization and reject the
        // result. Validate the real destination first, then pass Cargo a normal
        // absolute drive path so downstream generators can traverse parents.
        let root = compiler_path(root);
        let cargo_home = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .filter(|path| validate_build_root(path).is_ok())
            .unwrap_or_else(|| root.join("cargo"));
        let paths = Self {
            target: root.join("app-target"),
            temp: root.join("tmp"),
            cargo_home,
            root,
        };
        for path in [&paths.target, &paths.temp, &paths.cargo_home] {
            std::fs::create_dir_all(path).map_err(|e| format!("create {}: {e}", path.display()))?;
            validate_build_root(&path.canonicalize().map_err(|e| e.to_string())?)?;
        }
        Ok(paths)
    }

    pub fn apply(&self, command: &mut Command) {
        command
            .env("CARGO_HOME", &self.cargo_home)
            .env("CARGO_TARGET_DIR", &self.target)
            .env("TEMP", &self.temp)
            .env("TMP", &self.temp)
            .env("TMPDIR", &self.temp)
            .env("PYTHONPYCACHEPREFIX", self.root.join("pycache"));
    }
}

fn compiler_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    if let Some(normal) = path.to_str().and_then(|path| path.strip_prefix(r"\\?\"))
        && normal.as_bytes().get(1) == Some(&b':')
    {
        return PathBuf::from(normal);
    }
    path
}

pub fn validate_build_root(path: &Path) -> Result<()> {
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err("build paths must not contain '..'".into());
    }
    let portable = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let portable = portable.strip_prefix("//?/").unwrap_or(&portable);
    if cfg!(windows) {
        if portable.starts_with("d:/buildcache/") {
            return Ok(());
        }
    } else if cfg!(target_os = "macos") {
        if path.starts_with("/Volumes/Data/Caches/glory")
            && path != Path::new("/Volumes/Data/Caches/glory")
        {
            // Validate existing ancestors too, before creating any directories.
            let ancestor = path
                .ancestors()
                .find(|path| path.exists())
                .ok_or("build path has no existing ancestor")?;
            let resolved = ancestor.canonicalize().map_err(|e| e.to_string())?;
            if resolved.starts_with("/Volumes/Data/Caches/glory")
                || Path::new("/Volumes/Data/Caches/glory").starts_with(&resolved)
            {
                return Ok(());
            }
        }
    } else if portable.starts_with("/mnt/d/buildcache/") {
        let mount = capture(Command::new("findmnt").args(["-n", "-T", "/mnt/d", "-o", "SOURCE"]))?;
        if is_d_mount(&String::from_utf8_lossy(&mount.stdout)) {
            return Ok(());
        }
        return Err("/mnt/d is not verified as the Windows D: mount".into());
    }
    Err(format!(
        "build/cache paths must be under D:/BuildCache, verified /mnt/d/BuildCache, or macOS /Volumes/Data/Caches/glory: {}",
        path.display()
    ))
}

fn is_d_mount(source: &str) -> bool {
    let source = source.trim().replace('\\', "/").to_ascii_lowercase();
    source == "d:" || source.starts_with("d:/") || source == "//localhost/d$"
}

#[derive(Clone)]
pub struct Project {
    pub cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub root: PathBuf,
    pub manifest: PathBuf,
    pub package: Value,
    pub host: Host,
    pub platform: Platform,
    pub paths: Paths,
    pub rust_host: String,
    pub build_target: String,
    pub bindgen_version: Option<String>,
    pub watch_roots: Vec<PathBuf>,
}

impl Project {
    pub fn load(manifest: &Path, args: &Args) -> Result<Self> {
        let manifest = manifest
            .canonicalize()
            .map_err(|e| format!("{}: {e}", manifest.display()))?;
        let root = manifest
            .parent()
            .ok_or("manifest has no parent")?
            .to_path_buf();
        let paths = Paths::new(&manifest, args.build_root.as_deref())?;
        let mut command = cargo(&root, &paths);
        command
            .args([
                "metadata",
                "--locked",
                "--no-deps",
                "--format-version",
                "1",
                "--manifest-path",
            ])
            .arg(&manifest);
        if args.offline {
            command.arg("--offline");
        }
        let metadata: Value = serde_json::from_slice(
            &capture_with_cancel(&mut command, args.cancel.as_deref())?.stdout,
        )
        .map_err(|e| e.to_string())?;
        verify_target(&metadata, &paths)?;
        let packages = metadata["packages"]
            .as_array()
            .ok_or("Cargo metadata has no packages")?;
        let default_member = metadata["workspace_default_members"]
            .as_array()
            .filter(|members| members.len() == 1)
            .and_then(|members| members[0].as_str());
        let selected = packages
            .iter()
            .find(|package| {
                if let Some(name) = &args.package {
                    package["name"].as_str() == Some(name)
                } else {
                    package["manifest_path"]
                        .as_str()
                        .is_some_and(|path| Path::new(path) == manifest)
                }
            })
            .or_else(|| {
                if args.package.is_none() {
                    packages
                        .iter()
                        .find(|package| package["id"].as_str() == default_member)
                } else {
                    None
                }
            })
            .ok_or("select an application with --package when using a virtual workspace")?
            .clone();
        let manifest = PathBuf::from(
            selected["manifest_path"]
                .as_str()
                .ok_or("package has no manifest")?,
        );
        let root = manifest
            .parent()
            .ok_or("manifest has no parent")?
            .to_path_buf();
        let config = match selected["metadata"].get("glory") {
            Some(value) => serde_json::from_value::<Config>(value.clone())
                .map_err(|e| format!("package.metadata.glory: {e}"))?,
            None => Config::default(),
        };
        let targets = selected["targets"]
            .as_array()
            .ok_or("package has no targets")?;
        let has_web = targets.iter().any(|target| {
            target["crate_types"]
                .as_array()
                .is_some_and(|types| types.iter().any(|kind| kind == "cdylib"))
        });
        let platform = match args
            .platform
            .as_deref()
            .or(config.default_platform.as_deref())
            .unwrap_or(if has_web { "web" } else { "desktop" })
        {
            "web" => Platform::Web,
            "desktop" => Platform::Desktop,
            "server" => Platform::Server,
            other => {
                return Err(format!(
                    "unsupported platform {other:?}; choose web, desktop or server"
                ));
            }
        };
        if platform == Platform::Web && !has_web {
            return Err(
                "Web needs a library target with crate-type = [\"cdylib\", \"rlib\"]".into(),
            );
        }
        let mut host = match platform {
            Platform::Web => config.web,
            Platform::Desktop => config.desktop,
            Platform::Server => config.server,
        };
        if platform != Platform::Web {
            let bins: Vec<_> = targets
                .iter()
                .filter(|target| {
                    target["kind"]
                        .as_array()
                        .is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin"))
                })
                .collect();
            let target = if let Some(bin) = &host.bin {
                bins.iter()
                    .copied()
                    .find(|target| target["name"].as_str() == Some(bin))
            } else if bins.len() == 1 {
                Some(bins[0])
            } else {
                None
            }
            .ok_or(
                "set the selected platform's bin in package.metadata.glory (no unique binary)",
            )?;
            host.bin = target["name"].as_str().map(str::to_owned);
            if host.features.is_empty() {
                host.features = strings(&target["required-features"]);
            }
        } else if host.features.is_empty() && selected["features"].get("web").is_some() {
            host.features.push("web".into());
        }
        let rust = capture(Command::new("rustc").arg("-vV").current_dir(&root))?;
        let rust = String::from_utf8_lossy(&rust.stdout);
        let rust_host = rust
            .lines()
            .find_map(|line| line.strip_prefix("host: "))
            .ok_or("rustc did not report its host")?
            .to_owned();
        let build_target = resolve_target(
            platform,
            args.target.as_deref().or(host.target.as_deref()),
            &rust_host,
        )?;
        let targets = capture(Command::new("rustc").args(["--print", "target-list"]))?;
        if !String::from_utf8_lossy(&targets.stdout)
            .lines()
            .any(|target| target == build_target)
        {
            return Err(format!(
                "unknown built-in Rust target {build_target:?}; use rustc --print target-list"
            ));
        }
        let mut project = Self {
            cancel: args.cancel.clone(),
            root,
            manifest,
            package: selected,
            host,
            platform,
            paths,
            rust_host,
            build_target,
            bindgen_version: None,
            watch_roots: Vec::new(),
        };
        let mut command = project.cargo();
        command.args([
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--filter-platform",
            project.target(),
        ]);
        project.selection(&mut command, false, args);
        let metadata: Value = serde_json::from_slice(
            &capture_with_cancel(&mut command, args.cancel.as_deref())?.stdout,
        )
        .map_err(|e| e.to_string())?;
        project.bindgen_version = compatible_dependencies(
            &metadata,
            project.package["id"].as_str().ok_or("package has no id")?,
        )?;
        let packages = metadata["packages"]
            .as_array()
            .ok_or("Cargo metadata has no packages")?;
        project.watch_roots = packages
            .iter()
            .filter(|package| package["source"].is_null())
            .filter_map(|package| package["manifest_path"].as_str())
            .filter_map(|path| Path::new(path).parent())
            .map(Path::to_path_buf)
            .filter(|path| !path.starts_with(&project.paths.root))
            .collect();
        Ok(project)
    }

    pub fn cargo(&self) -> Command {
        cargo(&self.root, &self.paths)
    }

    pub fn capture(&self, command: &mut Command) -> Result<Output> {
        capture_with_cancel(command, self.cancel.as_deref())
    }

    pub fn status(&self, command: &mut Command) -> Result<()> {
        crate::process::inherited(command, self.cancel.as_deref())
    }

    pub fn selection(&self, command: &mut Command, package: bool, args: &Args) {
        command.arg("--manifest-path").arg(&self.manifest);
        if package {
            command.arg("--package").arg(self.name());
        }
        if !self.host.features.is_empty() {
            command.arg("--features").arg(self.host.features.join(","));
        }
        if self.host.no_default_features {
            command.arg("--no-default-features");
        }
        if args.offline {
            command.arg("--offline");
        }
    }

    pub fn name(&self) -> &str {
        self.package["name"].as_str().unwrap_or("app")
    }
    pub fn target(&self) -> &str {
        &self.build_target
    }

    pub fn ensure_runnable(&self) -> Result<()> {
        if self.platform != Platform::Web && self.target() != self.rust_host {
            return Err(format!(
                "cannot run target {} on host {}; use build or bundle and run on the target system",
                self.target(),
                self.rust_host
            ));
        }
        Ok(())
    }

    pub fn plan(&self, args: &Args) -> Value {
        json!({ "cli_version": env!("CARGO_PKG_VERSION"), "manifest": self.manifest,
            "package": self.name(), "platform": self.platform, "features": self.host.features, "rust_host": self.rust_host,
            "bin": self.host.bin, "target": self.target(), "release": args.release,
            "build_root": self.paths.root, "cargo_home": self.paths.cargo_home,
            "target_directory": self.paths.target, "temporary_directory": self.paths.temp,
            "wasm_bindgen_version": self.bindgen_version, "bindgen_override": args.bindgen })
    }

    pub fn print(&self, args: &Args) -> Result<()> {
        println!(
            "{}",
            serde_json::to_string_pretty(&self.plan(args)).map_err(|e| e.to_string())?
        );
        Ok(())
    }

    pub fn doctor(&self) -> Result<()> {
        let rust = capture(
            Command::new("rustc")
                .arg("--version")
                .current_dir(&self.root),
        )?;
        println!(
            "Glory CLI {}\nApplication: {}\n{}",
            env!("CARGO_PKG_VERSION"),
            self.name(),
            String::from_utf8_lossy(&rust.stdout).trim()
        );
        println!(
            "Host: {}\nBuild root: {}\nCargo target: {}\nCargo home: {}",
            self.rust_host,
            self.paths.root.display(),
            self.paths.target.display(),
            self.paths.cargo_home.display()
        );
        let mut command = Command::new("rustc");
        command.args(["--print", "target-libdir", "--target", self.target()]);
        self.paths.apply(&mut command);
        let output = self.capture(&mut command)?;
        let library = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        let installed = std::fs::read_dir(&library).is_ok_and(|entries| {
            entries.filter_map(std::result::Result::ok).any(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.starts_with("libstd-") && name.ends_with(".rlib")
            })
        });
        println!(
            "Build target: {}\nTarget libraries: {}",
            self.target(),
            library.display()
        );
        if !installed {
            return Err(format!(
                "Rust standard library for {} is missing; install it with rustup target add {}",
                self.target(),
                self.target()
            ));
        }
        if self.platform != Platform::Web && self.target() != self.rust_host {
            println!(
                "Cross-compilation selected: configure the target linker/SDK using Cargo; run is disabled. Linker and runtime availability require a real build and target-system acceptance."
            );
        }
        if let Some(version) = &self.bindgen_version {
            println!(
                "Required wasm-bindgen: {version} (project-local installation on first Web build)"
            );
        }
        println!("Framework version and Cargo target verified; no global Glory CLI is used.");
        Ok(())
    }

    pub fn check(&self, args: &Args) -> Result<()> {
        for subcommand in ["check", "clippy"] {
            let mut command = self.cargo();
            command.args([
                subcommand,
                "--locked",
                "--all-targets",
                "--target",
                self.target(),
            ]);
            self.selection(&mut command, true, args);
            if subcommand == "clippy" {
                command.args(["--", "-D", "warnings"]);
            }
            self.status(&mut command)?;
        }
        Ok(())
    }

    pub fn format(&self, args: &Args) -> Result<()> {
        let mut command = self.cargo();
        command
            .args(["fmt", "--manifest-path"])
            .arg(&self.manifest)
            .args(["--package", self.name()]);
        if args.check {
            command.args(["--", "--check"]);
        }
        self.status(&mut command)
    }
}

fn resolve_target(platform: Platform, requested: Option<&str>, rust_host: &str) -> Result<String> {
    let target = requested.unwrap_or(if platform == Platform::Web {
        "wasm32-unknown-unknown"
    } else {
        rust_host
    });
    if target.is_empty()
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(
            "--target must be a built-in Rust target triple, not a path or command option".into(),
        );
    }
    if platform == Platform::Web {
        if target != "wasm32-unknown-unknown" {
            return Err("Web requires --target wasm32-unknown-unknown".into());
        }
    } else if !(target.contains("-windows-")
        || (target.contains("-linux-") && !target.contains("android"))
        || target.ends_with("-apple-darwin"))
    {
        return Err(format!(
            "{target:?} has no configured desktop/server host; Android and iOS need dedicated application hosts and packaging"
        ));
    }
    Ok(target.to_owned())
}

fn cargo(root: &Path, paths: &Paths) -> Command {
    let mut command = Command::new("cargo");
    command.current_dir(root);
    paths.apply(&mut command);
    command
}

pub fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn verify_target(metadata: &Value, paths: &Paths) -> Result<()> {
    let actual = metadata["target_directory"]
        .as_str()
        .ok_or("Cargo metadata has no target_directory")?;
    let actual = std::fs::canonicalize(actual).map_err(|e| e.to_string())?;
    if actual != paths.target.canonicalize().map_err(|e| e.to_string())? {
        return Err(format!(
            "Cargo target directory differs from the selected D: build root: {actual:?}"
        ));
    }
    Ok(())
}

pub fn compatible_dependencies(metadata: &Value, application: &str) -> Result<Option<String>> {
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("Cargo metadata has no dependency resolution")?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("Cargo metadata has no packages")?;
    let mut pending = vec![application.to_owned()];
    let mut visited = HashSet::new();
    let mut framework = false;
    let mut bindgen = None;
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let package = packages
            .iter()
            .find(|package| package["id"].as_str() == Some(&id))
            .ok_or("resolved dependency is missing")?;
        let name = package["name"].as_str().unwrap_or("");
        if ["glory", "glory-core", "glory-render", "glory-ui"].contains(&name) {
            framework = true;
            if package["version"].as_str() != Some(env!("CARGO_PKG_VERSION")) {
                return Err(format!(
                    "{name} {} is incompatible with glory-cli {}; update the project's tool dependency and Cargo.lock together",
                    package["version"],
                    env!("CARGO_PKG_VERSION")
                ));
            }
            verify_source(package)?;
        }
        if name == "wasm-bindgen" {
            let version = package["version"]
                .as_str()
                .ok_or("wasm-bindgen has no version")?
                .to_owned();
            if bindgen
                .as_ref()
                .is_some_and(|existing| existing != &version)
            {
                return Err("multiple wasm-bindgen versions reach the selected application".into());
            }
            bindgen = Some(version);
        }
        if let Some(node) = nodes.iter().find(|node| node["id"].as_str() == Some(&id))
            && let Some(deps) = node["deps"].as_array()
        {
            for dep in deps {
                let normal = dep["dep_kinds"]
                    .as_array()
                    .is_none_or(|kinds| kinds.iter().any(|kind| kind["kind"].is_null()));
                if normal && let Some(id) = dep["pkg"].as_str() {
                    pending.push(id.to_owned());
                }
            }
        }
    }
    if !framework {
        return Err("selected application has no active Glory dependency".into());
    }
    Ok(bindgen)
}

pub fn capture(command: &mut Command) -> Result<Output> {
    capture_with_cancel(command, None)
}

fn capture_with_cancel(
    command: &mut Command,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Output> {
    let output = crate::process::captured(command, cancel)?;
    if !output.status.success() {
        let diagnostics = output
            .stdout
            .split(|byte| *byte == b'\n')
            .filter_map(|line| {
                let message: Value = serde_json::from_slice(line).ok()?;
                message["message"]["rendered"].as_str().map(str::to_owned)
            })
            .collect::<Vec<_>>()
            .join("");
        return Err(format!(
            "{command:?} exited with {}\n{}{diagnostics}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output)
}

fn verify_source(package: &Value) -> Result<()> {
    let tool_revision = env!("GLORY_CLI_SOURCE_REV");
    let framework_revision = if let Some(source) = package["source"].as_str() {
        if source.starts_with("git+") {
            Some(
                source
                    .rsplit('#')
                    .next()
                    .ok_or("Git dependency has no revision")?
                    .to_owned(),
            )
        } else {
            None
        }
    } else if let Some(manifest) = package["manifest_path"].as_str() {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(
                Path::new(manifest)
                    .parent()
                    .ok_or("dependency manifest has no parent")?,
            )
            .output();
        output
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        None
    };
    if let Some(revision) = framework_revision
        && (tool_revision.is_empty() || tool_revision != revision)
    {
        return Err(format!(
            "Glory source revision {revision} differs from CLI revision {tool_revision:?}; pin both to the same full Git commit (a matching version number is insufficient)"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_targets_are_explicit_and_mobile_is_not_misreported_as_desktop() {
        let host = "x86_64-pc-windows-msvc";
        assert_eq!(resolve_target(Platform::Desktop, None, host).unwrap(), host);
        assert_eq!(
            resolve_target(Platform::Web, None, host).unwrap(),
            "wasm32-unknown-unknown"
        );
        for target in [
            "aarch64-apple-darwin",
            "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-musl",
            host,
        ] {
            assert_eq!(
                resolve_target(Platform::Desktop, Some(target), host).unwrap(),
                target
            );
            assert_eq!(
                resolve_target(Platform::Server, Some(target), host).unwrap(),
                target
            );
            assert!(resolve_target(Platform::Web, Some(target), host).is_err());
        }
        for target in [
            "aarch64-linux-android",
            "aarch64-apple-ios",
            "aarch64-apple-ios-sim",
            "wasm32-unknown-unknown",
            "../target.json",
            "--help",
            "",
            "x86_64\nlinux",
        ] {
            assert!(
                resolve_target(Platform::Desktop, Some(target), host).is_err(),
                "{target}"
            );
        }
    }

    #[test]
    fn cross_target_can_be_planned_but_cannot_run_on_the_host() {
        let scratch = crate::test_support::Scratch::new();
        let mut project = scratch.project(Platform::Desktop);
        let args = Args::parse(["print".into()]).unwrap();
        assert!(project.ensure_runnable().is_ok());
        project.build_target = "aarch64-apple-darwin".into();
        assert_eq!(project.plan(&args)["target"], "aarch64-apple-darwin");
        assert_eq!(project.plan(&args)["rust_host"], "x86_64-pc-windows-msvc");
        assert!(
            project
                .ensure_runnable()
                .unwrap_err()
                .contains("cannot run target")
        );
        // Rejection precedes Cargo or executable creation, even for a broken app.
        assert!(
            crate::build::run(&project, &args)
                .unwrap_err()
                .contains("cannot run target")
        );
    }
    #[test]
    fn recognizes_both_wsl_drive_mount_formats_without_accepting_other_drives() {
        for source in ["D:\\\n", "D:", "//localhost/D$\n"] {
            assert!(is_d_mount(source), "{source}");
        }
        for source in [
            "C:\\",
            "/dev/sdb",
            "//remote/D$",
            "//localhost/C$",
            "tmpfs D:",
        ] {
            assert!(!is_d_mount(source), "{source}");
        }
    }
    #[test]
    fn rejects_system_and_escaping_cache_directories() {
        for path in [
            "C:/tmp/glory",
            "D:/Works/app/target",
            "D:/BuildCache/app/../other",
            "/tmp/glory",
        ] {
            assert!(validate_build_root(Path::new(path)).is_err(), "{path}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn apple_cache_symlinks_cannot_escape_before_directory_creation() {
        let scratch = crate::test_support::Scratch::new();
        let escaped = scratch.0.join("escaped");
        std::os::unix::fs::symlink("/tmp", &escaped).unwrap();
        assert!(validate_build_root(&escaped.join("uncreated-child")).is_err());
        assert!(
            Paths::new(
                &scratch.0.join("Cargo.toml"),
                Some(&escaped.join("uncreated-child"))
            )
            .is_err()
        );
    }

    #[test]
    fn matching_semver_does_not_hide_a_different_git_revision() {
        assert!(verify_source(&json!({"source":"git+https://example.invalid/glory?rev=other#0000000000000000000000000000000000000000"})).is_err());
        let revision = env!("GLORY_CLI_SOURCE_REV");
        if !revision.is_empty() {
            assert!(
                verify_source(
                    &json!({"source":format!("git+https://example.invalid/glory#{revision}")})
                )
                .is_ok()
            );
        }
    }
    #[test]
    fn incompatible_framework_is_rejected_but_unrelated_workspace_versions_are_not_visited() {
        let mut metadata = json!({"packages": [{"id":"app","name":"app"}, {"id":"current","name":"glory","version":env!("CARGO_PKG_VERSION")}, {"id":"other","name":"glory","version":"0.1.0"}], "resolve":{"nodes":[{"id":"app","deps":[{"pkg":"current","dep_kinds":[{"kind":null}]}]}]}});
        assert!(compatible_dependencies(&metadata, "app").is_ok());
        metadata["resolve"]["nodes"][0]["deps"][0]["pkg"] = json!("other");
        assert!(
            compatible_dependencies(&metadata, "app")
                .unwrap_err()
                .contains("incompatible")
        );
    }
}

/// Keep project identities and artifact digests stable across SHA-2 API versions.
pub(crate) fn sha256(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod digest_tests {
    #[test]
    fn sha256_matches_the_standard_abc_vector() {
        assert_eq!(
            super::sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
