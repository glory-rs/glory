use crate::{Error, Result};
use std::collections::BTreeMap;

/// Catch-all captures retain segment boundaries, including encoded slashes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Params(pub BTreeMap<String, Vec<String>>);
impl Params {
    pub fn required<T: std::str::FromStr>(&self, name: &str) -> Result<T> {
        self.optional(name)?
            .ok_or_else(|| Error::BadParams(format!("missing path parameter: {name}")))
    }
    pub fn optional<T: std::str::FromStr>(&self, name: &str) -> Result<Option<T>> {
        match self.0.get(name) {
            None => Ok(None),
            Some(values) if values.len() == 1 => values[0]
                .parse()
                .map(Some)
                .map_err(|_| Error::BadParams(name.to_owned())),
            _ => Err(Error::BadParams(format!("expected one segment: {name}"))),
        }
    }
    pub fn rest(&self, name: &str) -> Result<&[String]> {
        self.0
            .get(name)
            .map(Vec::as_slice)
            .ok_or_else(|| Error::BadParams(name.to_owned()))
    }
    pub fn insert(&mut self, name: impl Into<String>, value: impl ToString) {
        self.0.insert(name.into(), vec![value.to_string()]);
    }
}
/// Pure codecs. Implementations must not perform I/O or create reactive state.
pub trait RouteParams: 'static {
    fn decode(params: &Params) -> Result<Self>
    where
        Self: Sized;
    fn encode(&self) -> Result<Params>;
}
impl RouteParams for () {
    fn decode(_: &Params) -> Result<Self> {
        Ok(())
    }
    fn encode(&self) -> Result<Params> {
        Ok(Params::default())
    }
}
impl RouteParams for Params {
    fn decode(params: &Params) -> Result<Self> {
        Ok(params.clone())
    }
    fn encode(&self) -> Result<Params> {
        Ok(self.clone())
    }
}
