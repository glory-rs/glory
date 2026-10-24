//! Finite layout declarations. These values describe CSS layout, not an
//! interactive grid/table accessibility model.
pub const MAX_GRID_TRACKS: usize = 64;
pub const MIN_ASPECT_RATIO: f64 = 0.000001;
pub const MAX_ASPECT_RATIO: f64 = 1_000_000.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridTrack {
    Auto,
    Px(f64),
    Percent(f64),
    Fr(f64),
}
impl GridTrack {
    pub(crate) fn validate(self) -> Result<(), &'static str> {
        match self {
            Self::Auto => Ok(()),
            Self::Px(value) | Self::Percent(value) if value.is_finite() && value >= 0.0 => Ok(()),
            Self::Fr(value) if value.is_finite() && value > 0.0 => Ok(()),
            _ => Err("grid tracks require finite nonnegative px/percent or positive fr"),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub enum GridTracks {
    #[default]
    None,
    Tracks(Vec<GridTrack>),
}
impl GridTracks {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::None => Ok(()),
            Self::Tracks(tracks) if !(1..=MAX_GRID_TRACKS).contains(&tracks.len()) => {
                Err("grid track lists require 1..=64 tracks")
            }
            Self::Tracks(tracks) => {
                for track in tracks {
                    track.validate()?;
                }
                Ok(())
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridPlacement {
    #[default]
    Auto,
    Span(u16),
    At {
        start: u16,
        span: u16,
    },
}
impl GridPlacement {
    pub(crate) fn validate(self) -> Result<(), &'static str> {
        match self {
            Self::Auto => Ok(()),
            Self::Span(span) if (1..=64).contains(&span) => Ok(()),
            Self::At { start, span }
                if (1..=64).contains(&start)
                    && (1..=64).contains(&span)
                    && u32::from(start) + u32::from(span) <= 65 =>
            {
                Ok(())
            }
            _ => Err("grid placement requires start/span in 1..=64 and end <=65"),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum AspectRatio {
    #[default]
    Auto,
    Ratio(f64),
}
impl AspectRatio {
    pub(crate) fn validate(self) -> Result<(), &'static str> {
        match self {
            Self::Auto => Ok(()),
            Self::Ratio(value)
                if value.is_finite() && (MIN_ASPECT_RATIO..=MAX_ASPECT_RATIO).contains(&value) =>
            {
                Ok(())
            }
            _ => Err("aspect ratio must be finite and in [0.000001,1000000]"),
        }
    }
}
