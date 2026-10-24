use crate::{Error, RangeValue};
/// Read-only progress. `None` is indeterminate, never an invented zero value.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgressValue {
    pub min: f64,
    pub max: f64,
    pub current: Option<f64>,
    pub value_text: Option<String>,
}
impl ProgressValue {
    pub fn validate(&self) -> Result<(), Error> {
        RangeValue {
            min: self.min,
            max: self.max,
            value: self.current.unwrap_or(self.min),
        }
        .validate()?;
        if self
            .value_text
            .as_ref()
            .is_some_and(|text| text.trim().is_empty() || text.len() > 1024)
        {
            return Err(Error::InvalidProps(
                "progress text must be nonempty and at most 1024 UTF-8 bytes",
            ));
        }
        Ok(())
    }
    pub fn fraction(&self) -> Result<Option<f64>, Error> {
        self.validate()?;
        self.current
            .map(|value| {
                RangeValue {
                    min: self.min,
                    max: self.max,
                    value,
                }
                .fraction()
            })
            .transpose()
    }
}
