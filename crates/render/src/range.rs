use crate::Error;

/// A finite, inclusive numeric position in a nonempty range.
///
/// This is a value constraint, not an interactive or accessibility role.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeValue {
    pub min: f64,
    pub max: f64,
    pub value: f64,
}
impl RangeValue {
    pub fn validate(self) -> Result<(), Error> {
        if !self.min.is_finite() || !self.max.is_finite() || self.min >= self.max {
            return Err(Error::InvalidProps(
                "range bounds must be finite and increasing",
            ));
        }
        if !self.value.is_finite() || self.value < self.min || self.value > self.max {
            return Err(Error::InvalidProps(
                "range value must be finite and within its bounds",
            ));
        }
        Ok(())
    }
    /// Unit progress calculated without overflowing a finite wide range.
    pub fn fraction(self) -> Result<f64, Error> {
        self.validate()?;
        let span = self.max - self.min;
        Ok(if span.is_finite() {
            (self.value - self.min) / span
        } else {
            (self.value / 2.0 - self.min / 2.0) / (self.max / 2.0 - self.min / 2.0)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_and_values_are_checked_independently() {
        for min in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 10.0] {
            assert!(
                RangeValue {
                    min,
                    max: 10.0,
                    value: 5.0
                }
                .validate()
                .is_err()
            );
        }
        for max in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0] {
            assert!(
                RangeValue {
                    min: 0.0,
                    max,
                    value: 0.0
                }
                .validate()
                .is_err()
            );
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 11.0] {
            assert!(
                RangeValue {
                    min: 0.0,
                    max: 10.0,
                    value
                }
                .validate()
                .is_err()
            );
        }
        for value in [0.0, 5.0, 10.0] {
            assert!(
                RangeValue {
                    min: 0.0,
                    max: 10.0,
                    value
                }
                .validate()
                .is_ok()
            );
        }
    }
    #[test]
    fn wide_finite_range_fraction_does_not_overflow() {
        for (value, expected) in [(-f64::MAX, 0.0), (0.0, 0.5), (f64::MAX, 1.0)] {
            assert_eq!(
                RangeValue {
                    min: -f64::MAX,
                    max: f64::MAX,
                    value
                }
                .fraction()
                .unwrap(),
                expected
            );
        }
    }
}
