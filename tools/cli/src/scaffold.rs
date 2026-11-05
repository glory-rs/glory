use std::fs;
use std::path::Path;

use toml_edit::{Array, DocumentMut, Item, Table, value};

use crate::Result;

pub fn init(manifest: &Path, force: bool, cli_path: Option<&Path>) -> Result<()> {
    let manifest = manifest.canonicalize().map_err(|e| e.to_string())?;
    let application_root = manifest.parent().ok_or("manifest has no parent")?;
    let paths = crate::project::Paths::new(&manifest, None)?;
    let mut metadata = std::process::Command::new("cargo");
    metadata
        .current_dir(application_root)
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(&manifest);
    paths.apply(&mut metadata);
    let metadata: serde_json::Value =
        serde_json::from_slice(&crate::project::capture(&mut metadata)?.stdout)
            .map_err(|e| e.to_string())?;
    let root = std::path::PathBuf::from(
        metadata["workspace_root"]
            .as_str()
            .ok_or("Cargo has no workspace_root")?,
    )
    .canonicalize()
    .map_err(|e| e.to_string())?;
    let manifest = root.join("Cargo.toml");
    let original = fs::read_to_string(&manifest).map_err(|e| e.to_string())?;
    let mut document = original.parse::<DocumentMut>().map_err(|e| e.to_string())?;
    let name = document
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(Item::as_str)
        .unwrap_or("glory-project");
    let mut name = format!("{name}-tools");
    let tool_manifest = root.join("tools/glory/Cargo.toml");
    let existing_tool = if tool_manifest.exists() {
        let text = fs::read_to_string(&tool_manifest).map_err(|e| e.to_string())?;
        let parsed = text.parse::<DocumentMut>().map_err(|e| e.to_string())?;
        name = parsed["package"]["name"]
            .as_str()
            .ok_or("existing tool has no package name")?
            .to_owned();
        Some(text)
    } else {
        None
    };
    let mut runner_manifest = format!(
        "[package]\nname = {name:?}\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[dependencies]\nglory-cli = \"={}\"\n",
        env!("CARGO_PKG_VERSION")
    );
    if let Some(path) = cli_path {
        let path = path.canonicalize().map_err(|e| e.to_string())?;
        let path = relative_to(&root.join("tools/glory"), &path);
        let path = path.to_string_lossy().replace('\\', "/");
        let path = path.strip_prefix("//?/").unwrap_or(&path);
        runner_manifest = format!(
            "[package]\nname = {name:?}\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[dependencies.glory-cli]\npath = {}\n",
            toml_edit::Value::from(path)
        );
    }
    // Re-running init never changes an existing project's chosen CLI source/pin.
    if let Some(existing) = existing_tool {
        runner_manifest = existing;
    }
    let runner = "fn main() -> std::process::ExitCode {\n    glory_cli::run_from_manifest(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../Cargo.toml\"))\n}\n";
    let mut config = if root.join(".cargo/config.toml").exists() {
        fs::read_to_string(root.join(".cargo/config.toml"))
            .map_err(|e| e.to_string())?
            .parse::<DocumentMut>()
            .map_err(|e| e.to_string())?
    } else {
        DocumentMut::new()
    };
    if config.get("alias").is_none() {
        config["alias"] = Item::Table(Table::new());
    }
    let alias = format!("run --locked -p {name} --");
    if config["alias"]
        .get("glory")
        .is_some_and(|existing| existing.as_str() != Some(&alias))
        && !force
    {
        return Err(".cargo/config.toml already defines alias.glory; use --force to replace generated entry points".into());
    }
    config["alias"]["glory"] = value(alias);
    let files = [
        (root.join("tools/glory/Cargo.toml"), runner_manifest),
        (root.join("tools/glory/src/main.rs"), runner.to_owned()),
        (root.join("bin/glory.ps1"), POWERSHELL.to_owned()),
        (root.join("bin/glory"), SHELL.to_owned()),
    ];
    for (path, content) in &files {
        if path.exists()
            && !force
            && fs::read_to_string(path).map_err(|e| e.to_string())? != *content
        {
            return Err(format!(
                "{} already exists; init did not write any files",
                path.display()
            ));
        }
    }
    if document.get("workspace").is_none() {
        document["workspace"] = Item::Table(Table::new());
    }
    if document["workspace"].get("resolver").is_none() {
        document["workspace"]["resolver"] = value("2");
    }
    if document["workspace"].get("members").is_none() {
        document["workspace"]["members"] = value(Array::new());
    }
    let original_members = document["workspace"]["members"]
        .as_array()
        .ok_or("workspace.members must be an array")?
        .clone();
    let root_package = document.get("package").is_some();
    let members = document["workspace"]["members"]
        .as_array_mut()
        .ok_or("workspace.members must be an array")?;
    if !members
        .iter()
        .any(|member| member.as_str() == Some("tools/glory"))
    {
        members.push("tools/glory");
    }
    if document["workspace"].get("default-members").is_none() {
        let mut defaults = original_members;
        if root_package {
            defaults = Array::new();
            defaults.push(".");
        }
        document["workspace"]["default-members"] = value(defaults);
    }
    let mut files = files.to_vec();
    files.push((root.join(".cargo/config.toml"), config.to_string()));
    files.push((manifest, document.to_string()));
    // All conflicts, workspace shapes and output boundaries precede writes.
    for (path, _) in &files {
        let mut parent = path.parent().ok_or("generated file has no parent")?;
        while !parent.exists() {
            parent = parent
                .parent()
                .ok_or("generated file has no existing ancestor")?;
        }
        if !parent.is_dir() {
            return Err(format!(
                "generated-file parent is not a directory: {}",
                parent.display()
            ));
        }
        if !parent
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(&root)
        {
            return Err(format!(
                "generated file escaped the workspace: {}",
                path.display()
            ));
        }
        if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(format!(
                "refusing to overwrite a generated-file symlink: {}",
                path.display()
            ));
        }
    }
    let mut written: Vec<(std::path::PathBuf, Option<Vec<u8>>)> = Vec::new();
    for (path, content) in &files {
        let previous = match fs::read(path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.to_string()),
        };
        written.push((path.clone(), previous));
        let write = fs::create_dir_all(path.parent().ok_or("generated file has no parent")?)
            .and_then(|_| fs::write(path, content));
        if let Err(error) = write {
            let mut failures = Vec::new();
            for (path, previous) in written.into_iter().rev() {
                let restore = if let Some(bytes) = previous {
                    fs::write(&path, bytes)
                } else if path.exists() {
                    fs::remove_file(&path)
                } else {
                    Ok(())
                };
                if let Err(error) = restore {
                    failures.push(format!("{}: {error}", path.display()));
                }
            }
            if !failures.is_empty() {
                return Err(format!(
                    "{error}; rollback failures: {}",
                    failures.join(", ")
                ));
            }
            return Err(error.to_string());
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root.join("bin/glory"), fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    println!(
        "Created project tools. Commit the launchers, tool package and updated Cargo.lock.\nRun bin/glory.ps1 doctor (Windows) or bin/glory doctor (WSL).\nThe launcher resolves Cargo.lock once on first use; subsequent runs use --locked.\nFor local Glory development, replace the tool dependency with a path to tools/cli."
    );
    Ok(())
}

fn relative_to(from: &Path, destination: &Path) -> std::path::PathBuf {
    let mut parent = from;
    let mut prefix = std::path::PathBuf::new();
    loop {
        if let Ok(remainder) = destination.strip_prefix(parent) {
            return prefix.join(remainder);
        }
        let Some(next) = parent.parent() else {
            return destination.to_path_buf();
        };
        prefix.push("..");
        parent = next;
    }
}

pub const POWERSHELL: &str = r#"param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
Push-Location $project
try {
    $offline = @()
    foreach ($argument in $Arguments) {
        if ($argument -eq '--') { break }
        if ($argument -eq '--offline') { $offline = @('--offline') }
    }
    $meta = cargo metadata @offline --no-deps --format-version 1 --manifest-path "$project/tools/glory/Cargo.toml" | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'Project tool metadata failed' }
    Write-Host "Cargo target: $($meta.target_directory)"
    $hostLine = rustc -vV | Where-Object { $_.StartsWith('host: ') }
    if ($LASTEXITCODE -or -not $hostLine) { throw 'Cannot determine the Rust host' }
    $native = $hostLine.Substring(6)
    $locked = @('--locked')
    if (-not (Test-Path -LiteralPath "$project/Cargo.lock") -or -not (Select-String -LiteralPath "$project/Cargo.lock" -Pattern '^name = "glory-cli"$' -Quiet)) {
        Write-Host 'Resolving the project CLI into Cargo.lock for the first time; commit that lockfile.'
        $locked = @()
    }
    cargo run @locked @offline --manifest-path "$project/tools/glory/Cargo.toml" --target $native -- @Arguments
    if ($LASTEXITCODE) { throw "Glory command exited with $LASTEXITCODE" }
} finally { Pop-Location }
"#;

pub const SHELL: &str = r#"#!/bin/sh
set -eu
project=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project"
offline=
for argument in "$@"; do
    [ "$argument" != -- ] || break
    if [ "$argument" = --offline ]; then offline=--offline; fi
done
cargo metadata $offline --no-deps --format-version 1 --manifest-path tools/glory/Cargo.toml | python3 -c 'import json,sys; print("Cargo target:",json.load(sys.stdin)["target_directory"])'
native=$(rustc -vV | sed -n 's/^host: //p')
[ -n "$native" ] || { echo 'Cannot determine the Rust host' >&2; exit 1; }
locked=--locked
if [ ! -f Cargo.lock ] || ! grep -q '^name = "glory-cli"$' Cargo.lock; then
    echo 'Resolving the project CLI into Cargo.lock for the first time; commit that lockfile.'
    locked=
fi
exec cargo run $locked $offline --manifest-path tools/glory/Cargo.toml --target "$native" -- "$@"
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    #[test]
    fn init_preserves_comments_existing_pins_and_is_repeatable_without_compiling_the_app() {
        let project = Scratch::new();
        project.write(
            "Cargo.toml",
            "# keep this comment\n[package]\nname='init-case'\nversion='0.0.0'\nedition='2024'\n",
        );
        project.write("src/main.rs", "this deliberately does not compile");
        let manifest = project.0.join("Cargo.toml");
        init(
            &manifest,
            false,
            Some(Path::new(env!("CARGO_MANIFEST_DIR"))),
        )
        .unwrap();
        let original = fs::read_to_string(&manifest).unwrap();
        let tool = fs::read_to_string(project.0.join("tools/glory/Cargo.toml")).unwrap();
        assert!(original.contains("# keep this comment"));
        let dependency = tool.parse::<DocumentMut>().unwrap();
        let dependency = dependency["dependencies"]["glory-cli"]["path"]
            .as_str()
            .unwrap();
        assert_eq!(
            project
                .0
                .join("tools/glory")
                .join(dependency)
                .canonicalize()
                .unwrap(),
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .canonicalize()
                .unwrap()
        );
        init(&manifest, false, None).unwrap();
        assert_eq!(fs::read_to_string(&manifest).unwrap(), original);
        assert_eq!(
            fs::read_to_string(project.0.join("tools/glory/Cargo.toml")).unwrap(),
            tool
        );
    }

    #[test]
    fn init_from_a_member_updates_its_workspace_not_a_nested_workspace() {
        let project = Scratch::new();
        project.write("Cargo.toml", "[workspace]\nmembers=['app']\nresolver='2'\n");
        project.write(
            "app/Cargo.toml",
            "[package]\nname='member-case'\nversion='0.0.0'\nedition='2024'\n",
        );
        project.write("app/src/main.rs", "fn main() {}\n");
        init(
            &project.0.join("app/Cargo.toml"),
            false,
            Some(Path::new(env!("CARGO_MANIFEST_DIR"))),
        )
        .unwrap();
        let root = fs::read_to_string(project.0.join("Cargo.toml"))
            .unwrap()
            .parse::<DocumentMut>()
            .unwrap();
        assert_eq!(
            root["workspace"]["default-members"][0].as_str(),
            Some("app")
        );
        assert!(project.0.join("tools/glory/src/main.rs").exists());
        assert!(
            !fs::read_to_string(project.0.join("app/Cargo.toml"))
                .unwrap()
                .contains("[workspace]")
        );
    }

    #[test]
    fn launcher_conflict_rejects_before_changing_the_manifest_or_creating_a_tool_package() {
        let project = Scratch::new();
        let original = "[package]\nname='conflict-case'\nversion='0.0.0'\nedition='2024'\n";
        project.write("Cargo.toml", original);
        project.write("src/main.rs", "fn main() {}\n");
        project.write("bin/glory", "user command\n");
        assert!(init(&project.0.join("Cargo.toml"), false, None).is_err());
        assert_eq!(
            fs::read_to_string(project.0.join("Cargo.toml")).unwrap(),
            original
        );
        assert_eq!(
            fs::read_to_string(project.0.join("bin/glory")).unwrap(),
            "user command\n"
        );
        assert!(!project.0.join("tools/glory").exists());
    }
}
