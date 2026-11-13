//! Finite queued pointer observations for explicitly declared plot viewports.
use crate::{Error, Rect};

/// Logical CSS-pixel admission bound, independent of physical device scale.
pub const MAX_PLOT_POINTER_COORDINATE: f64 = 10_000_000.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotPointerDevice {
    Mouse,
    Pen,
    Touch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotPointerPhase {
    Move,
    Down,
    Up,
    Leave,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotPointer {
    pub pointer_id: i32,
    pub device: PlotPointerDevice,
    pub phase: PlotPointerPhase,
    /// Fractional local border-box coordinates. A captured terminal point can
    /// lie outside the viewport; no clamping, rounding or DPR multiplication.
    pub x: f64,
    pub y: f64,
    /// The same observation's fractional border-box extent, with x=y=0.
    /// Consumers map this extent to their accepted logical drawing viewport.
    pub viewport: Rect,
}

impl PlotPointer {
    pub fn validate(self) -> Result<(), Error> {
        let terminal = matches!(
            self.phase,
            PlotPointerPhase::Leave | PlotPointerPhase::Cancel
        );
        let bounded = |value: f64| value.is_finite() && value.abs() <= MAX_PLOT_POINTER_COORDINATE;
        if !bounded(self.x)
            || !bounded(self.y)
            || self.viewport.x != 0.
            || self.viewport.y != 0.
            || !bounded(self.viewport.width)
            || !bounded(self.viewport.height)
            || self.viewport.width < 0.
            || self.viewport.height < 0.
            || (!terminal && (self.viewport.width == 0. || self.viewport.height == 0.))
        {
            return Err(Error::InvalidEvent("invalid local plot pointer geometry"));
        }
        Ok(())
    }
}
