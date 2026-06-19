use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::fs::File;
use std::io::{Cursor, Write};
use std::sync::Arc;

use brotli::CompressorWriter;
use camino::{Utf8Path, Utf8PathBuf};
use flate2::write::GzEncoder;
use serde::Serialize;

use crate::config::{BuildTarget, Config, Project};
use crate::ext::anyhow::{Context, Result, anyhow, bail};
use crate::ext::fs;
use crate::ext::sync::{CommandResult, wait_interruptible};
use crate::logger::GRAY;
use crate::signal::Interrupt;
use tokio::process::Command;

/// Output folder collecting the distributable artifacts.
const DIST_DIR: &str = "dist";

pub async fn bundle_all(conf: &Config, optimize_images: bool) -> Result<()> {
    for proj in &conf.projects {
        bundle_proj(proj, optimize_images).await?;
    }
    Ok(())
}

/// Build the project and collect the artifacts into `dist/<name>/`.
pub async fn bundle_proj(proj: &Arc<Project>, optimize_images: bool) -> Result<()> {
    if !proj.release {
        log::warn!("Bundling a debug build. Pass --release for an optimized distributable.");
    }

    if !super::build::build_proj(proj).await.dot()? {
        bail!("Build failed; nothing to bundle");
    }

    let dist = prepare_dist(&proj.name).await?;

    match proj.target {
        BuildTarget::Web => bundle_web(proj, &dist).await?,
        BuildTarget::Desktop | BuildTarget::Native => bundle_hosted_binary(proj, &dist).await?,
        BuildTarget::Android => bundle_android(proj, &dist).await?,
        BuildTarget::Ios => bundle_ios(proj, &dist).await?,
    }

    let mut asset_map = BTreeMap::new();
    if optimize_images {
        write_optimized_image_assets(proj.target, &dist, &mut asset_map).await?;
    }
    write_hashed_asset_copies(proj.target, &dist, &mut asset_map).await?;
    optimize_static_assets(&dist).await?;
    if proj.target == BuildTarget::Desktop {
        bundle_desktop_installers(proj, &dist).await?;
    }
    write_manifest(proj, &dist, &asset_map).await?;

    log::info!("Glory bundled {} into {}", proj.name, GRAY.paint(dist.as_str()));
    Ok(())
}

async fn prepare_dist(name: &str) -> Result<Utf8PathBuf> {
    let dist = Utf8PathBuf::from(DIST_DIR).join(name);
    if dist.exists() {
        fs::remove_dir_all(&dist).await.dot()?;
    }
    fs::create_dir_all(&dist).await.dot()?;
    Ok(dist)
}

async fn bundle_web(proj: &Project, dist: &Utf8Path) -> Result<()> {
    copy_site_if_present(proj, &dist.join("public")).await?;
    copy_server_binary(proj, dist).await?;
    Ok(())
}

async fn bundle_hosted_binary(proj: &Project, dist: &Utf8Path) -> Result<()> {
    copy_site_if_present(proj, dist).await?;
    copy_server_binary(proj, dist).await?;
    Ok(())
}

async fn bundle_desktop_installers(proj: &Project, dist: &Utf8Path) -> Result<()> {
    if !proj.builds_server() {
        return Ok(());
    }

    if cfg!(target_os = "windows") {
        write_windows_msi_artifacts(proj, dist).await?;
    }
    if cfg!(target_os = "linux") {
        write_linux_deb(proj, dist).await?;
    }

    Ok(())
}

async fn write_windows_msi_artifacts(proj: &Project, dist: &Utf8Path) -> Result<()> {
    let out_dir = dist.join("installers/windows");
    let staging = out_dir.join("staging");
    let obj_dir = out_dir.join("obj");
    if staging.exists() {
        fs::remove_dir_all(&staging).await.dot()?;
    }
    if obj_dir.exists() {
        fs::remove_dir_all(&obj_dir).await.dot()?;
    }
    fs::create_dir_all(&staging).await.dot()?;
    fs::create_dir_all(&obj_dir).await.dot()?;
    copy_bundle_payload(dist, &staging).await?;

    let product_name = installer_product_name(&proj.name);
    let version = msi_version(&proj.bin.version);
    let manufacturer = xml_escape(&installer_publisher());
    let upgrade_code = deterministic_guid(&format!("glory:{}:{}", proj.name, proj.bin.name));
    let product_wxs = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="{product_name}" Language="1033" Version="{version}" Manufacturer="{manufacturer}" UpgradeCode="{upgrade_code}">
    <Package InstallerVersion="500" Compressed="yes" InstallScope="perMachine" />
    <MajorUpgrade DowngradeErrorMessage="A newer version of {product_name} is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <Feature Id="DefaultFeature" Title="{product_name}" Level="1">
      <ComponentGroupRef Id="AppFiles" />
    </Feature>
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="{product_name}" />
      </Directory>
    </Directory>
  </Product>
</Wix>
"#
    );
    fs::write(out_dir.join("product.wxs"), product_wxs).await.dot()?;

    let msi_name = format!("{}_{}_x64.msi", package_file_stem(&proj.name), version);
    fs::write(
        out_dir.join("build-msi.ps1"),
        format!(
            r#"$ErrorActionPreference = "Stop"
$Here = Split-Path -Parent $MyInvocation.MyCommand.Path
$WixBin = $env:WIX_BIN
if ($WixBin) {{
  $Heat = Join-Path $WixBin "heat.exe"
  $Candle = Join-Path $WixBin "candle.exe"
  $Light = Join-Path $WixBin "light.exe"
}} else {{
  $Heat = "heat.exe"
  $Candle = "candle.exe"
  $Light = "light.exe"
}}
& $Heat dir (Join-Path $Here "staging") -cg AppFiles -dr INSTALLFOLDER -srd -gg -sfrag -out (Join-Path $Here "app-files.wxs")
& $Candle (Join-Path $Here "product.wxs") (Join-Path $Here "app-files.wxs") -out (Join-Path $Here "obj\")
& $Light (Join-Path $Here "obj\product.wixobj") (Join-Path $Here "obj\app-files.wixobj") -out (Join-Path $Here "{msi_name}")
"#
        ),
    )
    .await
    .dot()?;

    let Some((heat, candle, light)) = wix_tools() else {
        log::warn!(
            "WiX toolset not found; wrote MSI sources and build-msi.ps1 under {}",
            GRAY.paint(out_dir.as_str())
        );
        return Ok(());
    };

    let app_files = out_dir.join("app-files.wxs");
    let mut heat_cmd = Command::new(heat);
    heat_cmd
        .args([
            "dir",
            staging.as_str(),
            "-cg",
            "AppFiles",
            "-dr",
            "INSTALLFOLDER",
            "-srd",
            "-gg",
            "-sfrag",
            "-out",
        ])
        .arg(app_files.as_str());
    run_checked("WiX heat", heat_cmd).await?;

    let mut candle_cmd = Command::new(candle);
    candle_cmd
        .arg(out_dir.join("product.wxs").as_str())
        .arg(app_files.as_str())
        .arg("-out")
        .arg(obj_dir.join("").as_str());
    run_checked("WiX candle", candle_cmd).await?;

    let mut light_cmd = Command::new(light);
    light_cmd
        .arg(obj_dir.join("product.wixobj").as_str())
        .arg(obj_dir.join("app-files.wixobj").as_str())
        .arg("-out")
        .arg(out_dir.join(msi_name).as_str());
    run_checked("WiX light", light_cmd).await
}

async fn write_linux_deb(proj: &Project, dist: &Utf8Path) -> Result<()> {
    let package = debian_package_name(&proj.name);
    let version = proj.bin.version.clone();
    let arch = debian_arch();
    let out_dir = dist.join("installers/linux");
    fs::create_dir_all(&out_dir).await.dot()?;

    let data_tar = build_deb_data_tar(proj, dist, &package)?;
    let installed_size = (data_tar.len() as u64).div_ceil(1024).max(1);
    let control_tar = build_deb_control_tar(proj, &package, &version, arch, installed_size)?;
    let deb_path = out_dir.join(format!("{package}_{version}_{arch}.deb"));
    write_deb_archive(&deb_path, &control_tar, &data_tar)?;
    Ok(())
}

async fn bundle_android(proj: &Project, dist: &Utf8Path) -> Result<()> {
    let jni_libs = proj.site.root_dir.join("android").join("jniLibs");
    if !jni_libs.exists() {
        bail!("Android bundle missing {}; run `glory build --target android` first", jni_libs);
    }
    fs::copy_dir_all(&jni_libs, dist.join("android/jniLibs")).await.dot()?;
    let android_project = find_android_project(proj).ok_or_else(|| {
        anyhow!(
            "Android host project missing. Expected `android/settings.gradle.kts` or `android/app/build.gradle.kts` under {}",
            proj.working_dir
        )
    })?;
    run_android_gradle(proj, &android_project).await?;
    let apk_dir = dist.join("android/apk");
    let apks = copy_android_apks(&android_project, &apk_dir).await?;
    write_android_scripts(&apks, &android_project, &dist.join("android")).await?;
    maybe_run_android_app(&android_project).await?;
    copy_site_if_present(proj, &dist.join("site")).await?;
    Ok(())
}

async fn bundle_ios(proj: &Project, dist: &Utf8Path) -> Result<()> {
    let triple = proj
        .mobile_target_triple()
        .ok_or_else(|| anyhow!("iOS bundle invoked for non-iOS target"))?;
    let artifact_dir = Utf8PathBuf::from("target/mobile").join(triple).join(proj.lib.profile.to_string());
    if !artifact_dir.exists() {
        bail!("iOS bundle missing {}; run `glory build --target ios` first", artifact_dir);
    }
    fs::copy_dir_all(&artifact_dir, dist.join("ios/lib")).await.dot()?;
    let ios_project = find_ios_project(proj).ok_or_else(|| {
        anyhow!(
            "iOS host project missing. Expected `ios/project.yml` or an `.xcodeproj` under {}",
            proj.working_dir
        )
    })?;
    run_ios_packaging(proj, &ios_project).await?;
    copy_ios_products(&ios_project, &dist.join("ios")).await?;
    write_ios_scripts(&ios_project, &dist.join("ios")).await?;
    copy_site_if_present(proj, &dist.join("site")).await?;
    Ok(())
}

fn find_android_project(proj: &Project) -> Option<Utf8PathBuf> {
    [proj.working_dir.join("android"), proj.site.root_dir.join("android")]
        .into_iter()
        .find(|dir| has_android_project(dir))
}

fn has_android_project(dir: &Utf8Path) -> bool {
    dir.join("settings.gradle.kts").exists()
        || dir.join("settings.gradle").exists()
        || dir.join("app/build.gradle.kts").exists()
        || dir.join("app/build.gradle").exists()
}

async fn run_android_gradle(proj: &Project, android_project: &Utf8Path) -> Result<()> {
    let program = gradle_program(android_project)?;
    let tasks = android_gradle_tasks(proj.release);
    let mut command = Command::new(program);
    command.current_dir(android_project).args(&tasks).envs(proj.to_envs());
    if let Ok(serial) = env::var("GLORY_ANDROID_DEVICE")
        && !serial.trim().is_empty()
    {
        command.env("ANDROID_SERIAL", serial);
    }
    run_checked("Gradle (android)", command).await
}

fn gradle_program(android_project: &Utf8Path) -> Result<String> {
    let windows_wrapper = android_project.join("gradlew.bat");
    if windows_wrapper.exists() {
        return Ok(windows_wrapper.to_string());
    }
    let unix_wrapper = android_project.join("gradlew");
    if unix_wrapper.exists() {
        return Ok(unix_wrapper.to_string());
    }
    which::which("gradle").context("Android packaging needs `android/gradlew` or `gradle` on PATH")?;
    Ok("gradle".to_owned())
}

fn android_gradle_tasks(release: bool) -> Vec<String> {
    let variant = android_variant(release);
    let mut tasks = env_list("GLORY_ANDROID_GRADLE_TASK").unwrap_or_else(|| vec![format!("assemble{variant}")]);
    if env_flag("GLORY_ANDROID_INSTALL") {
        tasks.push(format!("install{variant}"));
    }
    tasks
}

fn android_variant(release: bool) -> &'static str {
    if release { "Release" } else { "Debug" }
}

async fn copy_android_apks(android_project: &Utf8Path, dest: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    let apk_root = android_project.join("app/build/outputs/apk");
    let apks = collect_files_with_extension(&apk_root, "apk")?;
    if apks.is_empty() {
        bail!("Gradle finished but no APK was found under {}", apk_root);
    }
    fs::create_dir_all(dest).await.dot()?;
    let mut copied = Vec::with_capacity(apks.len());
    for apk in apks {
        let file_name = apk.file_name().ok_or_else(|| anyhow!("APK path has no file name: {apk}"))?;
        let to = dest.join(file_name);
        fs::copy(&apk, &to).await.dot()?;
        copied.push(to);
    }
    Ok(copied)
}

async fn write_android_scripts(apks: &[Utf8PathBuf], android_project: &Utf8Path, dist_android: &Utf8Path) -> Result<()> {
    let Some(apk) = apks.first() else {
        return Ok(());
    };
    let apk_name = apk.file_name().ok_or_else(|| anyhow!("APK path has no file name: {apk}"))?;
    let app_id = android_application_id(android_project).unwrap_or_else(|| "com.example.glory_app".to_owned());
    let activity = android_launcher_activity(android_project, &app_id).unwrap_or_else(|| format!("{app_id}.MainActivity"));
    let component = format!("{app_id}/{activity}");

    fs::write(
        dist_android.join("install.ps1"),
        format!(
            r#"param([string]$Serial = $env:GLORY_ANDROID_DEVICE)
$apk = Join-Path $PSScriptRoot "apk\{apk_name}"
$adbArgs = @()
if ($Serial) {{ $adbArgs += @("-s", $Serial) }}
& adb @adbArgs install -r $apk
"#
        ),
    )
    .await
    .dot()?;
    fs::write(
        dist_android.join("run.ps1"),
        format!(
            r#"param([string]$Serial = $env:GLORY_ANDROID_DEVICE)
$adbArgs = @()
if ($Serial) {{ $adbArgs += @("-s", $Serial) }}
& adb @adbArgs shell am start -n "{component}"
"#
        ),
    )
    .await
    .dot()?;
    fs::write(
        dist_android.join("install.sh"),
        format!(
            r#"#!/usr/bin/env sh
set -eu
DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
ADB="${{ADB:-adb}}"
SERIAL_ARG=""
if [ -n "${{GLORY_ANDROID_DEVICE:-}}" ]; then SERIAL_ARG="-s $GLORY_ANDROID_DEVICE"; fi
# shellcheck disable=SC2086
"$ADB" $SERIAL_ARG install -r "$DIR/apk/{apk_name}"
"#
        ),
    )
    .await
    .dot()?;
    fs::write(
        dist_android.join("run.sh"),
        format!(
            r#"#!/usr/bin/env sh
set -eu
ADB="${{ADB:-adb}}"
SERIAL_ARG=""
if [ -n "${{GLORY_ANDROID_DEVICE:-}}" ]; then SERIAL_ARG="-s $GLORY_ANDROID_DEVICE"; fi
# shellcheck disable=SC2086
"$ADB" $SERIAL_ARG shell am start -n "{component}"
"#
        ),
    )
    .await
    .dot()?;
    Ok(())
}

async fn maybe_run_android_app(android_project: &Utf8Path) -> Result<()> {
    if !env_flag("GLORY_ANDROID_RUN") {
        return Ok(());
    }
    let app_id = android_application_id(android_project)
        .ok_or_else(|| anyhow!("GLORY_ANDROID_RUN needs an applicationId in android/app/build.gradle(.kts)"))?;
    let activity = android_launcher_activity(android_project, &app_id).unwrap_or_else(|| format!("{app_id}.MainActivity"));
    let mut command = Command::new("adb");
    if let Ok(serial) = env::var("GLORY_ANDROID_DEVICE")
        && !serial.trim().is_empty()
    {
        command.args(["-s", serial.trim()]);
    }
    command.args(["shell", "am", "start", "-n", &format!("{app_id}/{activity}")]);
    run_checked("adb (android run)", command).await
}

fn android_application_id(android_project: &Utf8Path) -> Option<String> {
    let gradle = read_first_existing(&[android_project.join("app/build.gradle.kts"), android_project.join("app/build.gradle")])?;
    quoted_value_after(&gradle, "applicationId").or_else(|| quoted_value_after(&gradle, "namespace"))
}

fn android_launcher_activity(android_project: &Utf8Path, app_id: &str) -> Option<String> {
    let manifest = std::fs::read_to_string(android_project.join("app/src/main/AndroidManifest.xml")).ok()?;
    let name = quoted_value_after(&manifest, "android:name")?;
    Some(if let Some(rest) = name.strip_prefix('.') {
        format!("{app_id}.{rest}")
    } else if name.contains('.') {
        name
    } else {
        format!("{app_id}.{name}")
    })
}

fn find_ios_project(proj: &Project) -> Option<Utf8PathBuf> {
    [proj.working_dir.join("ios"), proj.site.root_dir.join("ios")]
        .into_iter()
        .find(|dir| has_ios_project(dir))
}

fn has_ios_project(dir: &Utf8Path) -> bool {
    dir.join("project.yml").exists() || first_child_with_extension(dir, "xcodeproj").is_some()
}

async fn run_ios_packaging(proj: &Project, ios_project: &Utf8Path) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("iOS packaging requires macOS with Xcode and XcodeGen");
    }

    let scheme = ios_scheme(proj, ios_project);
    if ios_project.join("project.yml").exists() {
        ensure_program("xcodegen", "install XcodeGen and run `xcodegen generate`")?;
        let mut command = Command::new("xcodegen");
        command.current_dir(ios_project).arg("generate");
        run_checked("XcodeGen (ios)", command).await?;
    }

    ensure_program("xcodebuild", "install Xcode from the App Store or developer.apple.com")?;
    let xcodeproj = find_xcodeproj(ios_project, &scheme).ok_or_else(|| anyhow!("No .xcodeproj found under {}", ios_project))?;
    let configuration = if proj.release { "Release" } else { "Debug" };
    let sdk = env::var("GLORY_IOS_SDK").unwrap_or_else(|_| {
        if proj.release {
            "iphoneos".to_owned()
        } else {
            "iphonesimulator".to_owned()
        }
    });
    let derived_data = ios_project.join("build/DerivedData");
    let mut command = Command::new("xcodebuild");
    command
        .current_dir(ios_project)
        .args(["-project", xcodeproj.as_str()])
        .args(["-scheme", &scheme])
        .args(["-configuration", configuration])
        .args(["-sdk", &sdk])
        .args(["-derivedDataPath", derived_data.as_str()]);
    if let Ok(destination) = env::var("GLORY_IOS_DESTINATION")
        && !destination.trim().is_empty()
    {
        command.args(["-destination", destination.trim()]);
    }
    command.arg("build").envs(proj.to_envs());
    run_checked("xcodebuild (ios)", command).await?;

    if env_flag("GLORY_IOS_ARCHIVE") {
        let archive_path = ios_project.join("build").join(format!("{scheme}.xcarchive"));
        let mut archive = Command::new("xcodebuild");
        archive
            .current_dir(ios_project)
            .args(["-project", xcodeproj.as_str()])
            .args(["-scheme", &scheme])
            .args(["-configuration", configuration])
            .args(["-sdk", &sdk])
            .args(["-archivePath", archive_path.as_str()])
            .arg("archive")
            .envs(proj.to_envs());
        run_checked("xcodebuild archive (ios)", archive).await?;
    }
    Ok(())
}

fn ios_scheme(proj: &Project, ios_project: &Utf8Path) -> String {
    env::var("GLORY_IOS_SCHEME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| ios_project_name(ios_project))
        .unwrap_or_else(|| pascal_case(&proj.name))
}

fn ios_project_name(ios_project: &Utf8Path) -> Option<String> {
    let project = std::fs::read_to_string(ios_project.join("project.yml")).ok()?;
    project
        .lines()
        .find_map(|line| line.trim().strip_prefix("name:").map(|value| value.trim().trim_matches('"').to_owned()))
}

fn find_xcodeproj(ios_project: &Utf8Path, scheme: &str) -> Option<Utf8PathBuf> {
    let expected = ios_project.join(format!("{scheme}.xcodeproj"));
    if expected.exists() {
        return Some(expected);
    }
    first_child_with_extension(ios_project, "xcodeproj")
}

async fn copy_ios_products(ios_project: &Utf8Path, dist_ios: &Utf8Path) -> Result<()> {
    let products = ios_project.join("build/DerivedData/Build/Products");
    let apps = collect_dirs_with_extension(&products, "app")?;
    if apps.is_empty() {
        bail!("xcodebuild finished but no .app was found under {}", products);
    }
    let app_dest = dist_ios.join("app");
    fs::create_dir_all(&app_dest).await.dot()?;
    for app in apps {
        let file_name = app.file_name().ok_or_else(|| anyhow!("app bundle path has no file name: {app}"))?;
        fs::copy_dir_all(&app, app_dest.join(file_name)).await.dot()?;
    }

    let archive_dest = dist_ios.join("archive");
    for archive in collect_dirs_with_extension(&ios_project.join("build"), "xcarchive")? {
        fs::create_dir_all(&archive_dest).await.dot()?;
        let file_name = archive.file_name().ok_or_else(|| anyhow!("archive path has no file name: {archive}"))?;
        fs::copy_dir_all(&archive, archive_dest.join(file_name)).await.dot()?;
    }
    Ok(())
}

async fn write_ios_scripts(ios_project: &Utf8Path, dist_ios: &Utf8Path) -> Result<()> {
    let scheme = ios_project_name(ios_project).unwrap_or_else(|| "GloryApp".to_owned());
    let app_name = format!("{scheme}.app");
    let bundle_id = ios_bundle_id(ios_project, &scheme).unwrap_or_else(|| format!("com.example.{scheme}"));
    fs::write(
        dist_ios.join("install-simulator.sh"),
        format!(
            r#"#!/usr/bin/env sh
set -eu
DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
APP="${{1:-$DIR/app/{app_name}}}"
xcrun simctl install booted "$APP"
"#
        ),
    )
    .await
    .dot()?;
    fs::write(
        dist_ios.join("run-simulator.sh"),
        format!(
            r#"#!/usr/bin/env sh
set -eu
xcrun simctl launch booted "{bundle_id}"
"#
        ),
    )
    .await
    .dot()?;
    Ok(())
}

fn ios_bundle_id(ios_project: &Utf8Path, scheme: &str) -> Option<String> {
    let project = std::fs::read_to_string(ios_project.join("project.yml")).ok()?;
    let prefix = project.lines().find_map(|line| {
        line.trim()
            .strip_prefix("bundleIdPrefix:")
            .map(|value| value.trim().trim_matches('"').to_owned())
    })?;
    Some(format!("{prefix}.{scheme}"))
}

async fn run_checked(name: &str, mut command: Command) -> Result<()> {
    let line = format!("{:?}", command.as_std());
    log::info!("Running {} {}", name, GRAY.paint(&line));
    match wait_interruptible(name, command.spawn()?, Interrupt::subscribe_any()).await? {
        CommandResult::Success(_) => Ok(()),
        CommandResult::Interrupted => bail!("{name} interrupted"),
        CommandResult::Failure(_) => bail!("{name} failed: {line}"),
    }
}

async fn copy_site_if_present(proj: &Project, dest: &Utf8Path) -> Result<()> {
    if proj.site.root_dir.exists() {
        fs::copy_dir_all(&proj.site.root_dir, dest).await.dot()?;
    }
    Ok(())
}

async fn copy_server_binary(proj: &Project, dist: &Utf8Path) -> Result<()> {
    if proj.builds_server() && proj.bin.exe_file.exists() {
        let file_name = proj
            .bin
            .exe_file
            .file_name()
            .ok_or_else(|| anyhow!("server exe path has no file name: {}", proj.bin.exe_file))?;
        fs::copy(&proj.bin.exe_file, dist.join(file_name)).await.dot()?;
    }
    Ok(())
}

async fn copy_bundle_payload(dist: &Utf8Path, dest: &Utf8Path) -> Result<()> {
    for file in collect_files(dist)? {
        let rel = file.strip_prefix(dist)?;
        if is_installer_path(rel) {
            continue;
        }
        let to = dest.join(rel);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).await.dot()?;
        }
        fs::copy(&file, &to).await.dot()?;
    }
    Ok(())
}

fn is_installer_path(path: &Utf8Path) -> bool {
    path.iter().next() == Some("installers")
}

fn wix_tools() -> Option<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)> {
    if let Ok(bin) = env::var("WIX_BIN") {
        let dir = std::path::PathBuf::from(bin);
        let heat = dir.join("heat.exe");
        let candle = dir.join("candle.exe");
        let light = dir.join("light.exe");
        if heat.exists() && candle.exists() && light.exists() {
            return Some((heat, candle, light));
        }
    }

    let heat = which::which("heat.exe").or_else(|_| which::which("heat")).ok()?;
    let candle = which::which("candle.exe").or_else(|_| which::which("candle")).ok()?;
    let light = which::which("light.exe").or_else(|_| which::which("light")).ok()?;
    Some((heat, candle, light))
}

fn build_deb_data_tar(proj: &Project, dist: &Utf8Path, package: &str) -> Result<Vec<u8>> {
    let encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let exe_name = proj
        .bin
        .exe_file
        .file_name()
        .ok_or_else(|| anyhow!("desktop executable path has no file name: {}", proj.bin.exe_file))?;
    let lib_root = format!("usr/lib/{package}");

    for file in collect_files(dist)? {
        let rel = file.strip_prefix(dist)?;
        if is_installer_path(rel) {
            continue;
        }
        let path = format!("{lib_root}/{}", rel.as_str().replace('\\', "/"));
        let mode = if rel.file_name() == Some(exe_name) { 0o755 } else { 0o644 };
        append_tar_file(&mut builder, &file, &path, mode)?;
    }

    append_tar_bytes(
        &mut builder,
        &format!("usr/share/applications/{package}.desktop"),
        desktop_entry(proj, package, exe_name).as_bytes(),
        0o644,
    )?;
    append_tar_symlink(&mut builder, &format!("usr/bin/{package}"), &format!("../lib/{package}/{exe_name}"))?;

    let encoder = builder.into_inner()?;
    Ok(encoder.finish()?)
}

fn build_deb_control_tar(proj: &Project, package: &str, version: &str, arch: &str, installed_size: u64) -> Result<Vec<u8>> {
    let encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let control = format!(
        "Package: {package}\nVersion: {version}\nSection: utils\nPriority: optional\nArchitecture: {arch}\nMaintainer: {maintainer}\nInstalled-Size: {installed_size}\nDescription: {description}\n",
        maintainer = installer_publisher(),
        description = debian_single_line(&format!("{} desktop application", proj.name)),
    );
    append_tar_bytes(&mut builder, "control", control.as_bytes(), 0o644)?;
    let encoder = builder.into_inner()?;
    Ok(encoder.finish()?)
}

fn append_tar_file<W: Write>(builder: &mut tar::Builder<W>, source: &Utf8Path, path: &str, mode: u32) -> Result<()> {
    let data = std::fs::read(source)?;
    append_tar_bytes(builder, path, &data, mode)
}

fn append_tar_bytes<W: Write>(builder: &mut tar::Builder<W>, path: &str, data: &[u8], mode: u32) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_path(path)?;
    header.set_size(data.len() as u64);
    header.set_mode(mode);
    header.set_mtime(0);
    header.set_uid(0);
    header.set_gid(0);
    header.set_cksum();
    builder.append(&header, Cursor::new(data))?;
    Ok(())
}

fn append_tar_symlink<W: Write>(builder: &mut tar::Builder<W>, path: &str, target: &str) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_path(path)?;
    header.set_link_name(target)?;
    header.set_size(0);
    header.set_mode(0o777);
    header.set_mtime(0);
    header.set_uid(0);
    header.set_gid(0);
    header.set_cksum();
    builder.append(&header, Cursor::new(Vec::<u8>::new()))?;
    Ok(())
}

fn write_deb_archive(path: &Utf8Path, control_tar: &[u8], data_tar: &[u8]) -> Result<()> {
    let mut file = File::create(path)?;
    file.write_all(b"!<arch>\n")?;
    write_ar_member(&mut file, "debian-binary", b"2.0\n")?;
    write_ar_member(&mut file, "control.tar.gz", control_tar)?;
    write_ar_member(&mut file, "data.tar.gz", data_tar)?;
    Ok(())
}

fn write_ar_member(writer: &mut impl Write, name: &str, data: &[u8]) -> Result<()> {
    let name = format!("{name}/");
    let header = format!("{:<16}{:<12}{:<6}{:<6}{:<8o}{:<10}`\n", name, 0, 0, 0, 0o100644, data.len());
    writer.write_all(header.as_bytes())?;
    writer.write_all(data)?;
    if data.len() % 2 == 1 {
        writer.write_all(b"\n")?;
    }
    Ok(())
}

fn desktop_entry(proj: &Project, package: &str, exe_name: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec=/usr/lib/{package}/{exe_name}\nTerminal=false\nCategories=Utility;\n",
        desktop_value(&installer_product_name(&proj.name)),
    )
}

fn msi_version(version: &str) -> String {
    semver::Version::parse(version)
        .map(|version| format!("{}.{}.{}", version.major, version.minor, version.patch))
        .unwrap_or_else(|_| "0.0.0".to_owned())
}

fn debian_arch() -> &'static str {
    match env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "arm" => "armhf",
        "x86" | "i686" => "i386",
        _ => "all",
    }
}

fn debian_package_name(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in name.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '+' | '.') {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let out = out.trim_matches('-').to_owned();
    if out.is_empty() { "glory-app".to_owned() } else { out }
}

fn package_file_stem(name: &str) -> String {
    debian_package_name(name).replace(['+', '.'], "-")
}

fn installer_product_name(name: &str) -> String {
    let product = pascal_case(name);
    if product.is_empty() { "GloryApp".to_owned() } else { product }
}

fn installer_publisher() -> String {
    env::var("GLORY_BUNDLE_PUBLISHER").unwrap_or_else(|_| "Glory".to_owned())
}

fn debian_single_line(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn desktop_value(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn deterministic_guid(input: &str) -> String {
    let a = seahash::hash(input.as_bytes());
    let b = seahash::hash(format!("{input}:glory").as_bytes());
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (a >> 32) as u32,
        (a >> 16) as u16,
        a as u16,
        (b >> 48) as u16,
        b & 0x0000_ffff_ffff_ffff
    )
}

async fn write_manifest(proj: &Project, dist: &Utf8Path, asset_map: &BTreeMap<String, String>) -> Result<()> {
    let files = bundle_files(dist).await?;
    let manifest = serde_json::json!({
        "name": proj.name,
        "target": format!("{:?}", proj.target).to_lowercase(),
        "release": proj.release,
        "site_root": proj.site.root_dir.as_str(),
        "executable": if proj.bin.exe_file.exists() { Some(proj.bin.exe_file.as_str()) } else { None },
        "asset_map": asset_map,
        "files": files,
    });
    fs::write(dist.join("glory-bundle.json"), serde_json::to_vec_pretty(&manifest)?)
        .await
        .dot()
}

async fn write_optimized_image_assets(target: BuildTarget, dist: &Utf8Path, asset_map: &mut BTreeMap<String, String>) -> Result<()> {
    for file in collect_files(dist)? {
        let rel = file.strip_prefix(dist)?;
        if !should_optimize_image(rel) {
            continue;
        }
        let data = fs::read(&file).await?;
        let webp = encode_webp(&data, rel).context(format!("Optimizing image asset {rel}"))?;
        let Some(webp_rel) = optimized_image_path(rel) else {
            continue;
        };
        fs::write(dist.join(&webp_rel), webp).await.dot()?;
        asset_map.insert(asset_public_path(target, rel), asset_public_path(target, &webp_rel));
    }
    Ok(())
}

async fn write_hashed_asset_copies(target: BuildTarget, dist: &Utf8Path, asset_map: &mut BTreeMap<String, String>) -> Result<()> {
    for file in collect_files(dist)? {
        let rel = file.strip_prefix(dist)?;
        if !should_hash_rewrite(rel) {
            continue;
        }
        let data = fs::read(&file).await?;
        let hash = format!("{:016x}", seahash::hash(&data));
        let Some(hashed_rel) = hashed_asset_path(rel, &hash) else {
            continue;
        };
        let hashed_file = dist.join(&hashed_rel);
        fs::write(&hashed_file, &data).await.dot()?;
        let public = asset_public_path(target, rel);
        let hashed_public = asset_public_path(target, &hashed_rel);
        for mapped in asset_map.values_mut() {
            if *mapped == public {
                *mapped = hashed_public.clone();
            }
        }
        asset_map.entry(public).or_insert(hashed_public);
    }
    Ok(())
}

async fn optimize_static_assets(dist: &Utf8Path) -> Result<()> {
    for file in collect_files(dist)? {
        if !should_precompress(&file) {
            continue;
        }
        let data = fs::read(&file).await?;
        write_gzip(&file, &data)?;
        write_brotli(&file, &data)?;
    }
    Ok(())
}

fn should_precompress(path: &Utf8Path) -> bool {
    matches!(
        path.extension(),
        Some("css" | "html" | "js" | "json" | "map" | "mjs" | "svg" | "txt" | "wasm" | "xml")
    )
}

fn should_hash_rewrite(path: &Utf8Path) -> bool {
    if is_manifest_excluded(path) {
        return false;
    }
    if matches!(path.file_name(), Some("glory-bundle.json" | "index.html")) {
        return false;
    }
    const EXTENSIONS: &[&str] = &[
        "avif", "css", "gif", "html", "ico", "jpeg", "jpg", "js", "json", "map", "mjs", "otf", "png", "svg", "ttf", "txt", "wasm", "webp", "woff",
        "woff2", "xml",
    ];
    path.extension()
        .is_some_and(|ext| EXTENSIONS.iter().any(|candidate| ext.eq_ignore_ascii_case(candidate)))
}

fn should_optimize_image(path: &Utf8Path) -> bool {
    path.extension()
        .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png"))
}

fn hashed_asset_path(path: &Utf8Path, hash: &str) -> Option<Utf8PathBuf> {
    let stem = path.file_stem()?;
    let hash = hash.get(..16).unwrap_or(hash);
    let file_name = match path.extension() {
        Some(ext) => format!("{stem}.{hash}.{ext}"),
        None => format!("{stem}.{hash}"),
    };
    Some(path.with_file_name(file_name))
}

fn optimized_image_path(path: &Utf8Path) -> Option<Utf8PathBuf> {
    let stem = path.file_stem()?;
    Some(path.with_file_name(format!("{stem}.glory.webp")))
}

fn encode_webp(data: &[u8], path: &Utf8Path) -> Result<Vec<u8>> {
    let format = match path.extension().map(str::to_ascii_lowercase).as_deref() {
        Some("png") => image::ImageFormat::Png,
        Some("jpg" | "jpeg") => image::ImageFormat::Jpeg,
        _ => bail!("unsupported image asset format: {path}"),
    };
    let image = image::load_from_memory_with_format(data, format)?;
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, image::ImageFormat::WebP)?;
    Ok(output.into_inner())
}

fn asset_public_path(target: BuildTarget, rel: &Utf8Path) -> String {
    let rel = if target == BuildTarget::Web {
        rel.strip_prefix("public").unwrap_or(rel)
    } else {
        rel
    };
    format!("/{}", rel.as_str().trim_start_matches('/').replace('\\', "/"))
}

fn write_gzip(path: &Utf8Path, data: &[u8]) -> Result<()> {
    let gzip_file = File::create(format!("{}.gz", path.as_str()))?;
    let mut gzip_encoder = GzEncoder::new(gzip_file, flate2::Compression::best());
    gzip_encoder.write_all(data)?;
    gzip_encoder.finish()?;
    Ok(())
}

fn write_brotli(path: &Utf8Path, data: &[u8]) -> Result<()> {
    let brotli_file = File::create(format!("{}.br", path.as_str()))?;
    let mut brotli_writer = CompressorWriter::new(
        brotli_file,
        32 * 1024, // 32 KiB buffer
        11,        // BROTLI_PARAM_QUALITY
        22,        // BROTLI_PARAM_LGWIN
    );
    brotli_writer.write_all(data)?;
    Ok(())
}

async fn bundle_files(dist: &Utf8Path) -> Result<Vec<BundleFile>> {
    let mut files = Vec::new();
    for file in collect_files(dist)? {
        let rel = file.strip_prefix(dist)?;
        if file.file_name() == Some("glory-bundle.json") || is_manifest_excluded(rel) {
            continue;
        }
        let data = fs::read(&file).await?;
        files.push(BundleFile {
            path: rel.as_str().replace('\\', "/"),
            bytes: data.len() as u64,
            seahash: format!("{:016x}", seahash::hash(&data)),
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn is_manifest_excluded(path: &Utf8Path) -> bool {
    let parts = path.iter().take(3).collect::<Vec<_>>();
    matches!(parts.as_slice(), ["installers", "windows", "staging" | "obj"])
}

fn collect_files(root: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    let mut files = Vec::new();
    let mut dirs = VecDeque::from([root.to_owned()]);
    while let Some(dir) = dirs.pop_front() {
        let mut entries = dir.read_dir_utf8()?;
        while let Some(entry) = entries.next().transpose()? {
            let path = entry.path().to_owned();
            if entry.file_type()?.is_dir() {
                dirs.push_back(path);
            } else {
                files.push(path);
            }
        }
    }
    Ok(files)
}

fn collect_files_with_extension(root: &Utf8Path, extension: &str) -> Result<Vec<Utf8PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    Ok(collect_files(root)?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case(extension)))
        .collect())
}

fn collect_dirs_with_extension(root: &Utf8Path, extension: &str) -> Result<Vec<Utf8PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut matched = Vec::new();
    let mut dirs = VecDeque::from([root.to_owned()]);
    while let Some(dir) = dirs.pop_front() {
        let mut entries = dir.read_dir_utf8()?;
        while let Some(entry) = entries.next().transpose()? {
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let path = entry.path().to_owned();
            if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case(extension)) {
                matched.push(path);
            } else {
                dirs.push_back(path);
            }
        }
    }
    Ok(matched)
}

fn first_child_with_extension(root: &Utf8Path, extension: &str) -> Option<Utf8PathBuf> {
    let mut entries = root.read_dir_utf8().ok()?;
    while let Some(Ok(entry)) = entries.next() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case(extension)) {
            return Some(path.to_owned());
        }
    }
    None
}

fn read_first_existing(paths: &[Utf8PathBuf]) -> Option<String> {
    paths
        .iter()
        .find(|path| path.exists())
        .and_then(|path| std::fs::read_to_string(path).ok())
}

fn quoted_value_after(source: &str, key: &str) -> Option<String> {
    let idx = source.find(key)?;
    let rest = &source[idx + key.len()..];
    let start = rest.find('"')? + 1;
    let rest = &rest[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn env_list(name: &str) -> Option<Vec<String>> {
    let values = env::var(name).ok()?;
    let values = values
        .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    (!values.is_empty()).then_some(values)
}

fn ensure_program(name: &str, hint: &str) -> Result<()> {
    which::which(name).with_context(|| format!("`{name}` not found on PATH; {hint}"))?;
    Ok(())
}

fn pascal_case(value: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if uppercase_next {
                out.push(ch.to_ascii_uppercase());
                uppercase_next = false;
            } else {
                out.push(ch);
            }
        } else {
            uppercase_next = true;
        }
    }
    if out.is_empty() { "GloryApp".to_owned() } else { out }
}

#[derive(Serialize)]
struct BundleFile {
    path: String,
    bytes: u64,
    seahash: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precompresses_static_text_and_wasm_assets() {
        for path in ["index.html", "pkg/app.js", "pkg/app_bg.wasm", "style.css", "manifest.json"] {
            assert!(should_precompress(Utf8Path::new(path)), "{path}");
        }
    }

    #[test]
    fn skips_already_compressed_or_binary_assets() {
        for path in ["app.wasm.gz", "app.wasm.br", "image.png", "font.woff2"] {
            assert!(!should_precompress(Utf8Path::new(path)), "{path}");
        }
    }

    #[test]
    fn hash_rewrite_targets_static_assets_without_entry_html() {
        for path in ["assets/logo.png", "pkg/app_bg.wasm", "style.css", "docs/page.html", "fonts/app.woff2"] {
            assert!(should_hash_rewrite(Utf8Path::new(path)), "{path}");
        }
        for path in [
            "index.html",
            "server.exe",
            "glory-bundle.json",
            "app.wasm.gz",
            "installers/windows/staging/style.css",
        ] {
            assert!(!should_hash_rewrite(Utf8Path::new(path)), "{path}");
        }
    }

    #[test]
    fn hashed_asset_paths_preserve_parent_and_extension() {
        assert_eq!(
            hashed_asset_path(Utf8Path::new("assets/logo.png"), "0123456789abcdef").unwrap(),
            Utf8PathBuf::from("assets/logo.0123456789abcdef.png")
        );
        assert_eq!(
            hashed_asset_path(Utf8Path::new("pkg/app_bg.wasm"), "abcdef").unwrap(),
            Utf8PathBuf::from("pkg/app_bg.abcdef.wasm")
        );
    }

    #[test]
    fn web_asset_public_paths_strip_bundle_public_root() {
        assert_eq!(
            asset_public_path(BuildTarget::Web, Utf8Path::new("public/assets/logo.png")),
            "/assets/logo.png"
        );
        assert_eq!(
            asset_public_path(BuildTarget::Desktop, Utf8Path::new("assets/logo.png")),
            "/assets/logo.png"
        );
    }

    #[test]
    fn image_optimization_maps_original_to_hashed_webp() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let dir = temp_dir::TempDir::new().unwrap();
            let root = Utf8PathBuf::from_path_buf(dir.path().join("dist")).unwrap();
            std::fs::create_dir_all(root.join("assets")).unwrap();

            let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255])));
            let mut png = Cursor::new(Vec::new());
            image.write_to(&mut png, image::ImageFormat::Png).unwrap();
            std::fs::write(root.join("assets/logo.png"), png.into_inner()).unwrap();

            let mut asset_map = BTreeMap::new();
            write_optimized_image_assets(BuildTarget::Desktop, &root, &mut asset_map).await.unwrap();
            assert_eq!(asset_map.get("/assets/logo.png").map(String::as_str), Some("/assets/logo.glory.webp"));
            assert!(root.join("assets/logo.glory.webp").is_file());

            write_hashed_asset_copies(BuildTarget::Desktop, &root, &mut asset_map).await.unwrap();
            let mapped = asset_map.get("/assets/logo.png").unwrap();
            assert!(mapped.starts_with("/assets/logo.glory."), "{mapped}");
            assert!(mapped.ends_with(".webp"), "{mapped}");
            assert!(root.join(mapped.trim_start_matches('/')).is_file());
        });
    }

    #[test]
    fn desktop_installer_names_are_platform_friendly() {
        assert_eq!(debian_package_name("My Desktop_App"), "my-desktop-app");
        assert_eq!(debian_package_name("Glory++"), "glory++");
        assert_eq!(package_file_stem("Glory.App++"), "glory-app--");
        assert_eq!(installer_product_name("my-desktop_app"), "MyDesktopApp");
        assert_eq!(msi_version("1.2.3-beta.1"), "1.2.3");
        assert_eq!(msi_version("not-semver"), "0.0.0");
    }

    #[test]
    fn ar_member_writer_pads_odd_sized_members() {
        let mut out = Vec::new();
        write_ar_member(&mut out, "data.tar.gz", b"abc").unwrap();
        assert_eq!(out.len(), 64);
        assert_eq!(&out[0..12], b"data.tar.gz/");
        assert_eq!(&out[60..], b"abc\n");
    }

    #[test]
    fn android_metadata_helpers_parse_gradle_and_manifest() {
        let dir = temp_dir::TempDir::new().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().join("android")).unwrap();
        std::fs::create_dir_all(root.join("app/src/main")).unwrap();
        std::fs::write(
            root.join("app/build.gradle.kts"),
            r#"
android {
    namespace = "com.example.fallback"
    defaultConfig {
        applicationId = "com.example.real"
    }
}
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("app/src/main/AndroidManifest.xml"),
            r#"<manifest><application><activity android:name=".MainActivity" /></application></manifest>"#,
        )
        .unwrap();

        assert_eq!(android_application_id(&root).as_deref(), Some("com.example.real"));
        assert_eq!(
            android_launcher_activity(&root, "com.example.real").as_deref(),
            Some("com.example.real.MainActivity")
        );
    }

    #[test]
    fn ios_metadata_helpers_parse_project_yml() {
        let dir = temp_dir::TempDir::new().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().join("ios")).unwrap();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("project.yml"),
            r#"
name: DemoApp
options:
  bundleIdPrefix: com.example
"#,
        )
        .unwrap();

        assert_eq!(ios_project_name(&root).as_deref(), Some("DemoApp"));
        assert_eq!(ios_bundle_id(&root, "DemoApp").as_deref(), Some("com.example.DemoApp"));
        assert_eq!(pascal_case("my-mobile_app"), "MyMobileApp");
    }

    #[test]
    fn extension_collectors_find_files_and_bundles() {
        let dir = temp_dir::TempDir::new().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().join("out")).unwrap();
        std::fs::create_dir_all(root.join("apk/release")).unwrap();
        std::fs::write(root.join("apk/release/app-release.apk"), b"apk").unwrap();
        std::fs::create_dir_all(root.join("Build/Products/Debug-iphonesimulator/Demo.app")).unwrap();

        let apks = collect_files_with_extension(&root, "apk").unwrap();
        let apps = collect_dirs_with_extension(&root, "app").unwrap();
        assert_eq!(apks.len(), 1);
        assert_eq!(apps.len(), 1);
    }
}
