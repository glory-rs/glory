//! Browser History and scoped raw-anchor navigation. No matching or rendering
//! lives here: all decisions use glory-routing and the existing Widget host.
use glory_routing::{
    Entry, EntryKey, Error, History, Location, Result, RouterHandle, ScrollMemory, Traversal,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Element, Event, EventTarget, Window};

const STATE: &str = "__glory_routing_v1";
const ATTACHED: &str = "__glory_routing_attached_v1";
fn failure(value: JsValue) -> Error {
    Error::History(format!("{value:?}"))
}
fn url_location(window: &Window, href: &str) -> Result<Location> {
    let url = web_sys::Url::new_with_base(href, &window.location().href().map_err(failure)?)
        .map_err(failure)?;
    if url.origin() != window.location().origin().map_err(failure)? {
        return Err(Error::Unsupported("external browser target"));
    }
    let fragment = url
        .href()
        .split_once('#')
        .map(|(_, value)| format!("#{value}"))
        .unwrap_or_default();
    Location::parse(&format!("{}{}{fragment}", url.pathname(), url.search()))
}

pub struct BrowserHistory {
    window: Window,
    session: String,
    sequence: Cell<u64>,
    scroll: RefCell<ScrollMemory>,
    displayed: RefCell<Option<Entry>>,
}
impl BrowserHistory {
    pub fn new(window: Window) -> Result<Self> {
        let mut random = [0u8; 16];
        window
            .crypto()
            .map_err(failure)?
            .get_random_values_with_u8_array(&mut random)
            .map_err(failure)?;
        let session = random
            .iter()
            .map(|value| format!("{value:02x}"))
            .collect::<String>();
        let provider = Self {
            window,
            session,
            sequence: Cell::new(0),
            scroll: RefCell::new(ScrollMemory::default()),
            displayed: RefCell::new(None),
        };
        let current = provider.current()?;
        if current.key.is_none() {
            provider.replace(&current.location)?;
        }
        *provider.displayed.borrow_mut() = Some(provider.current()?);
        Ok(provider)
    }
    pub fn location(&self) -> Result<Location> {
        url_location(
            &self.window,
            &self.window.location().href().map_err(failure)?,
        )
    }
    fn key(&self) -> Result<EntryKey> {
        let sequence = self
            .sequence
            .get()
            .checked_add(1)
            .ok_or(Error::OverLimit("browser history identity"))?;
        self.sequence.set(sequence);
        Ok(EntryKey(format!("{}:{sequence}", self.session)))
    }
    fn capture(&self) -> Result {
        if let Some(entry) = self.displayed.borrow().as_ref()
            && let Some(key) = &entry.key
        {
            self.scroll.borrow_mut().record(
                key.clone(),
                "window",
                self.window.scroll_x().map_err(failure)?,
                self.window.scroll_y().map_err(failure)?,
            )?;
        }
        Ok(())
    }
    fn write(&self, location: &Location, push: bool) -> Result<Entry> {
        self.capture()?;
        let history = self.window.history().map_err(failure)?;
        let old = history.state().map_err(failure)?;
        if !old.is_null()
            && !old.is_undefined()
            && (!old.is_object() || js_sys::Array::is_array(&old))
        {
            return Err(Error::Unsupported(
                "non-object history.state; install an application history adapter",
            ));
        }
        let state = js_sys::Object::new();
        if old.is_object() {
            let prototype = js_sys::Object::get_prototype_of(&old);
            if !prototype.is_null() && prototype != js_sys::Object::get_prototype_of(&state) {
                return Err(Error::Unsupported("non-plain history.state object"));
            }
            js_sys::Object::assign(&state, &js_sys::Object::from(old));
        }
        let key = if push {
            self.key()?
        } else {
            match self.current()?.key {
                Some(key) => key,
                None => self.key()?,
            }
        };
        js_sys::Reflect::set(&state, &STATE.into(), &key.0.clone().into()).map_err(failure)?;
        if push {
            history.push_state_with_url(&state, "", Some(&location.to_string()))
        } else {
            history.replace_state_with_url(&state, "", Some(&location.to_string()))
        }
        .map_err(failure)?;
        Ok(Entry {
            location: location.clone(),
            key: Some(key),
        })
    }
    /// Attach once per Window. Keep the returned lease under the app's Owner;
    /// dropping it removes listeners, cancels RAF and restores scroll policy.
    pub fn attach(
        self: &Rc<Self>,
        owner: &glory_core::Scope,
        root: Element,
        router: RouterHandle,
        on_error: impl Fn(Error) + 'static,
    ) -> Result<BrowserLease> {
        if !owner.is_alive() {
            return Err(Error::Gone);
        }
        if js_sys::Reflect::get(&self.window, &ATTACHED.into())
            .map_err(failure)?
            .as_bool()
            == Some(true)
        {
            return Err(Error::History("browser routing already attached".into()));
        }
        if router.runtime()? != owner.runtime() {
            return Err(Error::WrongRuntime);
        }
        let history = self.window.history().map_err(failure)?;
        let previous = history.scroll_restoration().map_err(failure)?;
        history
            .set_scroll_restoration(web_sys::ScrollRestoration::Manual)
            .map_err(failure)?;
        let initial = router.current()?;
        let base = router.routes()?.base().to_owned();
        let driver = Rc::new(Driver {
            provider: self.clone(),
            router,
            root,
            base,
            queue: RefCell::new(VecDeque::new()),
            pending_error: RefCell::new(None),
            process: RefCell::new(None),
            restore: RefCell::new(None),
            pending: Cell::new(None),
            restoring: Cell::new(None),
            stopped: Cell::new(false),
            revision: Cell::new(initial.revision),
            intent: Cell::new(0),
            restore_intent: Cell::new(0),
            attempts: Cell::new(0),
            traversing: Cell::new(false),
            previous,
            errors: Rc::new(on_error),
        });
        let weak = Rc::downgrade(&driver);
        *driver.process.borrow_mut() = Some(Closure::new(move |_| {
            if let Some(driver) = weak.upgrade() {
                driver.pending.set(None);
                driver.process_inputs();
            }
        }));
        let weak = Rc::downgrade(&driver);
        *driver.restore.borrow_mut() = Some(Closure::new(move |_| {
            if let Some(driver) = weak.upgrade() {
                driver.restoring.set(None);
                driver.restore_position();
            }
        }));
        let mut lease = BrowserLease {
            driver: driver.clone(),
            listeners: Vec::new(),
        };
        let window: EventTarget = self.window.clone().into();
        for name in ["popstate", "hashchange"] {
            let weak = Rc::downgrade(&driver);
            lease.listen(window.clone(), name, move |_| {
                if let Some(driver) = weak.upgrade() {
                    driver.enqueue(Input::Observed);
                }
            })?;
        }
        for name in ["scroll", "pagehide"] {
            let weak = Rc::downgrade(&driver);
            lease.listen(window.clone(), name, move |_| {
                if let Some(driver) = weak.upgrade()
                    && let Err(error) = driver.provider.capture()
                {
                    driver.pending_error.borrow_mut().get_or_insert(error);
                    driver.enqueue(Input::Wake);
                }
            })?;
        }
        for name in ["wheel", "touchstart", "pointerdown", "keydown"] {
            let weak = Rc::downgrade(&driver);
            lease.listen(window.clone(), name, move |_| {
                if let Some(driver) = weak.upgrade() {
                    driver.intent.set(driver.intent.get().wrapping_add(1));
                }
            })?;
        }
        let weak = Rc::downgrade(&driver);
        lease.listen(driver.root.clone().into(), "click", move |event| {
            if let Some(driver) = weak.upgrade() {
                driver.click(event);
            }
        })?;
        let weak = Rc::downgrade(&driver);
        owner.effect(move || {
            let Some(driver) = weak.upgrade() else {
                return;
            };
            let Ok(snapshot) = driver.router.snapshot() else {
                return;
            };
            if driver.revision.replace(snapshot.revision) != snapshot.revision {
                driver.traversing.set(matches!(
                    snapshot.kind,
                    glory_routing::NavigationKind::Observed
                        | glory_routing::NavigationKind::Traverse(_)
                ));
                let previous = driver
                    .provider
                    .displayed
                    .replace(Some(snapshot.entry.clone()));
                let same_path = previous
                    .is_some_and(|previous| previous.location.path() == snapshot.location().path());
                if !same_path || snapshot.location().fragment().is_some() || driver.traversing.get()
                {
                    driver.restore_intent.set(driver.intent.get());
                    driver.attempts.set(0);
                    driver.schedule_restore();
                }
            }
        });
        js_sys::Reflect::set(&self.window, &ATTACHED.into(), &JsValue::TRUE).map_err(failure)?;
        Ok(lease)
    }
}
impl History for BrowserHistory {
    fn current(&self) -> Result<Entry> {
        let state = self
            .window
            .history()
            .map_err(failure)?
            .state()
            .map_err(failure)?;
        let key = js_sys::Reflect::get(&state, &STATE.into())
            .ok()
            .and_then(|value| value.as_string())
            .filter(|value| value.len() <= 128)
            .map(EntryKey);
        Ok(Entry {
            location: self.location()?,
            key,
        })
    }
    fn push(&self, location: &Location) -> Result<Entry> {
        self.write(location, true)
    }
    fn replace(&self, location: &Location) -> Result<Entry> {
        self.write(location, false)
    }
    fn preview(&self, _: i32) -> Result<Traversal> {
        Ok(Traversal::Host)
    }
    fn traverse(&self, delta: i32) -> Result<Option<Entry>> {
        self.capture()?;
        self.window
            .history()
            .map_err(failure)?
            .go_with_delta(delta)
            .map_err(failure)?;
        Ok(None)
    }
}
enum Input {
    Observed,
    Push(Location),
    Wake,
}
struct Driver {
    provider: Rc<BrowserHistory>,
    router: RouterHandle,
    root: Element,
    base: String,
    queue: RefCell<VecDeque<Input>>,
    pending_error: RefCell<Option<Error>>,
    process: RefCell<Option<Closure<dyn FnMut(f64)>>>,
    restore: RefCell<Option<Closure<dyn FnMut(f64)>>>,
    pending: Cell<Option<i32>>,
    restoring: Cell<Option<i32>>,
    stopped: Cell<bool>,
    revision: Cell<u64>,
    intent: Cell<u64>,
    restore_intent: Cell<u64>,
    attempts: Cell<u8>,
    traversing: Cell<bool>,
    previous: web_sys::ScrollRestoration,
    errors: Rc<dyn Fn(Error)>,
}
impl Driver {
    fn enqueue(&self, input: Input) {
        if self.stopped.get() {
            return;
        }
        if self.queue.borrow().len() >= 32 {
            self.pending_error.borrow_mut().get_or_insert(Error::Busy);
            return;
        }
        self.queue.borrow_mut().push_back(input);
        if self.pending.get().is_none()
            && let Some(callback) = self.process.borrow().as_ref()
        {
            match self
                .provider
                .window
                .request_animation_frame(callback.as_ref().unchecked_ref())
            {
                Ok(id) => self.pending.set(Some(id)),
                Err(error) => web_sys::console::error_1(&error),
            }
        }
    }
    fn process_inputs(&self) {
        let error = self.pending_error.borrow_mut().take();
        if let Some(error) = error {
            (self.errors)(error);
        }
        let inputs = std::mem::take(&mut *self.queue.borrow_mut());
        for input in inputs {
            if self.router.has_async_guards().unwrap_or(false) {
                let router = self.router.clone();
                let errors = self.errors.clone();
                let entry = self.provider.current();
                self.traversing.set(matches!(input, Input::Observed));
                wasm_bindgen_futures::spawn_local(async move {
                    let result = match input {
                        Input::Observed => match entry {
                            Ok(entry) => router.observe_async(entry).await,
                            Err(error) => Err(error),
                        },
                        Input::Push(location) => router.push_async(location).await,
                        Input::Wake => Ok(glory_routing::Navigation::Noop),
                    };
                    if let Err(error) = result
                        && error != Error::Superseded
                    {
                        errors(error);
                    }
                });
                continue;
            }
            let result = match input {
                Input::Observed => {
                    self.traversing.set(true);
                    self.provider
                        .current()
                        .and_then(|entry| self.router.observe(entry))
                }
                Input::Push(location) => {
                    self.traversing.set(false);
                    self.router.push(location)
                }
                Input::Wake => Ok(glory_routing::Navigation::Noop),
            };
            if let Err(error) = result {
                (self.errors)(error);
            }
        }
    }
    fn click(&self, event: Event) {
        let Some(mouse) = event.dyn_ref::<web_sys::MouseEvent>() else {
            return;
        };
        if event.default_prevented()
            || mouse.button() != 0
            || mouse.ctrl_key()
            || mouse.meta_key()
            || mouse.shift_key()
            || mouse.alt_key()
        {
            return;
        }
        let mut anchor = None;
        for value in event.composed_path().iter() {
            if let Ok(element) = value.dyn_into::<Element>() {
                if element == self.root {
                    break;
                }
                if element.has_attribute("data-gly-app") {
                    return;
                }
                if anchor.is_none() && element.local_name() == "a" {
                    anchor = Some(element);
                }
            }
        }
        let Some(anchor) = anchor else {
            return;
        };
        if !anchor.has_attribute("data-glory-route")
            || anchor.has_attribute("download")
            || anchor
                .get_attribute("target")
                .is_some_and(|target| !target.is_empty() && target != "_self")
            || anchor
                .get_attribute("rel")
                .is_some_and(|rel| rel.split_whitespace().any(|rel| rel == "external"))
        {
            return;
        }
        let href = js_sys::Reflect::get(&anchor, &"href".into())
            .ok()
            .and_then(|value| value.as_string());
        let Some(href) = href else {
            return;
        };
        let Ok(location) = url_location(&self.provider.window, &href) else {
            return;
        };
        // Only this Router's mount can claim an internal URL, including its fallback.
        if !self.router.is_alive()
            || !location
                .path()
                .strip_prefix(&self.base)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        {
            return;
        }
        if self.queue.borrow().len() >= 32 {
            return;
        }
        event.prevent_default();
        self.enqueue(Input::Push(location));
    }
    fn schedule_restore(&self) {
        if let Some(id) = self.restoring.take() {
            let _ = self.provider.window.cancel_animation_frame(id);
        }
        if let Some(callback) = self.restore.borrow().as_ref() {
            match self
                .provider
                .window
                .request_animation_frame(callback.as_ref().unchecked_ref())
            {
                Ok(id) => self.restoring.set(Some(id)),
                Err(error) => (self.errors)(failure(error)),
            }
        }
    }
    fn restore_position(&self) {
        if self.stopped.get() || self.intent.get() != self.restore_intent.get() {
            return;
        }
        let Ok(snapshot) = self.router.current() else {
            return;
        };
        let window = &self.provider.window;
        if self.traversing.get()
            && let Some(key) = &snapshot.entry.key
            && let Some((x, y)) = self.provider.scroll.borrow_mut().restore(key, "window")
        {
            window.scroll_to_with_x_and_y(x, y);
            return;
        }
        if let Some(fragment) = snapshot.location().fragment()
            && !fragment.is_empty()
        {
            if let Some(element) = window
                .document()
                .and_then(|doc| doc.get_element_by_id(fragment))
            {
                element.scroll_into_view();
                return;
            }
            let attempts = self.attempts.get() + 1;
            self.attempts.set(attempts);
            if attempts < 8 {
                self.schedule_restore();
                return;
            }
        }
        window.scroll_to_with_x_and_y(0.0, 0.0);
    }
}
type Listener = (EventTarget, &'static str, Closure<dyn FnMut(Event)>);
pub struct BrowserLease {
    driver: Rc<Driver>,
    listeners: Vec<Listener>,
}
impl BrowserLease {
    fn listen(
        &mut self,
        target: EventTarget,
        name: &'static str,
        callback: impl FnMut(Event) + 'static,
    ) -> Result {
        let callback = Closure::new(callback);
        target
            .add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())
            .map_err(failure)?;
        self.listeners.push((target, name, callback));
        Ok(())
    }
    /// For cooperating code that directly changed history without a popstate.
    pub fn notify_location(&self) {
        self.driver.enqueue(Input::Observed);
    }
}
impl Drop for BrowserLease {
    fn drop(&mut self) {
        self.driver.stopped.set(true);
        let _ = self.driver.router.cancel_pending();
        for (target, name, callback) in &self.listeners {
            let _ =
                target.remove_event_listener_with_callback(name, callback.as_ref().unchecked_ref());
        }
        let window = &self.driver.provider.window;
        for id in [self.driver.pending.take(), self.driver.restoring.take()]
            .into_iter()
            .flatten()
        {
            let _ = window.cancel_animation_frame(id);
        }
        if let Ok(history) = window.history() {
            let _ = history.set_scroll_restoration(self.driver.previous);
        }
        let _ = js_sys::Reflect::delete_property(
            &js_sys::Object::from(window.clone()),
            &ATTACHED.into(),
        );
    }
}

/// Owns the existing WebApp; navigation state is released with its root Owner.
pub struct RoutingApp {
    app: crate::WebApp,
    router: RouterHandle,
}
impl RoutingApp {
    pub fn app(&self) -> &crate::WebApp {
        &self.app
    }
    pub fn router(&self) -> RouterHandle {
        self.router.clone()
    }
}
struct AdoptingHistory {
    provider: Rc<BrowserHistory>,
    initial: RefCell<Option<Entry>>,
}
impl History for AdoptingHistory {
    fn current(&self) -> Result<Entry> {
        match self.initial.borrow().as_ref() {
            Some(entry) => Ok(entry.clone()),
            None => self.provider.current(),
        }
    }
    fn push(&self, location: &Location) -> Result<Entry> {
        self.provider.push(location)
    }
    fn replace(&self, location: &Location) -> Result<Entry> {
        self.provider.replace(location)
    }
    fn preview(&self, delta: i32) -> Result<Traversal> {
        self.provider.preview(delta)
    }
    fn traverse(&self, delta: i32) -> Result<Option<Entry>> {
        self.provider.traverse(delta)
    }
}

/// Launch the same RouteViews Widget used by SSR/native. Scoped raw links can
/// opt in with data-glory-route; semantic links use normal Widget handlers.
pub fn launch_with<W: glory_ui::Widget>(
    root: Element,
    routes: glory_routing::Routes,
    factory: impl FnOnce(&glory_ui::Scope, RouterHandle) -> W + 'static,
    on_error: impl Fn(Error) + 'static,
) -> Result<RoutingApp> {
    mount(root, routes, None, factory, on_error)
}
#[cfg(feature = "hydrate")]
pub fn hydrate_with<W: glory_ui::Widget>(
    root: Element,
    routes: glory_routing::Routes,
    bootstrap: &glory_routing::Bootstrap,
    codec_schema: &str,
    factory: impl FnOnce(&glory_ui::Scope, RouterHandle) -> W + 'static,
    on_error: impl Fn(Error) + 'static,
) -> Result<RoutingApp> {
    let initial = bootstrap.validate(&routes, codec_schema)?;
    mount(root, routes, Some(initial), factory, on_error)
}
fn mount<W: glory_ui::Widget>(
    root: Element,
    routes: glory_routing::Routes,
    initial: Option<Location>,
    factory: impl FnOnce(&glory_ui::Scope, RouterHandle) -> W + 'static,
    on_error: impl Fn(Error) + 'static,
) -> Result<RoutingApp> {
    let window = root
        .owner_document()
        .and_then(|document| document.default_view())
        .ok_or_else(|| Error::History("missing window".into()))?;
    let provider = Rc::new(BrowserHistory::new(window)?);
    let entry = provider.current()?;
    let adopting = Rc::new(AdoptingHistory {
        provider: provider.clone(),
        initial: RefCell::new(initial.as_ref().map(|location| Entry {
            location: location.clone(),
            key: entry.key,
        })),
    });
    let captured = Rc::new(RefCell::new(None));
    let capture = captured.clone();
    let history = adopting.clone();
    let mut factory = Some(factory);
    let widget = glory_ui::widget(move |scope| {
        let router = glory_routing::Router::new(scope, routes.clone(), history.clone())
            .map_err(|error| glory_render::Error::Backend(error.to_string()))?;
        let handle = router.handle();
        *capture.borrow_mut() = Some(handle.clone());
        factory.take().ok_or(glory_ui::Error::Gone)?(scope, handle).show_in(scope)
    });
    let app = if initial.is_some() {
        #[cfg(feature = "hydrate")]
        {
            crate::hydrate(root.clone(), widget).map_err(|_| Error::HydrationMismatch)?
        }
        #[cfg(not(feature = "hydrate"))]
        {
            return Err(Error::Unsupported("hydrate feature"));
        }
    } else {
        crate::launch(root.clone(), widget).map_err(|error| Error::History(error.to_string()))?
    };
    adopting.initial.borrow_mut().take();
    let router = captured.borrow_mut().take().ok_or(Error::Gone)?;
    let lease = provider.attach(app.app().scope(), root, router.clone(), on_error)?;
    if provider.current()? != router.current()?.entry {
        lease.notify_location();
    }
    app.app().scope().on_cleanup(move || drop(lease))?;
    Ok(RoutingApp { app, router })
}
