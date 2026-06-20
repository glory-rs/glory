use std::cell::RefCell;
use std::rc::Rc;

use educe::Educe;
use glory_core::holder::Enabler;
use glory_core::web::unescape;
use glory_core::Truck;
use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};

use crate::aviators::ScrollMemory;
use crate::locator::LocatorModifier;
use crate::url::Url;
use crate::{Aviator, Handler, Locator, NavigationError, PathState, Router};

#[derive(Educe, Clone)]
#[educe(Debug)]
pub struct BrowserAviator {
    pub truck: Rc<RefCell<Truck>>,
    pub router: Rc<Router>,
    #[educe(Debug(ignore))]
    pub catcher: Rc<dyn Handler>,
    base_path: String,
    curr_path: RefCell<String>,
    /// Full URL (path + query + fragment) of the entry currently on screen.
    /// Used as the scroll-memory key for the *outgoing* entry when a
    /// navigation switches away from it. `Rc`-shared so the value survives
    /// across the per-event aviator clones obtained from the truck.
    curr_url: Rc<RefCell<String>>,
    /// Per-history-entry scroll positions (see [`ScrollMemory`]). Shared
    /// across `Clone`s of the aviator so the `popstate` listener — which
    /// obtains its own clone from the truck — sees the same recorded
    /// positions as the navigation that left them.
    scroll_memory: Rc<RefCell<ScrollMemory>>,
}

impl BrowserAviator {
    pub fn new(router: impl Into<Rc<Router>>, catcher: impl Handler) -> Self {
        Self {
            truck: Default::default(),
            router: router.into(),
            catcher: Rc::new(catcher),
            base_path: Default::default(),
            curr_path: Default::default(),
            curr_url: Default::default(),
            scroll_memory: Default::default(),
        }
    }

    /// Record the live `window` scroll offset for the entry currently on
    /// screen, so it can be restored when the user navigates back/forward to
    /// it later. No-op before the first navigation (no outgoing entry yet).
    fn record_current_scroll(&self) {
        let curr_url = self.curr_url.borrow().clone();
        if curr_url.is_empty() {
            return;
        }
        let window = glory_core::web::window();
        let x = window.scroll_x().unwrap_or(0.0);
        let y = window.scroll_y().unwrap_or(0.0);
        self.scroll_memory.borrow_mut().record(curr_url, x, y);
    }

    /// Restore the remembered scroll offset for `new_url` if one exists,
    /// otherwise fall back to top-of-page / hash-target scrolling. Also
    /// updates the "current entry" key used for the next outgoing record.
    /// When `do_scroll` is `false` (e.g. an anchor with `noscroll`) the
    /// viewport is left untouched but the current-entry key is still updated.
    fn restore_scroll_for(&self, new_url: &str, do_scroll: bool) {
        if do_scroll {
            if let Some((x, y)) = self.scroll_memory.borrow_mut().restore(new_url) {
                glory_core::web::window().scroll_to_with_x_and_y(x, y);
            } else {
                scroll_after_navigation();
            }
        }
        *self.curr_url.borrow_mut() = new_url.to_owned();
    }

    pub(crate) fn locate(&self, raw_url: impl Into<String>) -> Result<(), NavigationError> {
        let raw_url = raw_url.into();
        let locator = {
            let truck = self.truck.borrow();
            truck.obtain::<Locator>().unwrap_throw().clone()
        };
        if &raw_url == &*locator.raw_url().get() {
            return Ok(());
        }
        let url = Url::parse(&raw_url)?;
        let new_path = url.path();
        if new_path == *self.curr_path.borrow() {
            locator.receive(raw_url.clone(), None)?;
            return Ok(());
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
    pub fn goto(&self, modifier: impl Into<LocatorModifier>) -> Result<(), NavigationError> {
        self.goto_with_state(modifier, JsValue::NULL)
    }

    fn goto_with_state(&self, modifier: impl Into<LocatorModifier>, state: JsValue) -> Result<(), NavigationError> {
        let modifier = modifier.into();
        // Remember where we are scrolled before leaving this entry, so a
        // later back/forward can restore it. `replace` overwrites the current
        // entry rather than creating a new one, so its scroll is not worth
        // remembering as a distinct entry.
        if !modifier.replace {
            self.record_current_scroll();
        }
        let history = glory_core::web::window().history().unwrap_throw();
        if modifier.replace {
            history
                .replace_state_with_url(&state, "", Some(&modifier.raw_url))
                .unwrap_throw();
        } else {
            history.push_state_with_url(&state, "", Some(&modifier.raw_url)).unwrap_throw();
        }
        let href = glory_core::web::location()
            .href()
            .map_err(|_| NavigationError::BrowserLocationUnavailable)?;
        self.locate(href.clone())?;
        // Pushing a fresh entry has no remembered scroll, so this normally
        // scrolls to top/hash; restoring only kicks in for back/forward.
        self.restore_scroll_for(&href, modifier.scroll);
        Ok(())
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

            // Let the browser handle links with a target, or inert anchors.
            // `state` may be supplied either as an attribute or a JS property.
            if !target.is_empty() || (href.is_empty() && !has_marker(&a, "state")) {
                return;
            }
            if href.is_empty() {
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
            let replace = has_marker(&a, "replace");
            let modifier = LocatorModifier {
                raw_url: to,
                replace,
                scroll: !a.has_attribute("noscroll"),
            };
            self.goto_with_state(modifier, get_anchor_state(&a)).unwrap_throw();
        }
    }
}

impl Aviator for BrowserAviator {
    fn goto(&self, url: &str) -> Result<(), NavigationError> {
        BrowserAviator::goto(self, url)
    }

    fn back(&self) -> Result<bool, NavigationError> {
        let history = glory_core::web::window()
            .history()
            .map_err(|_| NavigationError::BrowserHistoryUnavailable)?;
        history.back().map_err(|_| NavigationError::BrowserHistoryUnavailable)?;
        Ok(true)
    }

    fn forward(&self) -> Result<bool, NavigationError> {
        let history = glory_core::web::window()
            .history()
            .map_err(|_| NavigationError::BrowserHistoryUnavailable)?;
        history.forward().map_err(|_| NavigationError::BrowserHistoryUnavailable)?;
        Ok(true)
    }
}

impl Enabler for BrowserAviator {
    fn enable(mut self, truck: Rc<RefCell<Truck>>) {
        self.truck = truck.clone();
        truck.borrow_mut().inject(Locator::new());
        let initial_href = glory_core::web::location().href().unwrap_throw();
        self.locate(initial_href.clone()).unwrap_throw();
        // Seed the current-entry key so the first navigation records the
        // initial entry's scroll position (initial load itself is not
        // scrolled — the browser already restored it).
        *self.curr_url.borrow_mut() = initial_href;
        truck.borrow_mut().inject(self);

        glory_core::web::window_event_listener_untyped("popstate", {
            let truck = truck.clone();
            move |_| {
                let this = truck.borrow_mut().obtain::<Self>().unwrap_throw().clone();
                // back/forward: remember the entry we are leaving, switch to
                // the target, then restore its remembered scroll (or top).
                this.record_current_scroll();
                let href = glory_core::web::location().href().unwrap_throw();
                this.locate(href.clone()).unwrap_throw();
                this.restore_scroll_for(&href, true);
            }
        });
        glory_core::web::window_event_listener_untyped("click", move |event| {
            let this = truck.borrow_mut().obtain::<Self>().unwrap_throw().clone();
            this.handle_anchor_click(event);
        });
    }
}

fn scroll_after_navigation() {
    let location = glory_core::web::location();
    if let Ok(hash) = location.hash() {
        if let Some(id) = hash.strip_prefix('#') {
            if !id.is_empty() {
                if let Some(element) = glory_core::web::document().get_element_by_id(id) {
                    element.scroll_into_view();
                    return;
                }
            }
        }
    }
    glory_core::web::window().scroll_to_with_x_and_y(0.0, 0.0);
}

fn has_marker(anchor: &web_sys::HtmlAnchorElement, name: &str) -> bool {
    if anchor.has_attribute(name) {
        return true;
    }
    glory_core::web::helpers::get_property(anchor.unchecked_ref(), name)
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn get_anchor_state(anchor: &web_sys::HtmlAnchorElement) -> JsValue {
    glory_core::web::helpers::get_property(anchor.unchecked_ref(), "state").unwrap_or(JsValue::NULL)
}
