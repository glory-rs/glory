//! Finite viewport scrolling commands and CSSOM-like geometry observations.
use crate::{Error, KeyboardInput, Orientation};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollAxes {
    #[default]
    Both,
    Horizontal,
    Vertical,
}
impl ScrollAxes {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Both => "both",
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
    pub fn allows(self, axis: Orientation) -> bool {
        matches!(self, Self::Both)
            || matches!(
                (self, axis),
                (Self::Horizontal, Orientation::Horizontal)
                    | (Self::Vertical, Orientation::Vertical)
            )
    }
    /// Hosts call this only when the viewport itself owns actual focus. Native
    /// descendants keep their editing, activation and collection navigation.
    pub fn key(self, input: KeyboardInput<'_>) -> Option<ScrollCommand> {
        if input.editable
            || input.composing
            || input.control
            || input.meta
            || input.alt
            || input.shift
        {
            return None;
        }
        let primary = if self == Self::Horizontal {
            Orientation::Horizontal
        } else {
            Orientation::Vertical
        };
        let (axis, movement) = match input.key {
            "ArrowLeft" => (Orientation::Horizontal, ScrollMovement::StepNegative),
            "ArrowRight" => (Orientation::Horizontal, ScrollMovement::StepPositive),
            "ArrowUp" => (Orientation::Vertical, ScrollMovement::StepNegative),
            "ArrowDown" => (Orientation::Vertical, ScrollMovement::StepPositive),
            "PageUp" => (primary, ScrollMovement::PageNegative),
            "PageDown" => (primary, ScrollMovement::PagePositive),
            "Home" => (primary, ScrollMovement::Start),
            "End" => (primary, ScrollMovement::End),
            _ => return None,
        };
        self.allows(axis)
            .then_some(ScrollCommand { axis, movement })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollMovement {
    StepNegative,
    StepPositive,
    PageNegative,
    PagePositive,
    Start,
    End,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollCommand {
    pub axis: Orientation,
    pub movement: ScrollMovement,
}

/// Logical pixels at one accepted query watermark. Native/browser offsets can
/// be negative (RTL/elastic scrolling); dimensions are finite and nonnegative.
/// Content dimensions are independent and need not exceed viewport dimensions.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollMetrics {
    pub x: f64,
    pub y: f64,
    pub viewport_width: f64,
    pub viewport_height: f64,
    pub content_width: f64,
    pub content_height: f64,
}
impl ScrollMetrics {
    pub fn validate(self) -> Result<(), Error> {
        if ![
            self.x,
            self.y,
            self.viewport_width,
            self.viewport_height,
            self.content_width,
            self.content_height,
        ]
        .into_iter()
        .all(f64::is_finite)
            || self.viewport_width < 0.0
            || self.viewport_height < 0.0
            || self.content_width < 0.0
            || self.content_height < 0.0
        {
            return Err(Error::InvalidResponse("invalid scroll metrics"));
        }
        Ok(())
    }
}
