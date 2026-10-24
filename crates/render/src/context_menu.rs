//! Opted-in context requests; coordinates are viewport CSS pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContextEditPolicy {
    /// Preserve the host editor's own context commands.
    #[default]
    Native,
    Override,
}
impl ContextEditPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native-editors",
            Self::Override => "override-editors",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextSource {
    Pointer,
    Keyboard,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContextPoint {
    pub x: f64,
    pub y: f64,
}
impl ContextPoint {
    pub fn validate(self) -> Result<(), crate::Error> {
        if !self.x.is_finite() || !self.y.is_finite() {
            return Err(crate::Error::InvalidEvent(
                "context point must be finite viewport CSS pixels",
            ));
        }
        Ok(())
    }
}
impl crate::KeyboardInput<'_> {
    pub fn context_menu(self) -> bool {
        !self.composing
            && !self.repeat
            && !self.control
            && !self.alt
            && !self.meta
            && ((self.key == "ContextMenu" && !self.shift) || (self.key == "F10" && self.shift))
    }
}
