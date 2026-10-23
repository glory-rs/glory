//! Logical drag phases. Coordinates use viewport CSS pixels in every host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragPhase {
    Start,
    Move,
    Drop,
    Cancel,
}
