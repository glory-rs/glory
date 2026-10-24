//! Numeric accessibility coordinates for an editable text spinbutton.
//! Applications may retain an exact decimal model and put its exact text in
//! value_text; these finite f64 fields are not the business arithmetic domain.
use crate::{Error, KeyboardInput, RangeCommand};

#[derive(Clone, Debug, PartialEq)]
pub struct SpinValue {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub value: Option<f64>,
    pub step: f64,
    pub page_step: f64,
    pub value_text: Option<String>,
    pub can_increase: bool,
    pub can_decrease: bool,
}
impl RangeCommand {
    /// Only hosts that have already identified the focused single-line Spin
    /// input may call this branch. Text editing remains enabled on that input.
    pub fn spin_key(has_min: bool, has_max: bool, input: KeyboardInput<'_>) -> Option<Self> {
        if input.composing || input.control || input.meta || input.alt || input.shift {
            return None;
        }
        match input.key {
            "ArrowUp" => Some(Self::Increase),
            "ArrowDown" => Some(Self::Decrease),
            "PageUp" => Some(Self::PageIncrease),
            "PageDown" => Some(Self::PageDecrease),
            "Home" if has_min => Some(Self::Minimum),
            "End" if has_max => Some(Self::Maximum),
            _ => None,
        }
    }
}
impl Default for SpinValue {
    fn default() -> Self {
        Self {
            min: None,
            max: None,
            value: None,
            step: 1.,
            page_step: 10.,
            value_text: None,
            can_increase: true,
            can_decrease: true,
        }
    }
}
impl SpinValue {
    pub fn validate(&self) -> Result<(), Error> {
        if [self.min, self.max, self.value]
            .into_iter()
            .flatten()
            .any(|v| !v.is_finite())
            || !self.step.is_finite()
            || self.step <= 0.
            || !self.page_step.is_finite()
            || self.page_step <= 0.
            || self.min.zip(self.max).is_some_and(|(min, max)| min > max)
        {
            return Err(Error::InvalidProps(
                "spinbutton coordinates and positive steps must be finite with ordered bounds",
            ));
        }
        if self
            .value_text
            .as_ref()
            .is_some_and(|v| v.len() > 1024 || v.contains('\0'))
        {
            return Err(Error::InvalidProps(
                "spinbutton value text must be within 1024 bytes without NUL",
            ));
        }
        Ok(())
    }
    pub fn validate_command(&self, command: RangeCommand) -> Result<(), Error> {
        self.validate()?;
        if matches!(command,RangeCommand::Set(v) if !v.is_finite()) {
            return Err(Error::InvalidEvent("spinbutton set value must be finite"));
        }
        if matches!(command, RangeCommand::Minimum) && self.min.is_none()
            || matches!(command, RangeCommand::Maximum) && self.max.is_none()
        {
            return Err(Error::InvalidEvent(
                "spinbutton endpoint command requires a known bound",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_and_outside_values_are_distinct_and_finite_metadata_is_validated() {
        let mut value = SpinValue::default();
        assert!(value.validate().is_ok());
        value.min = Some(0.);
        value.max = Some(10.);
        value.value = Some(20.);
        assert!(
            value.validate().is_ok(),
            "outside values can remain visible with separate invalid metadata"
        );
        for nonfinite in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            value.value = Some(nonfinite);
            assert!(value.validate().is_err())
        }
        value.value = None;
        value.min = Some(11.);
        assert!(value.validate().is_err());
        value.min = Some(0.);
        value.step = 0.;
        assert!(value.validate().is_err());
    }
    #[test]
    fn endpoint_commands_require_bounds_and_set_values_are_finite() {
        let mut value = SpinValue::default();
        assert!(value.validate_command(RangeCommand::Minimum).is_err());
        assert!(value.validate_command(RangeCommand::Maximum).is_err());
        assert!(value.validate_command(RangeCommand::Set(f64::NAN)).is_err());
        value.min = Some(0.);
        assert!(value.validate_command(RangeCommand::Minimum).is_ok());
        value.value_text = Some("x".repeat(1025));
        assert!(value.validate().is_err());
    }
    #[test]
    fn spin_keys_allow_editable_origins_and_respect_caret_composition_and_modifier_keys() {
        let mut input = KeyboardInput {
            key: "ArrowUp",
            control: false,
            meta: false,
            alt: false,
            shift: false,
            repeat: false,
            composing: false,
            editable: true,
        };
        assert_eq!(
            RangeCommand::spin_key(false, false, input),
            Some(RangeCommand::Increase)
        );
        for key in [
            "ArrowLeft",
            "ArrowRight",
            "Delete",
            "Backspace",
            "Home",
            "End",
        ] {
            input.key = key;
            assert_eq!(RangeCommand::spin_key(false, false, input), None);
        }
        input.key = "Home";
        assert_eq!(
            RangeCommand::spin_key(true, false, input),
            Some(RangeCommand::Minimum)
        );
        input.key = "End";
        assert_eq!(
            RangeCommand::spin_key(false, true, input),
            Some(RangeCommand::Maximum)
        );
        input.key = "ArrowUp";
        input.composing = true;
        assert_eq!(RangeCommand::spin_key(true, true, input), None);
        input.composing = false;
        input.shift = true;
        assert_eq!(RangeCommand::spin_key(true, true, input), None);
    }
}
