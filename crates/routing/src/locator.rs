use std::collections::BTreeMap;

use multimap::MultiMap;
use url::Url;

use glory_core::reflow::{self, Cage, ReadCage};

#[derive(Default, Clone, Debug)]
pub struct LocatorModifier {
    pub raw_url: String,
    pub replace: bool,
    pub scroll: bool,
}

impl From<String> for LocatorModifier {
    fn from(raw_url: String) -> Self {
        Self {
            raw_url,
            replace: false,
            scroll: true,
        }
    }
}
impl<'a> From<&'a String> for LocatorModifier {
    fn from(raw_url: &'a String) -> Self {
        Self {
            raw_url: raw_url.to_owned(),
            replace: false,
            scroll: true,
        }
    }
}
impl<'a> From<&'a str> for LocatorModifier {
    fn from(raw_url: &'a str) -> Self {
        Self {
            raw_url: raw_url.to_owned(),
            replace: false,
            scroll: true,
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct Locator {
    raw_url: Cage<String>,
    scheme: Cage<String>,
    authority: Cage<String>,
    host: Cage<Option<String>>,
    port: Cage<Option<u16>>,
    path: Cage<String>,
    params: Cage<BTreeMap<String, String>>,
    queries: Cage<MultiMap<String, String>>,
    fragment: Cage<Option<String>>,
}

impl Locator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn raw_url(&self) -> ReadCage<String> {
        ReadCage::new(self.raw_url.clone())
    }

    pub fn path(&self) -> ReadCage<String> {
        ReadCage::new(self.path.clone())
    }

    pub fn params(&self) -> ReadCage<BTreeMap<String, String>> {
        ReadCage::new(self.params.clone())
    }

    pub fn queries(&self) -> ReadCage<MultiMap<String, String>> {
        ReadCage::new(self.queries.clone())
    }

    pub fn receive(&self, raw_url: impl Into<String>, raw_params: BTreeMap<String, String>) -> Result<(), url::ParseError> {
        let raw_url = raw_url.into();
        if raw_url == *self.raw_url.borrow() {
            return Ok(());
        }
        let new_url = Url::parse(&raw_url)?;
        let me = self;

        let update = || {
            if *me.scheme.borrow() != new_url.scheme() {
                me.scheme.revise(|mut scheme| *scheme = new_url.scheme().to_string());
            }
            if *me.authority.borrow() != new_url.authority() {
                me.authority.revise(|mut authority| *authority = new_url.authority().to_string());
            }
            if (*me.host.borrow()).as_deref() != new_url.host_str() {
                me.host.revise(|mut host| *host = new_url.host_str().map(|v| v.to_owned()));
            }
            if *me.port.borrow() != new_url.port() {
                me.port.revise(|mut port| *port = new_url.port().map(|v| v.to_owned()));
            }
            if *me.path.borrow() != new_url.path() {
                me.path.revise(|mut path| *path = new_url.path().to_owned());
            }
            if (*me.fragment.borrow()).as_deref() != new_url.fragment() {
                me.fragment.revise(|mut fragment| *fragment = new_url.fragment().map(|v| v.to_owned()));
            }
            if *me.params.borrow() != raw_params {
                me.params.revise(|mut params| *params = raw_params);
            }
            let new_queries: MultiMap<String, String> = url::form_urlencoded::parse(new_url.query().unwrap_or_default().as_bytes())
                .into_owned()
                .collect();
            if new_queries != *me.queries().borrow() {
                me.queries.revise(|mut queries| {
                    *queries = new_queries;
                });
            }
            me.raw_url.revise(|mut raw_url| *raw_url = new_url.to_string());
        };
        cfg_if! {
            if #[cfg(feature = "__single_holder")] {
                reflow::batch(update);
                glory_core::info!("=======rawxxx");
            } else {
                use glory_core::reflow::Revisable;
                if let Some(holder_id) = self.path.holder_id() {
                    reflow::batch(holder_id, update);
                } else {
                    update();
                }
            }
        }
        Ok(())
    }
}
