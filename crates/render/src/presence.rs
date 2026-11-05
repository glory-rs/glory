//! Bounded presence presentation. Logical availability follows `open` immediately.
use crate::Error;

pub const MAX_PRESENCE_MILLIS: u16 = 2000;
/// Closed presentation; opacity/scale use thousandths, translations CSS pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresenceStyle {
    pub opacity: u16,
    pub scale_x: u16,
    pub scale_y: u16,
    pub translate_x: i16,
    pub translate_y: i16,
    pub rotate: i16,
    pub skew_x: i16,
    pub skew_y: i16,
    pub origin_x: u8,
    pub origin_y: u8,
}
impl Default for PresenceStyle {
    fn default() -> Self {
        Self {
            opacity: 0,
            scale_x: 1000,
            scale_y: 1000,
            translate_x: 0,
            translate_y: 0,
            rotate: 0,
            skew_x: 0,
            skew_y: 0,
            origin_x: 50,
            origin_y: 50,
        }
    }
}
impl PresenceStyle {
    pub fn validate(self) -> Result<(), Error> {
        if self.opacity > 1000
            || self.scale_x > 2000
            || self.scale_y > 2000
            || self.translate_x.unsigned_abs() > 200
            || self.translate_y.unsigned_abs() > 200
            || self.rotate.unsigned_abs() > 180
            || self.skew_x.unsigned_abs() > 45
            || self.skew_y.unsigned_abs() > 45
            || self.origin_x > 100
            || self.origin_y > 100
        {
            return Err(Error::InvalidProps(
                "presence style exceeds opacity/scale/translation/angle/origin bounds",
            ));
        }
        Ok(())
    }
    pub fn has_transform(self) -> bool {
        self.scale_x != 1000
            || self.scale_y != 1000
            || self.translate_x != 0
            || self.translate_y != 0
            || self.rotate != 0
            || self.skew_x != 0
            || self.skew_y != 0
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresenceEffect {
    None,
    #[default]
    Fade,
    FadeUp,
    FadeDown,
    FadeLeft,
    FadeRight,
    Scale,
    ScaleX,
    ScaleY,
    SlideUp,
    SlideDown,
    SlideLeft,
    SlideRight,
    SkewUp,
    SkewDown,
    RotateLeft,
    RotateRight,
    Pop,
    PopBottomLeft,
    PopBottomRight,
    PopTopLeft,
    PopTopRight,
    Custom(PresenceStyle),
}
impl PresenceEffect {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Fade => "fade",
            Self::FadeUp => "fade-up",
            Self::FadeDown => "fade-down",
            Self::FadeLeft => "fade-left",
            Self::FadeRight => "fade-right",
            Self::Scale => "scale",
            Self::ScaleX => "scale-x",
            Self::ScaleY => "scale-y",
            Self::SlideUp => "slide-up",
            Self::SlideDown => "slide-down",
            Self::SlideLeft => "slide-left",
            Self::SlideRight => "slide-right",
            Self::SkewUp => "skew-up",
            Self::SkewDown => "skew-down",
            Self::RotateLeft => "rotate-left",
            Self::RotateRight => "rotate-right",
            Self::Pop => "pop",
            Self::PopBottomLeft => "pop-bottom-left",
            Self::PopBottomRight => "pop-bottom-right",
            Self::PopTopLeft => "pop-top-left",
            Self::PopTopRight => "pop-top-right",
            Self::Custom(_) => "custom",
        }
    }
}
impl PresenceEffect {
    pub fn style(self, distance: u16) -> PresenceStyle {
        let mut style = PresenceStyle::default();
        let distance = distance.min(200) as i16;
        match self {
            Self::Custom(value) => return value,
            Self::None => style.opacity = 1000,
            Self::Fade => (),
            Self::FadeUp | Self::SlideUp => style.translate_y = distance,
            Self::FadeDown | Self::SlideDown => style.translate_y = -distance,
            Self::FadeLeft | Self::SlideLeft => style.translate_x = distance,
            Self::FadeRight | Self::SlideRight => style.translate_x = -distance,
            Self::Scale => {
                style.scale_x = 850;
                style.scale_y = 850;
            }
            Self::ScaleX => style.scale_x = 0,
            Self::ScaleY => style.scale_y = 0,
            Self::SkewUp => {
                style.skew_x = -10;
                style.translate_y = distance;
            }
            Self::SkewDown => {
                style.skew_x = 10;
                style.translate_y = -distance;
            }
            Self::RotateLeft => {
                style.rotate = -12;
                style.origin_x = 0;
            }
            Self::RotateRight => {
                style.rotate = 12;
                style.origin_x = 100;
            }
            Self::Pop
            | Self::PopBottomLeft
            | Self::PopBottomRight
            | Self::PopTopLeft
            | Self::PopTopRight => {
                style.scale_x = 900;
                style.scale_y = 900;
                match self {
                    Self::PopBottomLeft => {
                        style.origin_x = 0;
                        style.origin_y = 100;
                    }
                    Self::PopBottomRight => {
                        style.origin_x = 100;
                        style.origin_y = 100;
                    }
                    Self::PopTopLeft => {
                        style.origin_x = 0;
                        style.origin_y = 0;
                    }
                    Self::PopTopRight => {
                        style.origin_x = 100;
                        style.origin_y = 0;
                    }
                    _ => (),
                }
            }
        }
        if matches!(
            self,
            Self::SlideUp | Self::SlideDown | Self::SlideLeft | Self::SlideRight
        ) {
            style.opacity = 1000;
        }
        style
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresenceTiming {
    Linear,
    #[default]
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
}
impl PresenceTiming {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Ease => "ease",
            Self::EaseIn => "ease-in",
            Self::EaseOut => "ease-out",
            Self::EaseInOut => "ease-in-out",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PresenceConfig {
    pub open: bool,
    /// Animate an initially open declaration. Otherwise its first frame settles.
    pub appear: bool,
    pub effect: PresenceEffect,
    pub timing: PresenceTiming,
    pub duration_millis: u16,
    pub delay_millis: u16,
    pub distance: u16,
    pub revision: u64,
}
impl PresenceConfig {
    pub fn validate(&self) -> Result<(), Error> {
        self.effect.style(self.distance).validate()?;
        if self.duration_millis > MAX_PRESENCE_MILLIS
            || self.delay_millis > MAX_PRESENCE_MILLIS
            || self.distance > 200
        {
            return Err(Error::InvalidProps(
                "presence requires duration/delay <= 2000ms and distance <= 200px",
            ));
        }
        Ok(())
    }
    pub(crate) fn validate_style(&self, style: &crate::style::Style) -> Result<(), Error> {
        if style.opacity.is_some() || style.display.is_some() {
            return Err(Error::InvalidProps(
                "presence owns opacity and display; put custom presentation on its child",
            ));
        }
        Ok(())
    }
}
pub(crate) fn revision(
    previous: Option<&PresenceConfig>,
    next: Option<&PresenceConfig>,
) -> Result<(), Error> {
    if let Some(previous) = previous {
        let next = next.ok_or(Error::InvalidProps(
            "a live presence declaration cannot be removed",
        ))?;
        if next.revision < previous.revision
            || next.revision == previous.revision && next != previous
        {
            return Err(Error::PresenceRevision {
                current: previous.revision,
                incoming: next.revision,
            });
        }
    }
    Ok(())
}
