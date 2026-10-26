//! Finite input hints. Hosts never retrieve a code or credential on this basis.
pub const MAX_INPUT_PLACEHOLDER_BYTES: usize = 4096;

/// Validate a presentation hint without rewriting an editor's business value.
pub fn validate_input_placeholder(value: &str, multiline: bool) -> Result<(), crate::Error> {
    if value.len() > MAX_INPUT_PLACEHOLDER_BYTES
        || value.contains(['\0', '\r'])
        || (!multiline && value.contains('\n'))
    {
        return Err(crate::Error::InvalidProps(
            "invalid bounded input placeholder",
        ));
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputAutofill {
    Off,
    OneTimeCode,
}
impl InputAutofill {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::OneTimeCode => "one-time-code",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Text,
    Numeric,
}
impl InputMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Numeric => "numeric",
        }
    }
}
/// Omitted members preserve the intrinsic Purpose hint. No actual OS keyboard
/// or autofill service is implied; native hosts may retain these as hints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputAssist {
    pub autofill: Option<InputAutofill>,
    pub mode: Option<InputMode>,
}
