//! Finite intrinsic-height presentation. Logical hiding and interaction
//! blocking apply immediately; a matching terminal observation can release
//! delayed content ownership after the physical close finishes.
use crate::Error;

pub const MAX_COLLAPSE_MILLIS: u16 = 500;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollapseConfig {
    pub open: bool,
    pub duration_millis: u16,
    pub revision: u64,
}
impl CollapseConfig {
    pub fn validate(&self) -> Result<(), Error> {
        if self.duration_millis > MAX_COLLAPSE_MILLIS {
            return Err(Error::InvalidProps(
                "collapse duration exceeds 500 milliseconds",
            ));
        }
        Ok(())
    }
    pub(crate) fn validate_style(&self, style: &crate::style::Style) -> Result<(), Error> {
        use crate::style::{BoxShadow, Display, Edges, Overflow, Position};
        if style.height.is_some()
            || style.min_height.is_some()
            || style.max_height.is_some()
            || style.aspect_ratio.is_some()
            || style
                .padding
                .is_some_and(|padding| padding != Edges::all(0.))
            || style
                .border_width
                .is_some_and(|width| width != Edges::all(0.))
            || style
                .box_shadow
                .is_some_and(|shadow| shadow != BoxShadow::None)
            || style.grow.is_some_and(|grow| grow != 0.)
            || style
                .position
                .is_some_and(|position| position != Position::Relative)
            || style
                .display
                .is_some_and(|display| !matches!(display, Display::Block | Display::None))
            || style
                .overflow_x
                .is_some_and(|overflow| overflow != Overflow::Hidden)
            || style
                .overflow_y
                .is_some_and(|overflow| overflow != Overflow::Hidden)
        {
            return Err(Error::InvalidProps(
                "collapse owns an intrinsic block height and clipping; place sizing, padding, border, shadow and flexible layout on its content",
            ));
        }
        Ok(())
    }
}

pub(crate) fn revision(
    previous: Option<&CollapseConfig>,
    next: Option<&CollapseConfig>,
) -> Result<(), Error> {
    let Some(previous) = previous else {
        return Ok(());
    };
    let Some(next) = next else {
        return Err(Error::InvalidProps(
            "a live collapse declaration cannot be removed",
        ));
    };
    if next.revision < previous.revision || next.revision == previous.revision && next != previous {
        return Err(Error::CollapseRevision {
            current: previous.revision,
            incoming: next.revision,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_duration_includes_explicit_immediate_settlement() {
        for duration_millis in [0, 1, 240, MAX_COLLAPSE_MILLIS] {
            assert!(
                CollapseConfig {
                    duration_millis,
                    ..Default::default()
                }
                .validate()
                .is_ok()
            );
        }
        assert!(
            CollapseConfig {
                duration_millis: 501,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn config_revisions_are_monotonic_and_equal_revisions_require_equal_config() {
        let previous = CollapseConfig {
            open: true,
            duration_millis: 240,
            revision: 8,
        };
        assert!(revision(Some(&previous), Some(&previous)).is_ok());
        for next in [
            CollapseConfig {
                revision: 7,
                ..previous
            },
            CollapseConfig {
                open: false,
                ..previous
            },
            CollapseConfig {
                duration_millis: 0,
                ..previous
            },
        ] {
            assert!(matches!(
                revision(Some(&previous), Some(&next)),
                Err(Error::CollapseRevision { .. })
            ));
        }
        assert!(
            revision(
                Some(&previous),
                Some(&CollapseConfig {
                    open: false,
                    revision: 9,
                    ..previous
                })
            )
            .is_ok()
        );
        assert!(revision(Some(&previous), None).is_err());
    }
}
