//! Finite inherited direction and language metadata, independent of UI locale
//! tables, application text, fonts, editors and platform APIs.
use crate::{Direction, Error, NodeId, memory::Memory};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageTag(String);
impl LanguageTag {
    /// A bounded ASCII language tag syntax. This does not consult a language
    /// registry or silently infer a locale's writing direction.
    pub fn new(value: impl AsRef<str>) -> Result<Self, Error> {
        let value = value.as_ref();
        if value.len() > 63 || value.len() < 2 || !value.is_ascii() {
            return Err(Error::InvalidProps("invalid bounded language tag"));
        }
        let mut pieces = value.split('-');
        let first = pieces.next().expect("nonempty language tag");
        if !(2..=8).contains(&first.len())
            || !first.bytes().all(|byte| byte.is_ascii_alphabetic())
            || pieces.any(|part| {
                part.is_empty()
                    || part.len() > 8
                    || !part.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
        {
            return Err(Error::InvalidProps("invalid bounded language tag"));
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    pub direction: Direction,
    pub language: LanguageTag,
}
impl Memory {
    /// Current semantic ancestry, including unplaced and suspended nodes. The
    /// caller still applies its host's liveness and capability rules separately.
    pub fn inherited_environment(&self, mut id: NodeId) -> Result<Option<&Environment>, Error> {
        loop {
            let node = self.node(id)?;
            if let Some(environment) = node
                .descriptor
                .view()
                .semantics
                .as_ref()
                .and_then(|value| value.environment.as_ref())
            {
                return Ok(Some(environment));
            }
            let Some(parent) = node.parent else {
                return Ok(None);
            };
            id = parent;
        }
    }
}
