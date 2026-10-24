use crate::{Error, KeyboardInput, Orientation, RangeValue};

/// Finite range proposals. Application state remains application-owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RangeCommand {
    Decrease,
    PageDecrease,
    PageIncrease,
    Increase,
    Minimum,
    Maximum,
    Set(f64),
}
impl RangeCommand {
    /// The argument is the separator line's orientation: movement is perpendicular.
    pub fn splitter_key(line: Orientation, input: KeyboardInput<'_>) -> Option<Self> {
        if input.editable
            || input.composing
            || input.control
            || input.meta
            || input.alt
            || input.shift
        {
            return None;
        }
        match (line, input.key) {
            (Orientation::Vertical, "ArrowLeft") | (Orientation::Horizontal, "ArrowUp") => {
                Some(Self::Decrease)
            }
            (Orientation::Vertical, "ArrowRight") | (Orientation::Horizontal, "ArrowDown") => {
                Some(Self::Increase)
            }
            (_, "Home") => Some(Self::Minimum),
            (_, "End") => Some(Self::Maximum),
            _ => None,
        }
    }
    pub fn validate(self, range: RangeValue) -> Result<(), Error> {
        if let Self::Set(value) = self
            && (!value.is_finite() || value < range.min || value > range.max)
        {
            return Err(Error::InvalidEvent(
                "range proposal must be finite and within accepted bounds",
            ));
        }
        Ok(())
    }
}
