//! macOS application bundles. The signature is finalized before hashing/publish.
use crate::{Result, args::Args, build, project::Project};
use std::{fs, path::Path, process::Command};

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub(crate) fn bundle(
    project: &Project,
    executable: &Path,
    stage: &Path,
    args: &Args,
) -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err("macOS bundle signing requires a macOS host".into());
    }
    let identifier = project
        .host
        .bundle_identifier
        .clone()
        .unwrap_or_else(|| format!("rs.glory.{}", project.name().replace('_', "-")));
    if identifier.split('.').count() < 2
        || identifier.split('.').any(|part| {
            part.is_empty() || !part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
    {
        return Err("bundle-identifier must be a reverse-DNS identifier".into());
    }
    let app = stage.join(format!("{}.app", project.name()));
    let contents = app.join("Contents");
    let resources = contents.join("Resources");
    let binaries = contents.join("MacOS");
    fs::create_dir_all(&resources).map_err(|e| e.to_string())?;
    fs::create_dir_all(&binaries).map_err(|e| e.to_string())?;
    for asset in &project.host.assets {
        build::copy_asset(&project.root, asset, &resources)?;
    }
    let name = executable
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("invalid executable name")?;
    fs::copy(executable, binaries.join(name)).map_err(|e| e.to_string())?;
    let version = project.package["version"]
        .as_str()
        .unwrap_or("0.0.0")
        .split(['-', '+'])
        .next()
        .unwrap();
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>{}</string>
<key>CFBundleName</key><string>{}</string>
<key>CFBundleExecutable</key><string>{}</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleShortVersionString</key><string>{}</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
"#,
        xml(&identifier),
        xml(project.name()),
        xml(name),
        xml(version)
    );
    fs::write(contents.join("Info.plist"), plist).map_err(|e| e.to_string())?;
    // An ad-hoc signature permits local installation; Developer ID and
    // notarization remain explicit release operations owned by the application.
    let mut sign = Command::new("codesign");
    sign.args(["--force", "--sign", "-"]).arg(&app);
    project.status(&mut sign)?;
    let mut verify = Command::new("codesign");
    verify.args(["--verify", "--strict"]).arg(&app);
    project.status(&mut verify)?;
    build::write_manifest(project, stage, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    #[test]
    fn signed_macos_bundle_survives_relocation_and_hashes_final_signature() {
        let scratch = crate::test_support::Scratch::new();
        scratch.write("assets/settings.json", "{\"theme\":\"dark\"}");
        let mut project = scratch.project(crate::project::Platform::Desktop);
        project.build_target = "aarch64-apple-darwin".into();
        project.rust_host = project.build_target.clone();
        project.host.bundle_identifier = Some("rs.glory.fixture".into());
        project.host.assets.push("assets".into());
        let args = Args::parse(["bundle".into()]).unwrap();
        let stage = build::staging(&scratch.0).unwrap();
        bundle(&project, Path::new("/usr/bin/true"), &stage, &args).unwrap();
        let moved = scratch.0.join("copied with spaces");
        fs::rename(stage, &moved).unwrap();
        let app = moved.join("fixture.app");
        assert!(
            Command::new(app.join("Contents/MacOS/true"))
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("codesign")
                .args(["--verify", "--strict"])
                .arg(&app)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            app.join("Contents/Resources/assets/settings.json")
                .is_file()
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(moved.join("glory-build.json")).unwrap()).unwrap();
        assert!(manifest["files"].as_array().unwrap().iter().any(|entry| {
            entry["path"]
                .as_str()
                .unwrap()
                .contains("_CodeSignature/CodeResources")
        }));
        for entry in manifest["files"].as_array().unwrap() {
            use sha2::{Digest, Sha256};
            let bytes = fs::read(moved.join(entry["path"].as_str().unwrap())).unwrap();
            assert_eq!(entry["sha256"], format!("{:x}", Sha256::digest(bytes)));
        }
    }

    #[test]
    fn plist_text_escapes_user_metadata() {
        assert_eq!(xml("a&<b>\"'"), "a&amp;&lt;b&gt;&quot;&apos;");
    }
}
