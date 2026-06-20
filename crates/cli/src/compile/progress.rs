//! Structured build-progress model.
//!
//! This is intentionally *not* a full-screen live TUI (a terminal-driven
//! renderer is hard to verify in CI). Instead it is a small, pure state model
//! describing which build stage is running, plus a string renderer that callers
//! can `log::info!`. Time is injected by the caller (`finish_with`/`fail_with`),
//! so the model never reads the clock and stays deterministic for tests.

use std::fmt::Write as _;

/// The ordered stages of a Glory project build.
///
/// Stage names map onto [`crate::command::build::build_proj`]: front compile +
/// wasm-bindgen + (release) wasm-opt, asset copy/optimization, stylesheet
/// build, server binary compile, and mobile packaging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildStage {
    /// Compiling the front (wasm) crate and running wasm-bindgen.
    Compiling,
    /// Running wasm-opt on the release wasm artifact.
    WasmOpt,
    /// Copying / optimizing static assets.
    Assets,
    /// Building stylesheets (sass / tailwind / lightningcss).
    Styling,
    /// Compiling the server binary.
    Server,
    /// Packaging the mobile artifact.
    Mobile,
}

impl BuildStage {
    /// Every stage in build order. Used to seed a fresh [`BuildProgress`].
    pub const ALL: [BuildStage; 6] = [
        BuildStage::Compiling,
        BuildStage::WasmOpt,
        BuildStage::Assets,
        BuildStage::Styling,
        BuildStage::Server,
        BuildStage::Mobile,
    ];

    /// Short human label for the stage.
    pub fn label(self) -> &'static str {
        match self {
            BuildStage::Compiling => "Compiling",
            BuildStage::WasmOpt => "WasmOpt",
            BuildStage::Assets => "Assets",
            BuildStage::Styling => "Styling",
            BuildStage::Server => "Server",
            BuildStage::Mobile => "Mobile",
        }
    }
}

/// Status of a single build stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageStatus {
    /// Not started yet.
    Pending,
    /// Currently running.
    Running,
    /// Finished successfully, with an optional caller-supplied duration (ms).
    Done { millis: Option<u64> },
    /// Failed, carrying an error message.
    Failed { message: String },
}

#[derive(Debug, Clone)]
struct StageState {
    stage: BuildStage,
    status: StageStatus,
}

/// Tracks the status of each build stage.
///
/// The model never reads a clock: durations are injected by the caller via
/// [`BuildProgress::finish_with`]. `start`/`finish`/`fail` mutate the per-stage
/// status; [`render_progress`] turns the current state into a readable line.
#[derive(Debug, Clone)]
pub struct BuildProgress {
    stages: Vec<StageState>,
}

impl Default for BuildProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildProgress {
    /// Create a progress model with every [`BuildStage`] marked pending.
    pub fn new() -> Self {
        Self {
            stages: BuildStage::ALL
                .iter()
                .map(|&stage| StageState {
                    stage,
                    status: StageStatus::Pending,
                })
                .collect(),
        }
    }

    fn slot(&mut self, stage: BuildStage) -> &mut StageState {
        // ALL stages are seeded in `new`; this lookup always succeeds.
        self.stages
            .iter_mut()
            .find(|s| s.stage == stage)
            .expect("stage seeded in BuildProgress::new")
    }

    /// Mark a stage as running.
    pub fn start(&mut self, stage: BuildStage) {
        self.slot(stage).status = StageStatus::Running;
    }

    /// Mark a stage as done (no duration recorded).
    pub fn finish(&mut self, stage: BuildStage) {
        self.slot(stage).status = StageStatus::Done { millis: None };
    }

    /// Mark a stage as done with a caller-measured duration in milliseconds.
    pub fn finish_with(&mut self, stage: BuildStage, millis: u64) {
        self.slot(stage).status = StageStatus::Done { millis: Some(millis) };
    }

    /// Mark a stage as failed with a message.
    pub fn fail(&mut self, stage: BuildStage, message: impl Into<String>) {
        self.slot(stage).status = StageStatus::Failed { message: message.into() };
    }

    /// Status of a single stage, if present.
    pub fn status(&self, stage: BuildStage) -> Option<&StageStatus> {
        self.stages.iter().find(|s| s.stage == stage).map(|s| &s.status)
    }

    /// True when every stage is done and none failed.
    pub fn is_complete(&self) -> bool {
        self.stages.iter().all(|s| matches!(s.status, StageStatus::Done { .. }))
    }

    /// True when any stage failed.
    pub fn is_failed(&self) -> bool {
        self.stages.iter().any(|s| matches!(s.status, StageStatus::Failed { .. }))
    }
}

/// Render the progress as a single readable line, e.g.
/// `✓ Compiling (120ms)  → Assets  · Styling  · Server`.
///
/// Markers: `✓` done, `→` running, `✗` failed, `·` pending. A failed stage
/// appends its message.
pub fn render_progress(progress: &BuildProgress) -> String {
    let mut out = String::new();
    for (idx, state) in progress.stages.iter().enumerate() {
        if idx > 0 {
            out.push_str("  ");
        }
        match &state.status {
            StageStatus::Pending => {
                let _ = write!(out, "· {}", state.stage.label());
            }
            StageStatus::Running => {
                let _ = write!(out, "→ {}", state.stage.label());
            }
            StageStatus::Done { millis } => {
                let _ = write!(out, "✓ {}", state.stage.label());
                if let Some(ms) = millis {
                    let _ = write!(out, " ({ms}ms)");
                }
            }
            StageStatus::Failed { message } => {
                let _ = write!(out, "✗ {} ({message})", state.stage.label());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_progress_is_all_pending() {
        let progress = BuildProgress::new();
        for stage in BuildStage::ALL {
            assert_eq!(progress.status(stage), Some(&StageStatus::Pending), "{stage:?}");
        }
        assert!(!progress.is_complete());
        assert!(!progress.is_failed());
    }

    #[test]
    fn stage_advances_through_start_then_finish() {
        let mut progress = BuildProgress::new();
        progress.start(BuildStage::Compiling);
        assert_eq!(progress.status(BuildStage::Compiling), Some(&StageStatus::Running));
        progress.finish_with(BuildStage::Compiling, 120);
        assert_eq!(progress.status(BuildStage::Compiling), Some(&StageStatus::Done { millis: Some(120) }));
    }

    #[test]
    fn finish_without_duration_records_none() {
        let mut progress = BuildProgress::new();
        progress.finish(BuildStage::Assets);
        assert_eq!(progress.status(BuildStage::Assets), Some(&StageStatus::Done { millis: None }));
    }

    #[test]
    fn fail_marks_failed_and_flags_failure() {
        let mut progress = BuildProgress::new();
        progress.start(BuildStage::Server);
        progress.fail(BuildStage::Server, "linker error");
        assert!(progress.is_failed());
        assert!(!progress.is_complete());
        match progress.status(BuildStage::Server) {
            Some(StageStatus::Failed { message }) => assert_eq!(message, "linker error"),
            other => panic!("expected failed, got {other:?}"),
        }
    }

    #[test]
    fn is_complete_only_when_every_stage_done() {
        let mut progress = BuildProgress::new();
        for stage in BuildStage::ALL {
            assert!(!progress.is_complete());
            progress.finish(stage);
        }
        assert!(progress.is_complete());
        assert!(!progress.is_failed());
    }

    #[test]
    fn render_contains_expected_stage_markers() {
        let mut progress = BuildProgress::new();
        progress.finish_with(BuildStage::Compiling, 120);
        progress.start(BuildStage::Assets);
        let line = render_progress(&progress);

        assert!(line.contains("✓ Compiling (120ms)"), "{line}");
        assert!(line.contains("→ Assets"), "{line}");
        // Later stages remain pending.
        assert!(line.contains("· Styling"), "{line}");
        assert!(line.contains("· Server"), "{line}");
        assert!(line.contains("· Mobile"), "{line}");
    }

    #[test]
    fn render_shows_failure_message() {
        let mut progress = BuildProgress::new();
        progress.fail(BuildStage::Server, "boom");
        let line = render_progress(&progress);
        assert!(line.contains("✗ Server (boom)"), "{line}");
    }
}
