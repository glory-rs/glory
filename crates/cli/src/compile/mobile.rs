//! Mobile (Android / iOS) library compilation.
//!
//! `glory build --target android` compiles the project's lib package as a
//! `cdylib` for `aarch64-linux-android` through `cargo-ndk`;
//! `--target ios` compiles a `staticlib` for `aarch64-apple-ios` (macOS
//! host only). The produced artifact is consumed by a host mobile project
//! — see `crates/cli/templates/mobile/README.md` for the wiring.

use std::sync::Arc;

use super::ChangeSet;
use crate::{
    config::{BuildTarget, Project},
    ext::anyhow::{Result, anyhow},
    ext::sync::{CommandResult, wait_interruptible},
    logger::GRAY,
    signal::{Interrupt, Outcome, Product},
};
use tokio::{process::Command, task::JoinHandle};

pub async fn mobile(proj: &Arc<Project>, changes: &ChangeSet) -> JoinHandle<Result<Outcome<Product>>> {
    let proj = proj.clone();
    let changes = changes.clone();

    tokio::spawn(async move {
        if !changes.need_server_build() {
            return Ok(Outcome::Success(Product::None));
        }
        let triple = proj
            .mobile_target_triple()
            .ok_or_else(|| anyhow!("mobile compile invoked for non-mobile target"))?;

        let mut command = match proj.target {
            BuildTarget::Android => {
                if which_cargo_subcommand("ndk").await.is_err() {
                    return Err(anyhow!(
                        "Android builds need cargo-ndk and an NDK toolchain.\n  install: cargo install cargo-ndk && rustup target add {triple}\n  (set ANDROID_NDK_HOME to your NDK root)"
                    ));
                }
                let mut command = Command::new("cargo");
                command.args([
                    "ndk",
                    "-t",
                    "arm64-v8a",
                    "-o",
                    proj.site.root_dir.join("android/jniLibs").as_str(),
                    "build",
                ]);
                command
            }
            BuildTarget::Ios => {
                if !cfg!(target_os = "macos") {
                    return Err(anyhow!("iOS builds require a macOS host with Xcode (rustup target add {triple})"));
                }
                let mut command = Command::new("cargo");
                command.args(["build", &format!("--target={triple}")]);
                command
            }
            _ => unreachable!("guarded by mobile_target_triple"),
        };

        command.arg(format!("--package={}", proj.lib.name));
        command.arg("--lib");
        command.arg("--target-dir=target/mobile");
        if proj.release {
            command.arg("--release");
        }
        if !proj.lib.features.is_empty() {
            command.arg(format!("--features={}", proj.lib.features.join(",")));
        }
        command.envs(proj.to_envs());

        let line = format!("{:?}", command.as_std());
        log::info!("Mobile compiling {:?} {}", proj.target, GRAY.paint(&line));

        match wait_interruptible("Cargo (mobile)", command.spawn()?, Interrupt::subscribe_any()).await? {
            CommandResult::Success(_) => {
                log::info!("Mobile lib build finished {}", GRAY.paint(line));
                Ok(Outcome::Success(Product::None))
            }
            CommandResult::Interrupted => Ok(Outcome::Stopped),
            CommandResult::Failure(_) => Ok(Outcome::Failed),
        }
    })
}

/// Checks that `cargo <sub>` exists by asking cargo to run it with
/// `--version` (cheap, no build).
async fn which_cargo_subcommand(sub: &str) -> Result<()> {
    let status = Command::new("cargo").arg(sub).arg("--version").output().await?;
    if status.status.success() {
        Ok(())
    } else {
        Err(anyhow!("cargo {sub} unavailable"))
    }
}
