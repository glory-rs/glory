//! Application-owned UIKit scene bridge; no additional UI/reactive runtime.
use std::{env, path::PathBuf, process::Command};

fn xcrun(args: &[&str]) -> String {
    let output = Command::new("xcrun")
        .args(args)
        .output()
        .expect("Xcode xcrun required");
    assert!(
        output.status.success(),
        "xcrun {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("UTF-8 Xcode path")
        .trim()
        .to_owned()
}

fn main() {
    println!("cargo:rerun-if-changed=ios/SceneDelegate.swift");
    println!("cargo:rerun-if-changed=ios/Permissions.swift");
    println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");
    println!("cargo:rerun-if-env-changed=GLORY_IOS_SCENE_MULTI");
    println!("cargo:rerun-if-env-changed=GLORY_MOBILE_NETWORK_ORIGIN");
    println!("cargo:rerun-if-env-changed=GLORY_MOBILE_TLS_ORIGIN");
    println!("cargo:rerun-if-env-changed=GLORY_MOBILE_REJECT_TLS_ORIGIN");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("ios") {
        return;
    }
    let target = env::var("TARGET").unwrap();
    let simulator = target.ends_with("-sim") || target.starts_with("x86_64");
    let sdk = if simulator {
        "iphonesimulator"
    } else {
        "iphoneos"
    };
    let arch = if target.starts_with("aarch64") {
        "arm64"
    } else {
        "x86_64"
    };
    let swift_target = format!(
        "{arch}-apple-ios15.0{}",
        if simulator { "-simulator" } else { "" }
    );
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let object = output.join("SceneDelegate.o");
    let mut compiler_command = Command::new("xcrun");
    compiler_command
        .args([
            "--sdk",
            sdk,
            "swiftc",
            "-parse-as-library",
            "-emit-object",
            "-target",
            &swift_target,
            "-sdk",
            &xcrun(&["--sdk", sdk, "--show-sdk-path"]),
            "-module-cache-path",
        ])
        .arg(output.join("SwiftModules"))
        .arg("ios/SceneDelegate.swift")
        .arg("-o")
        .arg(&object);
    if env::var_os("CARGO_FEATURE_PERMISSIONS").is_some() {
        compiler_command.args([
            "-whole-module-optimization",
            "-D",
            "GLORY_PERMISSION_HOST",
            "ios/Permissions.swift",
        ]);
        for framework in [
            "AVFoundation",
            "Photos",
            "CoreLocation",
            "UserNotifications",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    if env::var_os("CARGO_FEATURE_PERMISSIONS_HOST_PROBE").is_some() {
        compiler_command.args(["-D", "GLORY_PERMISSION_HOST_PROBE"]);
    }
    if env::var("GLORY_IOS_SCENE_MULTI").as_deref() == Ok("1") {
        assert!(simulator, "scene probe is simulator-only");
        compiler_command.args(["-D", "GLORY_SCENE_PROBE"]);
    }
    let status = compiler_command.status().expect("Swift compiler required");
    assert!(status.success(), "UIKit scene bridge compilation failed");
    let compiler = PathBuf::from(xcrun(&["--find", "swiftc"]));
    let libraries = compiler
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("lib/swift")
        .join(sdk);
    println!("cargo:rustc-link-search=native={}", libraries.display());
    println!("cargo:rustc-link-search=native=/usr/lib/swift");
    println!("cargo:rustc-link-arg-bin=counter-ios={}", object.display());
}
