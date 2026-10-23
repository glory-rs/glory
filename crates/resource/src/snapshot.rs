use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, names};

pub const VERSION: u32 = 1;
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub records: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: 8 * 1024 * 1024,
            records: 4096,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub name: String,
    pub schema: String,
    pub key: Value,
    pub result: Outcome,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "lowercase",
    deny_unknown_fields
)]
pub(crate) enum Outcome {
    Ready(Value),
    Failed(Value),
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u32,
    records: Vec<Record>,
}

/// JSON values are decoded only when a resource explicitly binds its name and
/// schema. Keys are application data, never RuntimeId/NodeId or DOM paths.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub(crate) records: Vec<Record>,
    pub(crate) limits: Limits,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self::new(Limits::default())
    }
}
impl Snapshot {
    pub fn new(limits: Limits) -> Self {
        Self {
            records: Vec::new(),
            limits,
        }
    }
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    pub fn limits(&self) -> Limits {
        self.limits
    }
    pub fn from_json(json: &str, limits: Limits) -> Result<Self, Error> {
        if json.len() > limits.bytes {
            return Err(Error::Limit("snapshot bytes"));
        }
        let wire: Wire = serde_json::from_str(json)?;
        if wire.version != VERSION {
            return Err(Error::Version(wire.version));
        }
        let snapshot = Self {
            records: wire.records,
            limits,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
    pub fn to_json(&self) -> Result<String, Error> {
        self.validate()?;
        #[derive(Serialize)]
        struct RefWire<'a> {
            version: u32,
            records: &'a [Record],
        }
        let json = serde_json::to_string(&RefWire {
            version: VERSION,
            records: &self.records,
        })?;
        if json.len() > self.limits.bytes {
            return Err(Error::Limit("snapshot bytes"));
        }
        Ok(json)
    }
    /// Encode prefetched request data. Serializing is explicit: arbitrary Cages
    /// and resources that were not registered never enter the payload.
    pub fn seed<K: Serialize, T: Serialize, E: Serialize>(
        &mut self,
        name: &str,
        schema: &str,
        key: &K,
        result: Result<&T, &E>,
    ) -> Result<(), Error> {
        names(name, schema)?;
        if self.records.iter().any(|record| record.name == name) {
            return Err(Error::Duplicate(name.into()));
        }
        if self.records.len() >= self.limits.records {
            return Err(Error::Limit("snapshot records"));
        }
        let record = encode(name, schema, key, result)?;
        self.records.push(record);
        if let Err(error) = self.to_json() {
            self.records.pop();
            return Err(error);
        }
        Ok(())
    }
    fn validate(&self) -> Result<(), Error> {
        if self.records.len() > self.limits.records {
            return Err(Error::Limit("snapshot records"));
        }
        let mut names_seen = BTreeSet::new();
        for record in &self.records {
            names(&record.name, &record.schema)?;
            if !names_seen.insert(&record.name) {
                return Err(Error::Duplicate(record.name.clone()));
            }
        }
        Ok(())
    }
}
pub(crate) fn encode<K: Serialize, T: Serialize, E: Serialize>(
    name: &str,
    schema: &str,
    key: &K,
    result: Result<&T, &E>,
) -> Result<Record, Error> {
    Ok(Record {
        name: name.into(),
        schema: schema.into(),
        key: serde_json::to_value(key)?,
        result: match result {
            Ok(value) => Outcome::Ready(serde_json::to_value(value)?),
            Err(error) => Outcome::Failed(serde_json::to_value(error)?),
        },
    })
}
