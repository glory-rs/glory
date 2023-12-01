use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;

use brotli::CompressorWriter;
use camino::{Utf8Path, Utf8PathBuf};
use tokio::process::Child;
use tokio::{process::Command, sync::broadcast, task::JoinHandle};
use wasm_bindgen_cli_support::Bindgen;

use super::ChangeSet;
use crate::config::Project;
use crate::ext::fs;
use crate::ext::sync::{wait_interruptible, CommandResult};
use crate::service::site::SiteFile;
use crate::signal::{Interrupt, Outcome, Product};
use crate::{
    ext::{
        anyhow::{Context, Result},
        exe::Exe,
    },
    logger::GRAY,
};

pub async fn front(proj: &Arc<Project>, changes: &ChangeSet) -> JoinHandle<Result<Outcome<Product>>> {
    let proj = proj.clone();
    let changes = changes.clone();
    tokio::spawn(async move {
        if !changes.need_front_build() {
            log::trace!("Front no changes to rebuild");
            return Ok(Outcome::Success(Product::None));
        }

        fs::create_dir_all(&proj.site.root_relative_pkg_dir()).await?;

        let (envs, line, process) = front_cargo_process("build", true, &proj)?;

        match wait_interruptible("Cargo", process, Interrupt::subscribe_any()).await? {
            CommandResult::Interrupted => return Ok(Outcome::Stopped),
            CommandResult::Failure(_) => return Ok(Outcome::Failed),
            _ => {}
        }
        log::debug!("Cargo envs: {}", GRAY.paint(envs));
        log::info!("Cargo finished {}", GRAY.paint(line));

        bindgen(&proj).await.dot()
    })
}

pub fn front_cargo_process(cmd: &str, wasm: bool, proj: &Project) -> Result<(String, String, Child)> {
    let mut command = Command::new("cargo");
    let (envs, line) = build_cargo_front_cmd(cmd, wasm, proj, &mut command);
    Ok((envs, line, command.spawn()?))
}

pub fn build_cargo_front_cmd(cmd: &str, wasm: bool, proj: &Project, command: &mut Command) -> (String, String) {
    let mut args = vec![
        cmd.to_string(),
        format!("--package={}", proj.lib.name.as_str()),
        // "--lib".to_string(),
        "--target-dir=target/front".to_string(),
    ];
    if wasm {
        args.push("--target=wasm32-unknown-unknown".to_string());
    }

    if !proj.lib.default_features {
        args.push("--no-default-features".to_string());
    }

    if !proj.lib.features.is_empty() {
        args.push(format!("--features={}", proj.lib.features.join(",")));
    }

    proj.lib.profile.add_to_args(&mut args);

    let envs = proj.to_envs();

    let envs_str = envs.iter().map(|(name, val)| format!("{name}={val}")).collect::<Vec<_>>().join(" ");

    command.args(&args).envs(envs);
    let line = format!("cargo {}", args.join(" "));
    println!("Build front: {}", line);
    (envs_str, line)
}

async fn bindgen(proj: &Project) -> Result<Outcome<Product>> {
    let wasm_file = &proj.lib.wasm_file;
    let interrupt = Interrupt::subscribe_any();

    log::info!("Front compiling WASM");

    // see:
    // https://github.com/rustwasm/wasm-bindgen/blob/main/crates/cli-support/src/lib.rs#L95
    // https://github.com/rustwasm/wasm-bindgen/blob/main/crates/cli/src/bin/wasm-bindgen.rs#L13
    let mut bindgen = Bindgen::new().input_path(&wasm_file.source).web(true).dot()?.generate_output().dot()?;

    bindgen.wasm_mut().emit_wasm_file(&wasm_file.dest).dot()?;
    log::info!("Front wrote wasm to {:?}", wasm_file.dest.as_str());
    if proj.release {
        match optimize(&wasm_file.dest, interrupt).await.dot()? {
            CommandResult::Interrupted => return Ok(Outcome::Stopped),
            CommandResult::Failure(_) => return Ok(Outcome::Failed),
            _ => {}
        }
        let data = fs::read(&wasm_file.dest).await?;
    }

    let br_file = File::create(format!("{}.br", wasm_file.dest.as_str()))?;
    let mut br_writer = CompressorWriter::new(
        br_file,
        32 * 1024, // 32 KiB buffer
        11,        // BROTLI_PARAM_QUALITY
        22,        // BROTLI_PARAM_LGWIN
    );
    br_writer.write_all(&data)?;

    let zstd_data = zstd::encode_all(&*data, 21)?;
    let mut zstd_file = File::create(format!("{}.zst", wasm_file.dest.as_str()))?;
    zstd_file.write_all(&zstd_data)?;

    let mut js_changed = false;

    js_changed |= write_snippets(proj, bindgen.snippets()).await?;

    js_changed |= write_modules(proj, bindgen.local_modules()).await?;

    let wasm_changed = proj.site.did_file_change(&proj.lib.wasm_file.as_site_file()).await.dot()?;
    js_changed |= proj.site.updated_with(&proj.lib.js_file, bindgen.js().as_bytes()).await.dot()?;
    log::info!("Front js changed: {js_changed}");
    log::info!("Front wasm changed: {wasm_changed}");

    if js_changed || wasm_changed {
        Ok(Outcome::Success(Product::Front))
    } else {
        Ok(Outcome::Success(Product::None))
    }
}

async fn optimize(file: &Utf8Path, interrupt: broadcast::Receiver<()>) -> Result<CommandResult<()>> {
    let wasm_opt = Exe::WasmOpt.get().await.dot()?;

    let args = [file.as_str(), "-Os", "-o", file.as_str()];
    log::info!("Front optimizing wasm: {} {}", wasm_opt.display(), args.join(" "));
    let process = Command::new(wasm_opt).args(args).spawn().context("Could not spawn command")?;
    wait_interruptible("wasm-opt", process, interrupt).await
}

async fn write_snippets(proj: &Project, snippets: &HashMap<String, Vec<String>>) -> Result<bool> {
    let mut js_changed = false;

    // Provide inline JS files
    for (identifier, list) in snippets.iter() {
        for (i, js) in list.iter().enumerate() {
            let name = format!("inline{}.js", i);
            let site_path = Utf8PathBuf::from("snippets").join(identifier).join(name);
            let file_path = proj.site.root_relative_pkg_dir().join(&site_path);

            fs::create_dir_all(file_path.parent().unwrap()).await?;

            let site_file = SiteFile {
                dest: file_path,
                site: site_path,
            };

            js_changed |= proj.site.updated_with(&site_file, js.as_bytes()).await?;
        }
    }
    Ok(js_changed)
}

async fn write_modules(proj: &Project, modules: &HashMap<String, String>) -> Result<bool> {
    let mut js_changed = false;
    // Provide snippet files from JS snippets
    for (path, js) in modules.iter() {
        let site_path = Utf8PathBuf::from("snippets").join(path);
        let file_path = proj.site.root_relative_pkg_dir().join(&site_path);

        fs::create_dir_all(file_path.parent().unwrap()).await?;

        let site_file = SiteFile {
            dest: file_path,
            site: site_path,
        };

        js_changed |= proj.site.updated_with(&site_file, js.as_bytes()).await?;
    }
    Ok(js_changed)
}
