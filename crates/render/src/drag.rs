//! Logical drag phases. Coordinates use viewport CSS pixels in every host.
/// Immediate primary-pointer capture on a boxed View. Applications provide
/// separate semantic keyboard controls; capture does not invent a control role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragCapture {
    /// Preserve the current actual focus, such as an adjacent text editor.
    Press,
    /// Focus the View, which must declare Programmatic or TabStop focus.
    PressFocus,
}
impl DragCapture {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Press => "press",
            Self::PressFocus => "press-focus",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragPhase {
    Start,
    Move,
    Drop,
    Cancel,
}
