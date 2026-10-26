use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new() -> Self {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .expect("set CARGO_TARGET_DIR to a D: build directory for tests");
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
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = crate::build::remove_owned(&self.0, self.0.parent().unwrap());
    }
}
