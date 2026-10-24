//! Two-dimensional grid navigation, independent of collection orientation.
use crate::{Direction, KeyboardInput};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridCommand {
    Left,
    Right,
    Up,
    Down,
    FirstInRow,
    LastInRow,
    PreviousPage,
    NextPage,
    /// Shift+PageUp; the grid defines its finite larger page interval.
    PreviousLargePage,
    /// Shift+PageDown; the grid defines its finite larger page interval.
    NextLargePage,
}
impl GridCommand {
    pub fn key(direction: Direction, input: KeyboardInput<'_>) -> Option<Self> {
        if input.composing || input.editable || input.control || input.meta || input.alt {
            return None;
        }
        match (input.key, input.shift) {
            ("PageUp", true) => Some(Self::PreviousLargePage),
            ("PageDown", true) => Some(Self::NextLargePage),
            (_, true) => None,
            ("ArrowLeft", false) => Some(if direction == Direction::Rtl {
                Self::Right
            } else {
                Self::Left
            }),
            ("ArrowRight", false) => Some(if direction == Direction::Rtl {
                Self::Left
            } else {
                Self::Right
            }),
            ("ArrowUp", false) => Some(Self::Up),
            ("ArrowDown", false) => Some(Self::Down),
            ("Home", false) => Some(Self::FirstInRow),
            ("End", false) => Some(Self::LastInRow),
            ("PageUp", false) => Some(Self::PreviousPage),
            ("PageDown", false) => Some(Self::NextPage),
            _ => None,
        }
    }
}
