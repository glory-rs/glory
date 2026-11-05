//! Optional, explicitly pinned Binaryen. Never download or discover a PATH tool.
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Result;
use crate::args::Args;
use crate::project::Project;

pub fn verify(project: &Project, args: &Args) -> Result<Option<PathBuf>> {
    let Some(path) = &args.wasm_opt else {
        return Ok(None);
    };
    let version = args
        .wasm_opt_version
        .as_deref()
        .ok_or("missing wasm-opt version")?;
    let path = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut command = Command::new(&path);
    command.arg("--version");
    project.paths.apply(&mut command);
    let output = project.capture(&mut command)?;
    if !version_matches(&String::from_utf8_lossy(&output.stdout), version) {
        return Err(format!(
            "expected wasm-opt version {version}, found {:?} at {}",
            String::from_utf8_lossy(&output.stdout).trim(),
            path.display()
        ));
    }
    Ok(Some(path))
}

fn version_matches(output: &str, version: &str) -> bool {
    let mut words = output.split_whitespace();
    words.next() == Some("wasm-opt")
        && words.next() == Some("version")
        && words.next() == Some(version)
}

pub fn optimize(project: &Project, tool: &Path, stage: &Path) -> Result<()> {
    let mut modules = Vec::new();
    for entry in fs::read_dir(stage).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_file()
            && entry.path().extension().is_some_and(|ext| ext == "wasm")
        {
            modules.push(entry.path());
        }
    }
    if modules.is_empty() {
        return Err("wasm-bindgen produced no WASM to optimize".into());
    }
    for module in modules {
        let output = module.with_extension("optimized");
        let mut command = Command::new(tool);
        command
            .arg(&module)
            .args([
                "-Oz",
                "--enable-reference-types",
                "--enable-bulk-memory",
                "--enable-mutable-globals",
                "--enable-nontrapping-float-to-int",
                "--strip-debug",
                "-o",
            ])
            .arg(&output);
        project.paths.apply(&mut command);
        project.status(&mut command)?;
        let mut header = [0; 8];
        fs::File::open(&output)
            .and_then(|mut file| file.read_exact(&mut header))
            .map_err(|e| format!("wasm-opt produced no complete module: {e}"))?;
        if &header != b"\0asm\x01\0\0\0" {
            return Err("wasm-opt produced an invalid WASM header".into());
        }
        // The outer build owns this staging directory. Failure never publishes it.
        fs::copy(&output, &module).map_err(|e| e.to_string())?;
        fs::remove_file(output).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_check_requires_the_exact_binaryen_release() {
        assert!(version_matches("wasm-opt version 133\n", "133"));
        assert!(version_matches("wasm-opt version 133 (version_133)", "133"));
        for output in [
            "wasm-opt version 1330",
            "wasm-opt version 133-dev",
            "other version 133",
            "",
        ] {
            assert!(!version_matches(output, "133"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn failed_or_invalid_optimizer_output_does_not_replace_the_input_module() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = crate::test_support::Scratch::new();
        scratch.write("stage/app.wasm", "unoptimized module");
        let tool = scratch.0.join("fail-tool");
        let project = scratch.project(crate::project::Platform::Web);
        for body in [
            "exit 1",
            "exit 0",
            "for arg do out=$arg; done; printf invalid > \"$out\"",
            "for arg do out=$arg; done; printf invalid-wasm > \"$out\"",
        ] {
            scratch.write("fail-tool", &format!("#!/bin/sh\n{body}\n"));
            fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
            assert!(optimize(&project, &tool, &scratch.0.join("stage")).is_err());
            assert_eq!(
                fs::read_to_string(scratch.0.join("stage/app.wasm")).unwrap(),
                "unoptimized module"
            );
        }
    }
}
