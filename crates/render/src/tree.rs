//! Finite tree presentation and keyboard commands. No data source lives here.
use crate::{Direction, Error, KeyboardInput};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeCheckState {
    Unchecked,
    Checked,
    Mixed,
}
impl TreeCheckState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unchecked => "false",
            Self::Checked => "true",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeItem {
    /// One-based depth, independently bounded from sibling position.
    pub level: u8,
    /// Zero-based position in the current logical sibling set.
    pub position: u16,
    pub count: u16,
}
impl TreeItem {
    pub fn validate(self) -> Result<(), Error> {
        if self.level == 0 || self.level > 64 || self.count == 0 || self.position >= self.count {
            Err(Error::InvalidProps(
                "tree item requires finite level and sibling coordinates",
            ))
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeNavigation {
    pub direction: Direction,
    pub typeahead: bool,
    pub multiple: bool,
}
impl Default for TreeNavigation {
    fn default() -> Self {
        Self {
            direction: Direction::Ltr,
            typeahead: true,
            multiple: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeCommand {
    Previous,
    Next,
    First,
    Last,
    Open,
    Close,
    Select,
    Activate,
    Search { text: String, at_millis: u64 },
}
impl TreeCommand {
    pub fn key(config: TreeNavigation, input: KeyboardInput<'_>, at_millis: u64) -> Option<Self> {
        if input.composing || input.editable || input.control || input.meta || input.alt {
            return None;
        }
        if !input.shift {
            let command = match input.key {
                "ArrowUp" => Some(Self::Previous),
                "ArrowDown" => Some(Self::Next),
                "Home" => Some(Self::First),
                "End" => Some(Self::Last),
                "ArrowRight" => Some(if config.direction == Direction::Rtl {
                    Self::Close
                } else {
                    Self::Open
                }),
                "ArrowLeft" => Some(if config.direction == Direction::Rtl {
                    Self::Open
                } else {
                    Self::Close
                }),
                " " if !input.repeat => Some(Self::Select),
                "Enter" if !input.repeat => Some(Self::Activate),
                _ => None,
            };
            if command.is_some() {
                return command;
            }
        }
        (config.typeahead && input.typeahead(at_millis).is_some()).then(|| Self::Search {
            text: input.key.to_owned(),
            at_millis,
        })
    }
    pub fn validate(&self) -> Result<(), Error> {
        if let Self::Search { text, at_millis } = self {
            crate::validate_typeahead(text, *at_millis)?;
        }
        Ok(())
    }
}
