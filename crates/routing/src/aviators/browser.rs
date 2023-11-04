use std::cell::RefCell;
use std::rc::Rc;

use educe::Educe;
use glory_core::holder::Enabler;
use glory_core::web::unescape;
use glory_core::Truck;
use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};

use crate::locator::LocatorModifier;
use crate::url::Url;
use crate::{Aviator, Handler, Locator, PathState, Router};

#[derive(Educe, Clone)]
#[educe(Debug)]
pub struct BrowserAviator {
    pub truck: Rc<RefCell<Truck>>,
    pub router: Rc<Router>,
    #[educe(Debug(ignore))]
    pub catcher: Rc<dyn Handler>,
    base_path: String,
    curr_path: RefCell<String>,
}

impl BrowserAviator {
    pub fn new(router: impl Into<Rc<Router>>, catcher: impl Handler) -> Self {
        Self {
            truck: Default::default(),
            router: router.into(),
            catcher: Rc::new(catcher),
            base_path: Default::default(),
            curr_path: Default::default(),
        }
    }
    pub(crate) fn locate(&self, raw_url: impl Into<String>) -> Result<(), crate::url::ParseError> {
        let raw_url = raw_url.into();
        let locator = {
            let truck = self.truck.borrow();
            truck.obtain::<Locator>().unwrap_throw().clone()
        };
        if &raw_url == &*locator.raw_url().borrow() {
            return Ok(());
        }
        let url = Url::parse(&raw_url)?;
        let new_path = url.path();
        if new_path == *self.curr_path.borrow() {
            locator.receive(raw_url.clone(), None)?;
            return Ok(())
        }

        glory_core::info!("[locate]: {}  new_path: {}  old_path: {}", raw_url, new_path, *self.curr_path.borrow());
        *self.curr_path.borrow_mut() = new_path.clone();

        let mut detect_state = PathState::new(new_path);
        let matched = self.router.detect(&url, &self.truck.borrow(), &mut detect_state);
        locator.receive(raw_url.clone(), Some(detect_state.params))?;
        if let Some(dm) = matched {
            for hoop in [&dm.hoops[..], &[dm.goal]].concat() {
                hoop.handle(self.truck.clone());
            }
        } else {
            glory_core::info!("No matched route found for {:?}", raw_url);
            self.catcher.handle(self.truck.clone());
        }
        Ok(())
    }
    pub fn goto(&self, modifier: impl Into<LocatorModifier>) {
        let modifier = modifier.into();
        let history = glory_core::web::window().history().unwrap_throw();
        history.push_state_with_url(&JsValue::NULL, "", Some(&modifier.raw_url)).unwrap_throw();
    }
    pub(crate) fn handle_anchor_click(&self, event: web_sys::Event) {
        let event = event.unchecked_into::<web_sys::MouseEvent>();
        if event.default_prevented() || event.button() != 0 || event.meta_key() || event.alt_key() || event.ctrl_key() || event.shift_key() {
            return;
        }

        let composed_path = event.composed_path();
        let mut a: Option<web_sys::HtmlAnchorElement> = None;
        for i in 0..composed_path.length() {
            if let Ok(el) = composed_path.get(i).dyn_into::<web_sys::HtmlAnchorElement>() {
                a = Some(el);
            }
        }
        if let Some(a) = a {
            let href = a.href();
            let target = a.target();

            // let browser handle this event if link has target,
            // or if it doesn't have href or state
            // TODO "state" is set as a prop, not an attribute
            if !target.is_empty() || (href.is_empty() && !a.has_attribute("state")) {
                return;
            }

            let rel = a.get_attribute("rel").unwrap_or_default();
            let mut rel = rel.split([' ', '\t']);

            // let browser handle event if it has rel=external or download
            if a.has_attribute("download") || rel.any(|p| p == "external") {
                return;
            }
            if glory_core::web::location().href().as_deref() == Ok(href.as_str()) {
                event.prevent_default();
                return;
            }

            let url = Url::try_from(href.as_str()).unwrap();
            let path_name = unescape(&url.path());

            // let browser handle this event if it leaves our domain
            // or our base path
            if url.origin() != glory_core::web::location().origin().unwrap_or_default()
                || (!self.base_path.is_empty() && !path_name.is_empty() && !path_name.to_lowercase().starts_with(&self.base_path.to_lowercase()))
            {
                return;
            }

            let query = if url.query().unwrap_or_default().is_empty() {
                "".to_owned()
            } else {
                format!("?{}", unescape(&url.query().unwrap_or_default()))
            };
            let fragment = if url.fragment().unwrap_or_default().is_empty() {
                "".to_owned()
            } else {
                format!("#{}", unescape(&url.fragment().unwrap_or_default()))
            };

            event.prevent_default();

            let to = format!("{path_name}{query}{fragment}");
            let replace = glory_core::web::helpers::get_property(a.unchecked_ref(), "replace")
                .ok()
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let modifier = LocatorModifier {
                raw_url: to,
                replace,
                scroll: !a.has_attribute("noscroll"),
            };
            self.goto(modifier);
            self.locate(glory_core::web::location().href().expect("Location not found"))
                .unwrap_throw();
        }
    }
}

impl Aviator for BrowserAviator {
    fn goto(&self, url: &str) {
        BrowserAviator::goto(self, url);
    }
}

impl Enabler for BrowserAviator {
    fn enable(mut self, truck: Rc<RefCell<Truck>>) {
        self.truck = truck.clone();
        truck.borrow_mut().inject(Locator::new());
        self.locate(glory_core::web::location().href().unwrap_throw()).unwrap_throw();
        truck.borrow_mut().inject(self);

        glory_core::web::window_event_listener_untyped("popstate", {
            let truck = truck.clone();
            move |_| {
                let this = truck.borrow_mut().obtain::<Self>().unwrap_throw().clone();
                this.locate(glory_core::web::location().href().unwrap_throw()).unwrap_throw();
            }
        });
        glory_core::web::window_event_listener_untyped("click", move |event| {
            let this = truck.borrow_mut().obtain::<Self>().unwrap_throw().clone();
            this.handle_anchor_click(event);
        });
    }
}
