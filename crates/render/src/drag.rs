//! Logical drag phases. Coordinates use viewport CSS pixels in every host.
/// Primary-pointer capture on a boxed View. Applications provide
/// separate semantic keyboard controls; capture does not invent a control role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragCapture {
    /// Preserve the current actual focus, such as an adjacent text editor.
    Press,
    /// Focus the View, which must declare Programmatic or TabStop focus.
    PressFocus,
    /// Wry preserves clicks until the pointer moves six CSS pixels, then captures
    /// it and routes Drop to the nearest live Drag observer under the pointer.
    Threshold,
    /// Like Threshold, with a Wry preview of the source View's accessible name.
    ThresholdPreview,
}
impl DragCapture {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Press => "press",
            Self::PressFocus => "press-focus",
            Self::Threshold => "threshold",
            Self::ThresholdPreview => "threshold-preview",
        }
    }

    pub fn is_threshold(self) -> bool {
        matches!(self, Self::Threshold | Self::ThresholdPreview)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragPhase {
    Start,
    Move,
    Drop,
    Cancel,
}
