use crate::Result;
use std::io::Read;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub fn captured(command: &mut Command, cancel: Option<&AtomicBool>) -> Result<Output> {
    let Some(cancel) = cancel else {
        return command.output().map_err(|e| format!("{command:?}: {e}"));
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().ok_or("child stdout missing")?;
    let mut stderr = child.stderr.take().ok_or("child stderr missing")?;
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let status = wait(&mut child, cancel);
    let stdout = out
        .join()
        .map_err(|_| "stdout reader panicked")?
        .map_err(|e| e.to_string())?;
    let stderr = err
        .join()
        .map_err(|_| "stderr reader panicked")?
        .map_err(|e| e.to_string())?;
    Ok(Output {
        status: status?,
        stdout,
        stderr,
    })
}

pub fn inherited(command: &mut Command, cancel: Option<&AtomicBool>) -> Result<()> {
    let result = if let Some(cancel) = cancel {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        wait(&mut child, cancel)?
    } else {
        command.status().map_err(|e| format!("{command:?}: {e}"))?
    };
    if result.success() {
        Ok(())
    } else {
        Err(format!("{command:?} exited with {result}"))
    }
}

fn wait(child: &mut Child, cancel: &AtomicBool) -> Result<std::process::ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(error) => {
                kill_tree(child);
                let _ = child.wait();
                return Err(error.to_string());
            }
        }
        if cancel.load(Ordering::Acquire) {
            kill_tree(child);
            child.wait().map_err(|e| e.to_string())?;
            return Err("command cancelled".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// One owned native development process. Explicit stop and Drop reap it.
pub struct Running(Option<Child>);
impl Running {
    pub fn spawn(command: &mut Command) -> Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        command
            .spawn()
            .map(|child| Self(Some(child)))
            .map_err(|e| format!("{command:?}: {e}"))
    }
    pub fn exited(&mut self) -> Result<bool> {
        self.0
            .as_mut()
            .map(|child| {
                child
                    .try_wait()
                    .map(|status| status.is_some())
                    .map_err(|e| e.to_string())
            })
            .unwrap_or(Ok(true))
    }
    pub fn stop(&mut self) {
        if let Some(mut child) = self.0.take() {
            if matches!(child.try_wait(), Ok(None)) {
                kill_tree(&mut child);
            }
            let _ = child.wait();
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.stop();
    }
}

fn kill_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .creation_flags(0x08000000)
            .output();
    }
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-KILL", "--", &format!("-{}", child.id())])
            .output();
    }
    let _ = child.kill();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn child_entry() {
        if std::env::var_os("GLORY_CLI_CANCELLATION_CHILD").is_some() {
            std::thread::sleep(Duration::from_secs(30));
        }
    }
    #[test]
    fn cancellation_terminates_and_reaps_the_owned_child_without_waiting_for_its_work() {
        let cancel = Arc::new(AtomicBool::new(false));
        let trigger = cancel.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            trigger.store(true, Ordering::Release);
        });
        let start = std::time::Instant::now();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "process::tests::child_entry"])
            .env("GLORY_CLI_CANCELLATION_CHILD", "1");
        assert!(
            captured(&mut command, Some(&cancel))
                .unwrap_err()
                .contains("cancelled")
        );
        thread.join().unwrap();
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
