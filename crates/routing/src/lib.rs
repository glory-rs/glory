//! Routing and filters.
//!
//! A [`Router`] matches a URL against registered [`Graff`]s (route
//! definitions) and dispatches to the matched [`Handler`]. The
//! [`Aviator`] trait is glory-routing's history / navigation
//! abstraction: each platform backend implements `goto(url)` (and
//! optionally listens for `popstate`-style events). The platform
//! variants live under [`aviators`]:
//!
//! - [`aviators::BrowserAviator`] — wraps `window.history` and
//!   listens for `popstate` + delegated anchor clicks.
//! - [`aviators::ServerAviator`] — used during SSR; resolves request
//!   URLs through the same fallible navigation trait without touching
//!   browser history.
//!
//! To navigate from app code, obtain the active `Aviator` from the
//! `Truck` and call `goto`. The `<a>` element's default click is
//! intercepted by `BrowserAviator` so plain links also work.
//!
//! ## Adding a new history backend
//!
//! Implement [`Aviator`] (and optionally [`glory_core::holder::Enabler`]
//! if your backend needs lifecycle wiring), then inject your
//! implementation into the app's `Truck` from inside a holder. The
//! routing core only depends on the trait, not on `web_sys`.

#[macro_use]
mod cfg;
#[macro_use]
extern crate cfg_if;

mod aviator;
pub mod aviators;
pub mod filters;
pub use aviator::{Aviator, NavigationError};
mod graff;
mod locator;
mod router;
mod typed;
pub use filters::*;
pub use graff::Graff;
pub use locator::{Locator, LocatorModifier};
pub use router::Router;
pub use typed::{
    AviatorExt, FromRouteParam, Routable, RouteParamError, decode_route_param, encode_route_param, parse_route_param, required_route_param,
};

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
    pub(crate) segments: Vec<String>,
    pub(crate) cursor: PathCursor,
    pub(crate) params: PathParams,
    pub(crate) has_trailing_slash: bool,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PathCursor {
    pub segment: usize,
    pub offset: usize,
}
impl PathState {
    /// Create new `PathState`.
    #[inline]
    pub fn new(url_path: impl AsRef<str>) -> Self {
        let url_path = url_path.as_ref();
        let has_trailing_slash = url_path.ends_with('/');
        let segments = url_path
            .trim_start_matches('/')
            .trim_end_matches('/')
            .split('/')
            .filter_map(|p| if !p.is_empty() { Some(decode_url_path_safely(p)) } else { None })
            .collect::<Vec<_>>();
        PathState {
            segments,
            cursor: PathCursor::default(),
            params: PathParams::new(),
            has_trailing_slash,
        }
    }

    #[inline]
    pub fn pick(&self) -> Option<&str> {
        match self.segments.get(self.cursor.segment) {
            None => None,
            Some(part) => {
                if self.cursor.offset >= part.len() {
                    let segment = self.cursor.segment + 1;
                    self.segments.get(segment).map(|s| &**s)
                } else {
                    Some(&part[self.cursor.offset..])
                }
            }
        }
    }

    #[inline]
    pub fn all_rest(&self) -> Option<Cow<'_, str>> {
        if let Some(picked) = self.pick() {
            if self.cursor.segment >= self.segments.len() - 1 {
                if self.has_trailing_slash {
                    Some(Cow::Owned(format!("{picked}/")))
                } else {
                    Some(Cow::Borrowed(picked))
                }
            } else {
                let last = self.segments[self.cursor.segment + 1..].join("/");
                if self.has_trailing_slash {
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
        let mut steps = steps + self.cursor.offset;
        while let Some(part) = self.segments.get(self.cursor.segment) {
            if part.len() > steps {
                self.cursor.offset = steps;
                return;
            } else {
                steps -= part.len();
                self.cursor = PathCursor {
                    segment: self.cursor.segment + 1,
                    offset: 0,
                };
            }
        }
    }

    #[inline]
    pub fn is_ended(&self) -> bool {
        self.cursor.segment >= self.segments.len()
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
            if self.contains_stuff_key(graff, key) {
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
            // `shift_remove` preserves iteration order; consistent with the
            // glory-core migration. The deprecated `remove` aliases
            // `swap_remove` in current indexmap and would corrupt order.
            stuff = self.stuffs().revise(|mut stuffs| stuffs.shift_remove(graff));
        }
        stuff
    }
    fn insert_stuff_key(&self, graff: impl Into<String>, key: impl Into<String>) {
        let graff = graff.into();
        let key = key.into();
        self.stuff_keys().borrow_mut().insert(graff, key);
    }
    fn remove_stuff_key(&self, graff: &str) -> Option<String> {
        self.stuff_keys().borrow_mut().shift_remove(graff)
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
        *self.deref().borrow().get::<Cage<IndexMap<String, Stuff>>>(KEY).unwrap()
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
