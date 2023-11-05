//! Routing and filters
//! Router can router http requests to different handlers.

#[macro_use]
mod cfg;
#[macro_use]
extern crate cfg_if;

mod aviator;
pub mod aviators;
pub mod filters;
pub use aviator::Aviator;
mod graff;
mod locator;
mod router;
pub use filters::*;
pub use graff::Graff;
pub use locator::Locator;
pub use router::Router;

#[cfg(not(target_arch = "wasm32"))]
pub use regex;
#[cfg(target_arch = "wasm32")]
pub mod regex;

pub mod url;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::{borrow::Cow, ops::Deref};

use educe::Educe;
use glory_core::reflow::Lotus;
use glory_core::{Cage, Scope, Truck, ViewId, Widget};
use indexmap::IndexMap;

/// Handler
pub trait Handler: 'static {
    #[doc(hidden)]
    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Self>()
    }
    #[doc(hidden)]
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    fn handle(&self, truck: Rc<RefCell<Truck>>);
}

#[doc(hidden)]
pub struct DetectMatched {
    pub hoops: Vec<Rc<dyn Handler>>,
    pub goal: Rc<dyn Handler>,
}

#[doc(hidden)]
pub type PathParams = BTreeMap<String, String>;

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathState {
    pub(crate) parts: Vec<String>,
    /// (row, col), row is the index of parts, col is the index of char in the part.
    pub(crate) cursor: (usize, usize),
    pub(crate) params: PathParams,
    pub(crate) end_slash: bool, // For rest match, we want include the last slash.
}
impl PathState {
    /// Create new `PathState`.
    #[inline]
    pub fn new(url_path: impl AsRef<str>) -> Self {
        let url_path = url_path.as_ref();
        let end_slash = url_path.ends_with('/');
        let parts = url_path
            .trim_start_matches('/')
            .trim_end_matches('/')
            .split('/')
            .filter_map(|p| if !p.is_empty() { Some(decode_url_path_safely(p)) } else { None })
            .collect::<Vec<_>>();
        PathState {
            parts,
            cursor: (0, 0),
            params: PathParams::new(),
            end_slash,
        }
    }

    #[inline]
    pub fn pick(&self) -> Option<&str> {
        match self.parts.get(self.cursor.0) {
            None => None,
            Some(part) => {
                if self.cursor.1 >= part.len() {
                    let row = self.cursor.0 + 1;
                    self.parts.get(row).map(|s| &**s)
                } else {
                    Some(&part[self.cursor.1..])
                }
            }
        }
    }

    #[inline]
    pub fn all_rest(&self) -> Option<Cow<'_, str>> {
        if let Some(picked) = self.pick() {
            if self.cursor.0 >= self.parts.len() - 1 {
                if self.end_slash {
                    Some(Cow::Owned(format!("{picked}/")))
                } else {
                    Some(Cow::Borrowed(picked))
                }
            } else {
                let last = self.parts[self.cursor.0 + 1..].join("/");
                if self.end_slash {
                    Some(Cow::Owned(format!("{picked}/{last}/")))
                } else {
                    Some(Cow::Owned(format!("{picked}/{last}")))
                }
            }
        } else {
            None
        }
    }

    #[inline]
    pub fn forward(&mut self, steps: usize) {
        let mut steps = steps + self.cursor.1;
        while let Some(part) = self.parts.get(self.cursor.0) {
            if part.len() > steps {
                self.cursor.1 = steps;
                return;
            } else {
                steps -= part.len();
                self.cursor = (self.cursor.0 + 1, 0);
            }
        }
    }

    #[inline]
    pub fn is_ended(&self) -> bool {
        self.cursor.0 >= self.parts.len()
    }
}

#[inline]
fn decode_url_path_safely(path: &str) -> String {
    glory_core::web::unescape(path)
}

#[derive(Educe)]
#[educe(Debug)]
pub struct Stuff(#[educe(Debug(ignore))] Box<dyn FnOnce(&mut Scope) -> ViewId>);

pub trait TruckExt {
    fn insert_stuff(&self, graff: impl Into<String>, widget: impl Widget + 'static);
    fn remove_stuff(&self, graff: &str) -> Option<Stuff>;
    fn stuffs(&self) -> Cage<IndexMap<String, Stuff>>;
    fn stuff_keys(&self) -> Rc<RefCell<IndexMap<String, String>>>;
    fn insert_stuff_key(&self, graff: impl Into<String>, key: impl Into<String>);
    fn remove_stuff_key(&self, graff: &str) -> Option<String>;
    fn contains_stuff_key(&self, graff: &str, key: &str) -> bool;
}

impl TruckExt for Rc<RefCell<Truck>> {
    fn insert_stuff(&self, graff_and_key: impl Into<String>, widget: impl Widget) {
        let graff_and_key = graff_and_key.into();
        let graff = if let Some((graff, key)) = graff_and_key.split_once('@') {
            if self.contains_stuff_key(&graff, &key) {
                return;
            }
            self.insert_stuff_key(graff, key);
            graff.to_owned()
        } else {
            self.remove_stuff_key(&graff_and_key);
            graff_and_key
        };
        self.stuffs().revise(|mut stuffs| {
            let stuff = move |ctx: &mut Scope| -> ViewId { widget.store_in(ctx) };
            stuffs.insert(graff, Stuff(Box::new(stuff)));
        });
    }
    fn remove_stuff(&self, graff: &str) -> Option<Stuff> {
        let mut stuff = None;
        if self.stuffs().get().contains_key(graff) {
            stuff = self.stuffs().revise(|mut stuffs| stuffs.remove(graff));
        }
        stuff
    }
    fn insert_stuff_key(&self, graff: impl Into<String>, key: impl Into<String>) {
        let graff = graff.into();
        let key = key.into();
        self.stuff_keys().borrow_mut().insert(graff, key);
    }
    fn remove_stuff_key(&self, graff: &str) -> Option<String> {
        self.stuff_keys().borrow_mut().remove(graff)
    }
    fn contains_stuff_key(&self, graff: &str, key: &str) -> bool {
        self.stuff_keys().borrow().get(graff).map(|s| &**s) == Some(key)
    }
    fn stuffs(&self) -> Cage<IndexMap<String, Stuff>> {
        const KEY: &str = "glory::routing::stuffs";
        let exists = (*self).deref().borrow().contains_key(KEY);
        if !exists {
            let stuffs: Cage<IndexMap<String, Stuff>> = Default::default();
            (*self).deref().borrow_mut().insert(KEY.to_owned(), stuffs);
        }
        self.deref().borrow().get::<Cage<IndexMap<String, Stuff>>>(KEY).unwrap().clone()
    }
    fn stuff_keys(&self) -> Rc<RefCell<IndexMap<String, String>>> {
        const KEY: &str = "glory::routing::stuff_keys";
        let exists = (*self).deref().borrow().contains_key(KEY);
        if !exists {
            let stuff_keys: Rc<RefCell<IndexMap<String, String>>> = Default::default();
            (*self).deref().borrow_mut().insert(KEY.to_owned(), stuff_keys);
        }
        self.deref().borrow().get::<Rc<RefCell<IndexMap<String, String>>>>(KEY).unwrap().clone()
    }
}

#[doc(hidden)]
#[non_exhaustive]
pub struct WhenHoop<H, F> {
    pub inner: H,
    pub filter: F,
}
impl<H, F> Handler for WhenHoop<H, F>
where
    H: Handler,
    F: Fn(&Truck) -> bool + 'static,
{
    fn handle(&self, truck: Rc<RefCell<Truck>>) {
        if (self.filter)(&truck.borrow()) {
            self.inner.handle(truck);
        }
    }
}

impl<F> Handler for F
where
    F: Fn(Rc<RefCell<Truck>>) + 'static,
{
    fn handle(&self, truck: Rc<RefCell<Truck>>) {
        self(truck);
    }
}
