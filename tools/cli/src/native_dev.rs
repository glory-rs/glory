//! Native development restarts only after a successful build. Each running
//! executable has its own copy, so Windows builds can link the next generation.
use crate::{Result, args::Args, build, process::Running, project::Project};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

struct Prepared {
    directory: PathBuf,
    root: PathBuf,
    executable: PathBuf,
}
impl Prepared {
    fn build(project: &Project, args: &Args) -> Result<Self> {
        let built = build::build(project, args)?;
        let executable = built.executable.ok_or("native build has no executable")?;
        let directory = build::staging(&project.paths.root)?;
        let prepared = Self {
            executable: directory.join(executable.file_name().ok_or("executable has no name")?),
            directory,
            root: project.paths.root.clone(),
        };
        for asset in &project.host.assets {
            build::copy_asset(&project.root, asset, &prepared.directory)?;
        }
        if prepared.executable.exists() {
            return Err("assets collide with the native executable".into());
        }
        fs::copy(&executable, &prepared.executable).map_err(|e| e.to_string())?;
        Ok(prepared)
    }
    fn start(&self, project: &Project) -> Result<Running> {
        let mut command = Command::new(&self.executable);
        command.current_dir(&project.root);
        project.paths.apply(&mut command);
        Running::spawn(&mut command)
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        let _ = build::remove_owned(&self.directory, &self.root);
    }
}

pub(crate) fn serve(mut project: Project, mut args: Args) -> Result<()> {
    project.ensure_runnable()?;
    if !args.watch {
        return build::run(&project, &args);
    }
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::Release)).map_err(|e| e.to_string())?;
    project.cancel = Some(stop.clone());
    args.cancel = Some(stop.clone());
    let changed = Arc::new(AtomicBool::new(false));
    let mut watcher = crate::serve::watch(&project, changed.clone())?;
    // Declaration order drops the process before removing its executable copy.
    let mut prepared = Prepared::build(&project, &args)?;
    let mut running = prepared.start(&project)?;
    let mut exited = false;
    let mut last_event = None;
    println!("Native development: restart after successful changes (Ctrl+C to stop)");
    while !stop.load(Ordering::Acquire) {
        if !exited && running.exited()? {
            exited = true;
            println!("Application exited; waiting for source changes");
        }
        if changed.swap(false, Ordering::AcqRel) {
            last_event = Some(Instant::now());
        }
        if last_event.is_some_and(|time| time.elapsed() >= Duration::from_millis(200)) {
            last_event = None;
            match Project::load(&project.manifest, &args).and_then(|current| {
                current.ensure_runnable()?;
                let next = Prepared::build(&current, &args)?;
                Ok((current, next))
            }) {
                Ok((current, next)) => {
                    let next_watcher = crate::serve::watch(&current, changed.clone())?;
                    running.stop();
                    // If spawning fails, retain and restart the previous binary.
                    match next.start(&current) {
                        Ok(child) => {
                            running = child;
                            prepared = next;
                            project = current;
                            watcher = next_watcher;
                            exited = false;
                            println!("Native rebuild complete; application restarted");
                        }
                        Err(error) => {
                            running = prepared.start(&project)?;
                            exited = false;
                            eprintln!("{error}; previous application restarted");
                        }
                    }
                }
                Err(error) => eprintln!("{error}; keeping the current application"),
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    running.stop();
    drop(watcher);
    Ok(())
}
