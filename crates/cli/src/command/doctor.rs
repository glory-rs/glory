use crate::config::{BuildTarget, Opts};
use crate::ext::anyhow::{Result, bail};
use tokio::process::Command;

pub async fn doctor(opts: &Opts) -> Result<()> {
    let mut report = DoctorReport::default();

    report.command("cargo", ["--version"]).await;
    report.command("rustc", ["--version"]).await;
    report.command("rustfmt", ["--version"]).await;
    report.command("cargo", ["clippy", "--version"]).await;

    for target in required_rust_targets(opts.target) {
        report.rust_target(target).await;
    }

    match opts.target {
        BuildTarget::Android => {
            report.command("cargo", ["ndk", "--version"]).await;
            report.env_path("ANDROID_NDK_HOME");
        }
        BuildTarget::Ios => {
            report.host_os("macos", cfg!(target_os = "macos"));
            report.command("xcodebuild", ["-version"]).await;
            report.command("xcodegen", ["--version"]).await;
        }
        BuildTarget::Desktop => {
            report.note(
                "desktop",
                "requires the platform WebView runtime (WebView2 on Windows, WebKitGTK on Linux, WKWebView on macOS)",
            );
        }
        BuildTarget::Native => {
            report.note(
                "native",
                "Blitz/vello windowing is still experimental; `doctor` only checks Rust prerequisites",
            );
        }
        BuildTarget::Web => {
            report.command("wasm-bindgen", ["--version"]).await;
            report.command("wasm-opt", ["--version"]).await;
        }
    }

    report.print();
    if report.failed == 0 {
        Ok(())
    } else {
        bail!("doctor found {} failing check(s)", report.failed)
    }
}

fn required_rust_targets(target: BuildTarget) -> &'static [&'static str] {
    match target {
        BuildTarget::Web => &["wasm32-unknown-unknown"],
        BuildTarget::Android => &["aarch64-linux-android"],
        BuildTarget::Ios => &["aarch64-apple-ios"],
        BuildTarget::Desktop | BuildTarget::Native => &[],
    }
}

#[derive(Default)]
struct DoctorReport {
    rows: Vec<DoctorRow>,
    failed: usize,
}

impl DoctorReport {
    async fn command<I, S>(&mut self, program: &str, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let args = args.into_iter().collect::<Vec<_>>();
        let rendered = format!(
            "{} {}",
            program,
            args.iter()
                .map(|arg| arg.as_ref().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        );
        match Command::new(program).args(args).output().await {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let detail = stdout.lines().chain(stderr.lines()).next().unwrap_or("ok").trim().to_owned();
                self.ok(rendered, detail);
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                self.fail(rendered, stderr.lines().next().unwrap_or("command failed").trim());
            }
            Err(err) => self.fail(rendered, err.to_string()),
        }
    }

    async fn rust_target(&mut self, target: &str) {
        match Command::new("rustup").args(["target", "list", "--installed"]).output().await {
            Ok(output) if output.status.success() => {
                let installed = String::from_utf8_lossy(&output.stdout);
                if installed.lines().any(|line| line.trim() == target) {
                    self.ok(format!("rust target {target}"), "installed");
                } else {
                    self.fail(format!("rust target {target}"), format!("missing; run `rustup target add {target}`"));
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                self.fail(format!("rust target {target}"), stderr.lines().next().unwrap_or("rustup failed"));
            }
            Err(err) => self.fail(format!("rust target {target}"), err.to_string()),
        }
    }

    fn env_path(&mut self, name: &str) {
        match std::env::var(name) {
            Ok(value) if !value.trim().is_empty() => self.ok(format!("env {name}"), value),
            _ => self.fail(format!("env {name}"), "missing"),
        }
    }

    fn host_os(&mut self, name: &str, ok: bool) {
        if ok {
            self.ok(format!("host {name}"), "ok");
        } else {
            self.fail(format!("host {name}"), "wrong host OS for this target");
        }
    }

    fn note(&mut self, check: &str, detail: &str) {
        self.rows.push(DoctorRow {
            ok: None,
            check: check.to_owned(),
            detail: detail.to_owned(),
        });
    }

    fn ok(&mut self, check: impl Into<String>, detail: impl Into<String>) {
        self.rows.push(DoctorRow {
            ok: Some(true),
            check: check.into(),
            detail: detail.into(),
        });
    }

    fn fail(&mut self, check: impl Into<String>, detail: impl Into<String>) {
        self.failed += 1;
        self.rows.push(DoctorRow {
            ok: Some(false),
            check: check.into(),
            detail: detail.into(),
        });
    }

    fn print(&self) {
        for row in &self.rows {
            let marker = match row.ok {
                Some(true) => "ok",
                Some(false) => "fail",
                None => "note",
            };
            println!("[{marker}] {} - {}", row.check, row.detail);
        }
    }
}

struct DoctorRow {
    ok: Option<bool>,
    check: String,
    detail: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_targets_match_build_targets() {
        assert_eq!(required_rust_targets(BuildTarget::Web), &["wasm32-unknown-unknown"]);
        assert_eq!(required_rust_targets(BuildTarget::Android), &["aarch64-linux-android"]);
        assert_eq!(required_rust_targets(BuildTarget::Ios), &["aarch64-apple-ios"]);
        assert!(required_rust_targets(BuildTarget::Desktop).is_empty());
        assert!(required_rust_targets(BuildTarget::Native).is_empty());
    }

    #[test]
    fn failed_rows_increment_count() {
        let mut report = DoctorReport::default();
        report.fail("x", "y");
        report.ok("a", "b");
        assert_eq!(report.failed, 1);
        assert_eq!(report.rows.len(), 2);
    }
}
