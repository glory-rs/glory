//! Exact current-value selections use UTF-16 offsets. Program selections must
//! lie on Unicode scalar boundaries; hosts must reject unrepresentable layout
//! positions rather than silently clamping a requested range.
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

    /// Convert an ordered UTF-16 range to UTF-8 without splitting a surrogate
    /// pair. This inspects only the supplied current input value and allocates
    /// no intermediate string or UTF-16 buffer. Direction is preserved separately.
    pub fn byte_range(self, text: &str) -> Result<std::ops::Range<usize>, crate::Error> {
        let invalid = || crate::Error::InvalidProps("invalid UTF-16 text selection boundary");
        if self.start > self.end {
            return Err(invalid());
        }
        let mut start = None;
        let mut units = 0;
        for (byte, ch) in text.char_indices() {
            if units == self.start {
                start = Some(byte);
            }
            if units == self.end {
                return Ok(start.ok_or_else(invalid)?..byte);
            }
            units += ch.len_utf16();
            if units > self.end {
                return Err(invalid());
            }
        }
        if units == self.start {
            start = Some(text.len());
        }
        if units == self.end {
            Ok(start.ok_or_else(invalid)?..text.len())
        } else {
            Err(invalid())
        }
    }
}
