//! Finite input hints. Hosts never retrieve a code or credential on this basis.
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
