//! Opt-in bounded post-edit observations. A host never calls application code
//! from a native listener. Ordinary Inputs keep ValueChanged unchanged.
use crate::{Error, TextSelection};
pub const MAX_INPUT_EDIT_BYTES: usize = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputEditCapture {
    pub max_bytes: usize,
}
impl InputEditCapture {
    pub fn validate(self) -> Result<(), Error> {
        if self.max_bytes > 0 && self.max_bytes <= MAX_INPUT_EDIT_BYTES {
            Ok(())
        } else {
            Err(Error::InvalidProps(
                "input edit byte budget must be between 1 and 4096",
            ))
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputEditIntent {
    Insert,
    DeleteBackward,
    DeleteForward,
    #[default]
    Replace,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEditRejection {
    TooLong,
    SelectionUnavailable,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputEdit {
    Value {
        value: String,
        revision: u64,
        selection: TextSelection,
        before: Option<TextSelection>,
        intent: InputEditIntent,
        composing: bool,
    },
    Rejected {
        revision: u64,
        reason: InputEditRejection,
    },
}
impl InputEdit {
    pub fn revision(&self) -> u64 {
        match self {
            Self::Value { revision, .. } | Self::Rejected { revision, .. } => *revision,
        }
    }
    pub fn validate(&self, capture: InputEditCapture) -> Result<(), Error> {
        capture.validate()?;
        if let Self::Value {
            value,
            selection,
            before,
            ..
        } = self
        {
            if value.len() > capture.max_bytes || value.contains(['\0', '\r', '\n']) {
                return Err(Error::InvalidEvent(
                    "input edit exceeds its budget or is not normalized single-line text",
                ));
            }
            selection
                .validate(value.encode_utf16().count())
                .map_err(|_| Error::InvalidEvent("invalid input edit selection"))?;
            if let Some(before) = before {
                before
                    .validate(capture.max_bytes)
                    .map_err(|_| Error::InvalidEvent("invalid prior input edit selection"))?;
            }
        }
        Ok(())
    }
}
