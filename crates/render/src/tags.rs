//! Finite editor policy for free tags. It does not turn an Input into a combobox.
use crate::{Error, KeyboardInput};

pub const MAX_TAG_INPUT_BYTES: usize = 4096;
pub const MAX_TAG_PASTE_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TagSeparators {
    Enter,
    #[default]
    EnterComma,
    EnterCommaSemicolon,
}
impl TagSeparators {
    pub fn contains(self, character: char) -> bool {
        character == '\n'
            || character == '\r'
            || character == ',' && self != Self::Enter
            || character == ';' && self == Self::EnterCommaSemicolon
    }
    pub fn key(self, key: KeyboardInput<'_>, empty: bool) -> Option<TagKey> {
        if key.composing || key.control || key.meta || key.alt || key.shift || key.repeat {
            return None;
        }
        if key.key == "Backspace" && empty {
            return Some(TagKey::RemoveLast);
        }
        (key.key == "Enter" && !empty
            || key.key == "," && self != Self::Enter
            || key.key == ";" && self == Self::EnterCommaSemicolon)
            .then_some(TagKey::Commit)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::EnterComma => "enter-comma",
            Self::EnterCommaSemicolon => "enter-comma-semicolon",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagKey {
    Commit,
    RemoveLast,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagsCommand {
    /// A host budget refusal contains no rejected clipboard/editor body.
    Reject(TagHostRejection),
    Commit {
        value: String,
    },
    RemoveLast,
    Paste {
        value: String,
        text: String,
        start: u32,
        end: u32,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagHostRejection {
    InputBudget,
    ClipboardBudget,
}
impl TagsCommand {
    pub fn validate(&self) -> Result<(), Error> {
        let check = |value: &str| {
            if value.len() > MAX_TAG_INPUT_BYTES || value.contains(['\0', '\r', '\n']) {
                Err(Error::InvalidEvent(
                    "tags editor requires bounded canonical single-line text",
                ))
            } else {
                Ok(())
            }
        };
        match self {
            Self::Commit { value } => check(value),
            Self::RemoveLast | Self::Reject(_) => Ok(()),
            Self::Paste {
                value,
                text,
                start,
                end,
            } => {
                check(value)?;
                if text.len() > MAX_TAG_PASTE_BYTES
                    || text.contains('\0')
                    || start > end
                    || *end as usize > value.encode_utf16().count()
                {
                    return Err(Error::InvalidEvent(
                        "invalid bounded tags paste or UTF-16 selection",
                    ));
                }
                // Never split a surrogate pair when the component slices its
                // accepted String. Host selection offsets are UTF-16 units.
                for offset in [*start, *end] {
                    let mut position = 0u32;
                    for character in value.chars() {
                        if position == offset {
                            break;
                        }
                        position += character.len_utf16() as u32;
                        if position > offset {
                            return Err(Error::InvalidEvent("tags paste splits a surrogate pair"));
                        }
                    }
                }
                Ok(())
            }
        }
    }
}
