use crate::Error;

/// A finite host-owned graphic rotation. There is no application frame task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphicAnimation {
    Rotate { period_millis: u32 },
}
impl GraphicAnimation {
    pub fn validate(self) -> Result<(), Error> {
        match self {
            Self::Rotate { period_millis } if (16..=60000).contains(&period_millis) => Ok(()),
            _ => Err(Error::InvalidProps(
                "graphic rotation period must be in 16..=60000 milliseconds",
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
        let Self::Rotate { period_millis } = self;
        let period = f64::from(period_millis) / 1000.;
        Ok(seconds.rem_euclid(period) / period * std::f64::consts::TAU)
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
}
