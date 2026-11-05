use crate::Error;

/// Finite host-owned graphic motion. There is no application frame task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphicAnimation {
    Rotate {
        period_millis: u32,
    },
    /// A linear opacity wave between one quarter and full opacity. Phase is
    /// bounded within one period; Reduce presents the fully opaque shape.
    Pulse {
        period_millis: u32,
        phase_millis: u32,
    },
}
impl GraphicAnimation {
    pub fn validate(self) -> Result<(), Error> {
        match self {
            Self::Rotate { period_millis } if (16..=60000).contains(&period_millis) => Ok(()),
            Self::Pulse {
                period_millis,
                phase_millis,
            } if (16..=60000).contains(&period_millis) && phase_millis < period_millis => Ok(()),
            _ => Err(Error::InvalidProps(
                "graphic motion period must be in 16..=60000 milliseconds and pulse phase within its period",
            )),
        }
    }
    /// The phase stays finite even for a large finite host timestamp.
    pub fn radians(self, seconds: f64) -> Result<f64, Error> {
        self.validate()?;
        if !seconds.is_finite() || seconds < 0. {
            return Err(Error::InvalidProps(
                "graphic clock must be finite and nonnegative",
            ));
        }
        let Self::Rotate { period_millis } = self else {
            return Err(Error::InvalidProps("graphic radians requires rotation"));
        };
        let period = f64::from(period_millis) / 1000.;
        Ok(seconds.rem_euclid(period) / period * std::f64::consts::TAU)
    }
    pub fn opacity(self, seconds: f64) -> Result<f64, Error> {
        self.validate()?;
        if !seconds.is_finite() || seconds < 0. {
            return Err(Error::InvalidProps(
                "graphic clock must be finite and nonnegative",
            ));
        }
        let Self::Pulse {
            period_millis,
            phase_millis,
        } = self
        else {
            return Ok(1.);
        };
        let period = f64::from(period_millis) / 1000.;
        let phase = ((seconds.rem_euclid(period) + f64::from(phase_millis) / 1000.) / period)
            .rem_euclid(1.);
        let wave = if phase <= 0.5 {
            phase * 2.
        } else {
            (1. - phase) * 2.
        };
        Ok(0.25 + 0.75 * wave)
    }
}

/// Native hosts explicitly choose their preference. Unknown defaults to Reduce.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MotionPreference {
    #[default]
    Reduce,
    NoPreference,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn periods_and_clock_values_are_independently_bounded() {
        for period_millis in [0, 1, 15, 60001, u32::MAX] {
            assert!(
                GraphicAnimation::Rotate { period_millis }
                    .validate()
                    .is_err()
            );
        }
        for period_millis in [16, 1200, 60000] {
            GraphicAnimation::Rotate { period_millis }
                .validate()
                .unwrap();
        }
        let animation = GraphicAnimation::Rotate {
            period_millis: 1000,
        };
        for seconds in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.] {
            assert!(animation.radians(seconds).is_err());
        }
        assert_eq!(animation.radians(0.).unwrap(), 0.);
        assert!((animation.radians(0.25).unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(animation.radians(f64::MAX).unwrap().is_finite());
    }
    #[test]
    fn pulses_have_bounded_phases_finite_clocks_and_a_linear_opacity_wave() {
        let pulse = GraphicAnimation::Pulse {
            period_millis: 1000,
            phase_millis: 0,
        };
        for (seconds, expected) in [
            (0., 0.25),
            (0.25, 0.625),
            (0.5, 1.),
            (0.75, 0.625),
            (1., 0.25),
        ] {
            assert!((pulse.opacity(seconds).unwrap() - expected).abs() < 1e-12);
        }
        assert_eq!(
            GraphicAnimation::Pulse {
                period_millis: 1000,
                phase_millis: 500
            }
            .opacity(0.)
            .unwrap(),
            1.
        );
        for config in [
            GraphicAnimation::Pulse {
                period_millis: 15,
                phase_millis: 0,
            },
            GraphicAnimation::Pulse {
                period_millis: 60001,
                phase_millis: 0,
            },
            GraphicAnimation::Pulse {
                period_millis: 1000,
                phase_millis: 1000,
            },
            GraphicAnimation::Pulse {
                period_millis: 16,
                phase_millis: u32::MAX,
            },
        ] {
            assert!(config.validate().is_err());
        }
        for seconds in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.] {
            assert!(pulse.opacity(seconds).is_err());
        }
        assert!((0.25..=1.).contains(&pulse.opacity(f64::MAX).unwrap()));
        assert!(pulse.radians(0.).is_err());
    }
}
