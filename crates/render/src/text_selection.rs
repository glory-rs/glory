//! Exact current-value ASCII selection. Unicode grapheme addressing is outside
//! this action's initial contract; hosts must not silently clamp it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionDirection {
    #[default]
    None,
    Forward,
    Backward,
}
impl SelectionDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Forward => "forward",
            Self::Backward => "backward",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextSelection {
    pub start: usize,
    pub end: usize,
    pub direction: SelectionDirection,
}
impl TextSelection {
    pub fn validate(self, utf16_len: usize) -> Result<(), crate::Error> {
        if self.start <= self.end && self.end <= utf16_len {
            Ok(())
        } else {
            Err(crate::Error::InvalidProps("invalid UTF-16 text selection"))
        }
    }
}
