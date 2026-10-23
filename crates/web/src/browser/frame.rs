use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use futures_util::future::{AbortHandle, Abortable};
use glory_core::{Host, HostError, LocalFuture};
use glory_ui::App;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::Window;

pub(super) struct Frame {
    window: Window,
    app: RefCell<Weak<App>>,
    callback: RefCell<Option<Closure<dyn FnMut(f64)>>>,
    pending: Cell<Option<i32>>,
    processing: Cell<bool>,
    again: Cell<bool>,
    stopped: Cell<bool>,
}
impl Frame {
    pub fn profile(&self) -> Result<glory_render::profile::Profile, glory_render::Error> {
        let clock = self
            .window
            .performance()
            .ok_or(glory_render::Error::Unsupported(
                "browser Performance clock",
            ))?;
        Ok(glory_render::profile::Profile::new(move || {
            Duration::from_secs_f64(clock.now() / 1000.)
        }))
    }
    pub fn new(window: Window) -> Rc<Self> {
        let frame = Rc::new(Self {
            window,
            app: RefCell::new(Weak::new()),
            callback: RefCell::new(None),
            pending: Cell::new(None),
            processing: Cell::new(false),
            again: Cell::new(false),
            stopped: Cell::new(false),
        });
        let weak = Rc::downgrade(&frame);
        *frame.callback.borrow_mut() = Some(Closure::new(move |_| {
            let Some(frame) = weak.upgrade() else { return };
            frame.pending.set(None);
            if frame.stopped.get() {
                return;
            }
            frame.processing.set(true);
            let app = frame.app.borrow().upgrade();
            if let Some(app) = app
                && let Err(error) = app.pump()
            {
                web_sys::console::error_1(&format!("Glory frame: {error}").into());
            }
            frame.processing.set(false);
            if frame.again.replace(false) {
                frame.schedule();
            }
        }));
        frame
    }
    pub fn attach(&self, app: &Rc<App>) {
        *self.app.borrow_mut() = Rc::downgrade(app);
    }
    fn schedule(&self) {
        if self.stopped.get() || self.pending.get().is_some() {
            return;
        }
        if let Some(callback) = self.callback.borrow().as_ref() {
            match self
                .window
                .request_animation_frame(callback.as_ref().unchecked_ref())
            {
                Ok(id) => self.pending.set(Some(id)),
                Err(error) => web_sys::console::error_1(&error),
            }
        }
    }
    pub fn input(&self) {
        if self.processing.get() {
            self.again.set(true);
        } else {
            self.schedule();
        }
    }
    pub fn stop(&self) {
        self.stopped.set(true);
        if let Some(id) = self.pending.take() {
            let _ = self.window.cancel_animation_frame(id);
        }
        *self.app.borrow_mut() = Weak::new();
    }
}
impl Host for Frame {
    fn spawn(&self, future: LocalFuture) -> Result<Box<dyn FnOnce()>, HostError> {
        let (abort, registration) = AbortHandle::new_pair();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = Abortable::new(future, registration).await;
        });
        Ok(Box::new(move || abort.abort()))
    }
    fn wake(&self) {
        if !self.processing.get() {
            self.schedule();
        }
    }
    fn now(&self) -> Duration {
        Duration::from_secs_f64(self.window.performance().map_or(0.0, |clock| clock.now()) / 1000.0)
    }
}
