use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new() -> Self {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .expect("set CARGO_TARGET_DIR to an approved build directory for tests");
        let root = PathBuf::from(target).join("cli-test-data");
        crate::project::validate_build_root(&root).unwrap();
        let path = root.join(format!(
            "{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    pub fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    pub fn project(&self, platform: crate::project::Platform) -> crate::project::Project {
        let root = self.0.canonicalize().unwrap();
        crate::project::Project {
            cancel: None,
            manifest: root.join("Cargo.toml"),
            root: root.clone(),
            package: serde_json::json!({"id":"fixture", "name":"fixture"}),
            host: Default::default(),
            platform,
            paths: crate::project::Paths {
                cargo_home: root.join("cargo"),
                target: root.join("target"),
                temp: root.join("tmp"),
                root,
            },
            rust_host: "x86_64-pc-windows-msvc".into(),
            build_target: "x86_64-pc-windows-msvc".into(),
            bindgen_version: None,
            watch_roots: Vec::new(),
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = crate::build::remove_owned(&self.0, self.0.parent().unwrap());
    }
}
