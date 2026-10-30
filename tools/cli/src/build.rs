use crate::Result;
use crate::args::Args;
use crate::project::{Platform, Project};
use serde_json::{Value, json};

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct Built {
    pub executable: Option<PathBuf>,
    pub directory: Option<PathBuf>,
}

pub fn build(project: &Project, args: &Args) -> Result<Built> {
    let mut command = project.cargo();
    command.args([
        "build",
        "--locked",
        "--message-format=json-render-diagnostics",
        "--target",
        project.target(),
    ]);
    project.selection(&mut command, true, args);
    if args.release {
        command.arg("--release");
    }
    if project.platform == Platform::Web {
        command.arg("--lib");
    } else {
        command.arg("--bin").arg(
            project
                .host
                .bin
                .as_ref()
                .ok_or("native platform has no bin")?,
        );
    }
    let output = project.capture(&mut command)?;
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    let artifact = artifact(
        &output.stdout,
        project.platform,
        project.package["id"].as_str().ok_or("package has no id")?,
        project.host.bin.as_deref(),
    )?;
    if project.platform != Platform::Web {
        println!("Executable: {}", artifact.display());
        return Ok(Built {
            executable: Some(artifact),
            directory: None,
        });
    }
    let bindgen = bindgen(project, args)?;
    let output_root = project.paths.root.join(if args.release {
        "web-release"
    } else {
        "web-debug"
    });
    let stage = staging(&project.paths.root)?;
    let result = (|| {
        let mut command = Command::new(&bindgen);
        command
            .arg(&artifact)
            .args(["--target", "web", "--out-dir"])
            .arg(&stage);
        project.paths.apply(&mut command);
        project.status(&mut command)?;
        for asset in &project.host.assets {
            copy_asset(&project.root, asset, &stage)?;
        }
        if project.host.assets.is_empty() && project.root.join("public").is_dir() {
            copy_contents(&project.root.join("public"), &stage)?;
        }
        let index = project
            .host
            .index
            .clone()
            .unwrap_or_else(|| PathBuf::from("index.html"));
        if project.root.join(&index).exists() {
            let source = checked_asset(&project.root, &index)?;
            fs::copy(source, stage.join("index.html")).map_err(|e| e.to_string())?;
        } else if project.host.index.is_some() {
            return Err(format!(
                "configured HTML entry does not exist: {}",
                index.display()
            ));
        } else {
            let stem = artifact
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or("WASM artifact has no UTF-8 name")?;
            fs::write(stage.join("index.html"), format!("<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"></head><body><main id=\"app\"></main><script type=\"module\">import init from './{stem}.js';init();</script></body></html>")).map_err(|e| e.to_string())?;
        }
        if args.release {
            crate::compression::compress(&stage)?;
        }
        write_manifest(project, &stage, args)?;
        publish(&stage, &output_root, &project.paths.root)
    })();
    if result.is_err() {
        let _ = remove_owned(&stage, &project.paths.root);
    }
    result?;
    println!("Web output: {}", output_root.display());
    Ok(Built {
        executable: None,
        directory: Some(output_root),
    })
}

fn artifact(
    messages: &[u8],
    platform: Platform,
    package: &str,
    bin: Option<&str>,
) -> Result<PathBuf> {
    for line in messages.split(|byte| *byte == b'\n').rev() {
        let Ok(value) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        if value["reason"] != "compiler-artifact" || value["package_id"].as_str() != Some(package) {
            continue;
        }
        if platform != Platform::Web {
            if value["target"]["name"].as_str() != bin {
                continue;
            }
            if let Some(path) = value["executable"].as_str() {
                return Ok(PathBuf::from(path));
            }
        } else if let Some(paths) = value["filenames"].as_array() {
            for path in paths.iter().filter_map(Value::as_str) {
                if path.ends_with(".wasm") {
                    return Ok(PathBuf::from(path));
                }
            }
        }
    }
    Err("Cargo did not report the expected application artifact".into())
}

fn bindgen(project: &Project, args: &Args) -> Result<PathBuf> {
    let version = project
        .bindgen_version
        .as_ref()
        .ok_or("Web application has no resolved wasm-bindgen dependency")?;
    let executable = if let Some(path) = &args.bindgen {
        path.canonicalize()
            .map_err(|e| format!("{}: {e}", path.display()))?
    } else {
        let install = project
            .paths
            .root
            .join("tools")
            .join("wasm-bindgen")
            .join(version)
            .join(&project.rust_host);
        let executable = install.join("bin").join(if cfg!(windows) {
            "wasm-bindgen.exe"
        } else {
            "wasm-bindgen"
        });
        if !executable.exists() {
            if args.offline {
                return Err(format!(
                    "wasm-bindgen {version} is not installed in {}; run once without --offline",
                    install.display()
                ));
            }
            let mut command = project.cargo();
            command
                .args([
                    "install",
                    "wasm-bindgen-cli",
                    "--version",
                    version,
                    "--locked",
                    "--bin",
                    "wasm-bindgen",
                    "--root",
                ])
                .arg(&install)
                .env("CARGO_TARGET_DIR", project.paths.root.join("tool-build"));
            project.status(&mut command)?;
        }
        executable
    };
    let mut command = Command::new(&executable);
    command.arg("--version");
    project.paths.apply(&mut command);
    let output = project.capture(&mut command)?;
    let actual = String::from_utf8_lossy(&output.stdout);
    if actual.trim() != format!("wasm-bindgen {version}") {
        return Err(format!(
            "expected wasm-bindgen {version}, found {:?} at {}",
            actual.trim(),
            executable.display()
        ));
    }
    Ok(executable)
}

pub fn run(project: &Project, args: &Args) -> Result<()> {
    project.ensure_runnable()?;
    if project.platform == Platform::Web {
        let mut args = args.clone();
        args.watch = false;
        return crate::serve::serve(project.clone(), args);
    }
    let built = build(project, args)?;
    let mut command = Command::new(built.executable.ok_or("no executable was built")?);
    command.current_dir(&project.root);
    project.paths.apply(&mut command);
    project.status(&mut command)
}

pub fn bundle(project: &Project, args: &Args) -> Result<()> {
    let mut args = args.clone();
    args.release = true;
    let built = build(project, &args)?;
    let stage = staging(&project.paths.root)?;
    let result = (|| {
        if let Some(directory) = built.directory {
            copy_contents(&directory, &stage)?;
        }
        if let Some(executable) = built.executable {
            native_bundle(project, &executable, &stage, &args)?;
        }
        let platform = match project.platform {
            Platform::Web => "web",
            Platform::Desktop => "desktop",
            Platform::Server => "server",
        };
        let destination = project
            .paths
            .root
            .join("bundle")
            .join(project.name())
            .join(platform)
            .join(project.target());
        publish(&stage, &destination, &project.paths.root)?;
        println!("Bundle: {}", destination.display());
        Ok(())
    })();
    if result.is_err() {
        let _ = remove_owned(&stage, &project.paths.root);
    }
    result
}

fn native_bundle(project: &Project, executable: &Path, stage: &Path, args: &Args) -> Result<()> {
    if project.platform == Platform::Desktop && project.target().ends_with("-apple-darwin") {
        return crate::apple::bundle(project, executable, stage, args);
    }
    // Assets are copied first so neither the executable nor its integrity
    // manifest can be silently replaced by an application asset.
    for asset in &project.host.assets {
        copy_asset(&project.root, asset, stage)?;
    }
    let destination = stage.join(executable.file_name().ok_or("executable has no name")?);
    if destination.exists() || stage.join("glory-build.json").exists() {
        return Err("assets collide with the executable or reserved glory-build.json".into());
    }
    fs::copy(executable, destination).map_err(|e| e.to_string())?;
    write_manifest(project, stage, args)
}

pub fn staging(root: &Path) -> Result<PathBuf> {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = root.join(format!("stage-{}-{time}", std::process::id()));
    fs::create_dir(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn publish(stage: &Path, destination: &Path, root: &Path) -> Result<()> {
    let parent = destination.parent().ok_or("output has no parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    if !parent
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(&canonical_root)
    {
        return Err("output escaped the project build root".into());
    }
    let backup = stage.with_extension("previous");
    if destination.exists() {
        let target = destination.canonicalize().map_err(|e| e.to_string())?;
        if !target.starts_with(&canonical_root) || target == canonical_root {
            return Err("refusing to replace a foreign output".into());
        }
        fs::rename(destination, &backup).map_err(|e| e.to_string())?;
    }
    if let Err(error) = fs::rename(stage, destination) {
        if backup.exists() {
            fs::rename(&backup, destination)
                .map_err(|restore| format!("{error}; restore failed: {restore}"))?;
        }
        return Err(error.to_string());
    }
    if backup.exists()
        && let Err(error) = remove_owned(&backup, root)
    {
        eprintln!(
            "Published output; could not remove previous build {}: {error}",
            backup.display()
        );
    }
    Ok(())
}

pub fn remove_owned(path: &Path, root: &Path) -> Result<()> {
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if canonical == root || !canonical.starts_with(&root) {
        return Err("refusing to remove a foreign build directory".into());
    }
    fs::remove_dir_all(path).map_err(|e| e.to_string())
}

fn checked_asset(root: &Path, relative: &Path) -> Result<PathBuf> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(format!(
            "assets must use project-relative paths: {}",
            relative.display()
        ));
    }
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| format!("asset {}: {e}", relative.display()))?;
    if !path.starts_with(root) {
        return Err("asset symlink escaped the application".into());
    }
    Ok(path)
}

pub(crate) fn copy_asset(root: &Path, relative: &Path, destination: &Path) -> Result<()> {
    let source = checked_asset(root, relative)?;
    let target = destination.join(relative);
    if source.is_dir() {
        copy_contents(&source, &target)
    } else {
        fs::create_dir_all(target.parent().ok_or("asset has no parent")?)
            .map_err(|e| e.to_string())?;
        fs::copy(source, target).map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub fn copy_contents(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            return Err(format!(
                "asset directory contains a symlink: {}",
                entry.path().display()
            ));
        }
        if kind.is_dir() {
            copy_contents(&entry.path(), &destination.join(entry.file_name()))?;
        } else if kind.is_file() {
            fs::copy(entry.path(), destination.join(entry.file_name()))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub(crate) fn write_manifest(project: &Project, directory: &Path, args: &Args) -> Result<()> {
    let mut files = Vec::new();
    hash_files(directory, directory, &mut files)?;
    files.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    let manifest = json!({ "format": 1, "cli_version": env!("CARGO_PKG_VERSION"), "package": project.name(), "platform": project.platform, "target": project.target(), "rust_host": project.rust_host, "release": args.release, "wasm_bindgen": project.bindgen_version, "files": files });
    fs::write(
        directory.join("glory-build.json"),
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn hash_files(root: &Path, directory: &Path, files: &mut Vec<Value>) -> Result<()> {
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            hash_files(root, &entry.path(), files)?;
        } else {
            let bytes = fs::read(entry.path()).map_err(|e| e.to_string())?;
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            files.push(json!({ "path": relative, "bytes": bytes.len(), "sha256": crate::project::sha256(&bytes) }));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_bundle_contains_binary_assets_and_target_specific_hashes() {
        let scratch = crate::test_support::Scratch::new();
        scratch.write("compiled/fixture.exe", "executable bytes");
        scratch.write("assets/nested/config.txt", "asset bytes");
        let mut project = scratch.project(Platform::Desktop);
        project.host.assets.push("assets".into());
        let mut args = Args::parse(["bundle".into()]).unwrap();
        args.release = true;
        let stage = staging(&scratch.0).unwrap();
        native_bundle(
            &project,
            &scratch.0.join("compiled/fixture.exe"),
            &stage,
            &args,
        )
        .unwrap();
        let manifest: Value =
            serde_json::from_slice(&fs::read(stage.join("glory-build.json")).unwrap()).unwrap();
        assert_eq!(manifest["platform"], "desktop");
        assert_eq!(manifest["target"], project.target());
        assert_eq!(manifest["release"], true);
        let files = manifest["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["path"], "assets/nested/config.txt");
        for file in files {
            let bytes = fs::read(stage.join(file["path"].as_str().unwrap())).unwrap();
            assert_eq!(file["bytes"], bytes.len());
            assert_eq!(file["sha256"], crate::project::sha256(bytes));
        }
        let foreign = scratch
            .0
            .join("bundle/fixture/desktop/aarch64-apple-darwin");
        fs::create_dir_all(&foreign).unwrap();
        fs::write(foreign.join("keep"), "previous architecture").unwrap();
        let destination = scratch
            .0
            .join("bundle/fixture/desktop")
            .join(project.target());
        publish(&stage, &destination, &scratch.0).unwrap();
        assert!(destination.join("fixture.exe").exists());
        assert_eq!(
            fs::read_to_string(foreign.join("keep")).unwrap(),
            "previous architecture"
        );
    }

    #[test]
    fn native_bundle_rejects_assets_that_replace_its_binary_or_manifest() {
        for name in ["fixture.exe", "glory-build.json"] {
            let scratch = crate::test_support::Scratch::new();
            scratch.write("compiled/fixture.exe", "binary");
            scratch.write(name, "conflicting asset");
            let mut project = scratch.project(Platform::Desktop);
            project.host.assets.push(name.into());
            let args = Args::parse(["bundle".into()]).unwrap();
            let stage = staging(&scratch.0).unwrap();
            assert!(
                native_bundle(
                    &project,
                    &scratch.0.join("compiled/fixture.exe"),
                    &stage,
                    &args
                )
                .unwrap_err()
                .contains("collide")
            );
        }
    }
    #[test]
    fn chooses_reported_artifacts_not_guessed_target_filenames() {
        let input = br#"{"reason":"compiler-artifact","package_id":"app","filenames":["D:/odd/output.wasm"]}
{"reason":"compiler-artifact","package_id":"dependency","filenames":["D:/wrong/dependency.wasm"],"executable":"D:/wrong/helper.exe","target":{"name":"helper"}}
{"reason":"build-finished","success":true}
"#;
        assert_eq!(
            artifact(input, Platform::Web, "app", None).unwrap(),
            PathBuf::from("D:/odd/output.wasm")
        );
        assert!(artifact(input, Platform::Server, "app", Some("app")).is_err());
        assert!(artifact(input, Platform::Web, "missing", None).is_err());
    }

    #[test]
    fn failed_output_replacement_restores_the_last_successful_site() {
        let scratch = crate::test_support::Scratch::new();
        scratch.write("site/index.html", "previous build");
        assert!(
            publish(
                &scratch.0.join("missing-stage"),
                &scratch.0.join("site"),
                &scratch.0
            )
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(scratch.0.join("site/index.html")).unwrap(),
            "previous build"
        );
    }

    #[test]
    fn foreign_directories_cannot_be_removed_or_replaced() {
        let first = crate::test_support::Scratch::new();
        let second = crate::test_support::Scratch::new();
        second.write("keep/data", "owned by another project");
        assert!(remove_owned(&second.0.join("keep"), &first.0).is_err());
        assert!(publish(&first.0, &second.0.join("replacement"), &first.0).is_err());
        assert!(second.0.join("keep/data").exists());
    }
}
