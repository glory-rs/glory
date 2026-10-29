use crate::location::{decode, encode};
use crate::{Error, Location, Result, Routes};

/// Bounded, versioned, inert route seed. Its ASCII payload can be placed in a
/// non-executable script element; it never contains '<' or a closing script tag.
/// Carries no credentials, NodeIds, Scope IDs or arbitrary parameter objects.
#[derive(Clone, Debug)]
pub struct Bootstrap {
    schema: String,
    location: Location,
    chain: Vec<String>,
}
impl Bootstrap {
    pub fn new(routes: &Routes, location: Location, codec_schema: &str) -> Result<Self> {
        let chain = match routes.resolve(&location) {
            Ok(plan) => plan
                .chain
                .into_iter()
                .map(|entry| entry.id.0.into())
                .collect(),
            Err(Error::NoMatch) => Vec::new(),
            Err(error) => return Err(error),
        };
        Ok(Self {
            schema: routes.schema(codec_schema),
            location,
            chain,
        })
    }
    pub fn validate(&self, routes: &Routes, codec_schema: &str) -> Result<Location> {
        let expected = Self::new(routes, self.location.clone(), codec_schema)?;
        if self.schema != expected.schema || self.chain != expected.chain {
            return Err(Error::HydrationMismatch);
        }
        Ok(self.location.clone())
    }
    pub fn to_payload(&self) -> String {
        let mut lines = vec![
            "glory-routing/1".to_owned(),
            self.schema.clone(),
            encode(&self.location.to_string()),
        ];
        lines.extend(self.chain.iter().map(|name| encode(name)));
        lines.join("\n")
    }
    pub fn from_payload(payload: &str) -> Result<Self> {
        if payload.len() > 64 * 1024 {
            return Err(Error::OverLimit("route bootstrap"));
        }
        let mut lines = payload.lines();
        if lines.next() != Some("glory-routing/1") {
            return Err(Error::HydrationMismatch);
        }
        let schema = lines
            .next()
            .filter(|value| value.len() == 16 && value.chars().all(|ch| ch.is_ascii_hexdigit()))
            .ok_or(Error::HydrationMismatch)?
            .to_owned();
        let location = Location::parse(&decode(lines.next().ok_or(Error::HydrationMismatch)?)?)?;
        let mut chain = Vec::new();
        for name in lines {
            if chain.len() >= 64 {
                return Err(Error::OverLimit("bootstrap branches"));
            }
            let name = decode(name)?;
            if name.is_empty() || name.len() > 128 {
                return Err(Error::HydrationMismatch);
            }
            chain.push(name);
        }
        Ok(Self {
            schema,
            location,
            chain,
        })
    }
}
