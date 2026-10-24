use crate::{Direction, Error, KeyboardInput, Orientation, RangeCommand, RangeValue};

/// One actual slider's accepted configuration, separate from passive progress
/// and separator resizing. Hosts propose changes; they do not own the value.
#[derive(Clone, Debug, PartialEq)]
pub struct SliderValue {
    pub range: RangeValue,
    pub axis: Orientation,
    pub direction: Direction,
    pub step: f64,
    pub page_step: f64,
    pub value_text: Option<String>,
}
impl SliderValue {
    pub fn validate(&self) -> Result<(), Error> {
        // A multi-thumb slider can have one temporarily fixed thumb when
        // its neighbour reaches the minimum gap. This is a closed interval;
        // RangeValue's general strict interval contract stays unchanged.
        if !self.range.min.is_finite()
            || !self.range.max.is_finite()
            || !self.range.value.is_finite()
            || self.range.min > self.range.max
            || self.range.value < self.range.min
            || self.range.value > self.range.max
        {
            return Err(Error::InvalidProps("slider requires a finite closed range"));
        }
        if [self.step, self.page_step]
            .into_iter()
            .any(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(Error::InvalidProps(
                "slider steps must be finite and positive",
            ));
        }
        if self
            .value_text
            .as_ref()
            .is_some_and(|text| text.len() > 1024 || text.trim().is_empty())
        {
            return Err(Error::InvalidProps(
                "slider value text requires 1..=1024 bytes",
            ));
        }
        Ok(())
    }
}
impl RangeCommand {
    pub fn slider_key(
        axis: Orientation,
        direction: Direction,
        input: KeyboardInput<'_>,
    ) -> Option<Self> {
        if input.editable
            || input.composing
            || input.control
            || input.meta
            || input.alt
            || input.shift
        {
            return None;
        }
        match (axis, direction, input.key) {
            (Orientation::Horizontal, Direction::Ltr, "ArrowRight")
            | (Orientation::Horizontal, Direction::Rtl, "ArrowLeft")
            | (Orientation::Vertical, _, "ArrowUp") => Some(Self::Increase),
            (Orientation::Horizontal, Direction::Ltr, "ArrowLeft")
            | (Orientation::Horizontal, Direction::Rtl, "ArrowRight")
            | (Orientation::Vertical, _, "ArrowDown") => Some(Self::Decrease),
            (_, _, "PageUp") => Some(Self::PageIncrease),
            (_, _, "PageDown") => Some(Self::PageDecrease),
            (_, _, "Home") => Some(Self::Minimum),
            (_, _, "End") => Some(Self::Maximum),
            _ => None,
        }
    }
}
