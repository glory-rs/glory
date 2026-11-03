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
    dom: RefCell<Weak<super::dom::Dom>>,
    callback: RefCell<Option<Closure<dyn FnMut(f64)>>>,
    pending: Cell<Option<i32>>,
    processing: Cell<bool>,
    again: Cell<bool>,
    stopped: Cell<bool>,
    pointer_ready_at: Cell<Option<f64>>,
    pointer_scheduled: Cell<bool>,
    pointer_timer: Cell<Option<i32>>,
    pointer_callback: RefCell<Option<Closure<dyn FnMut()>>>,
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
            dom: RefCell::new(Weak::new()),
            callback: RefCell::new(None),
            pending: Cell::new(None),
            processing: Cell::new(false),
            again: Cell::new(false),
            stopped: Cell::new(false),
            pointer_ready_at: Cell::new(None),
            pointer_scheduled: Cell::new(false),
            pointer_timer: Cell::new(None),
            pointer_callback: RefCell::new(None),
        });
        let weak = Rc::downgrade(&frame);
        *frame.callback.borrow_mut() = Some(Closure::new(move |_| {
            let Some(frame) = weak.upgrade() else { return };
            frame.pending.set(None);
            frame.run();
        }));
        frame
    }
    fn run(&self) {
        if self.stopped.get() {
            return;
        }
        self.cancel_pointer_timer();
        let clock = self
            .pointer_ready_at
            .get()
            .and_then(|_| self.window.performance());
        let start = clock.as_ref().map(|clock| clock.now());
        self.processing.set(true);
        let app = self.app.borrow().upgrade();
        if let Some(app) = app
            && let Err(error) = app.pump()
        {
            web_sys::console::error_1(&format!("Glory frame: {error}").into());
        }
        if let Some(dom) = self.dom.borrow().upgrade()
            && let Err(error) = super::sheet::finish_turn(&dom)
        {
            dom.error.borrow_mut().get_or_insert(error);
        }
        self.processing.set(false);
        if let Some((clock, start)) = clock.zip(start)
            && self.pointer_ready_at.get().is_some()
        {
            let end = clock.now();
            if end - start > 2.5 {
                // Extra turns compete with layout and paint on busy pages.
                // Retire raw input for this capture and use the RAF path.
                self.pointer_pacing(false);
                let raw = self.dom.borrow().upgrade().and_then(|dom| {
                    dom.drag
                        .borrow_mut()
                        .as_mut()
                        .and_then(|drag| drag.raw.take())
                });
                drop(raw);
            } else {
                self.pointer_ready_at.set(Some(end + 4.0));
            }
        }
        if self.again.replace(false) {
            self.schedule();
        }
    }
    pub(super) fn pointer_pacing(&self, active: bool) {
        if !active {
            self.cancel_pointer_timer();
        }
        self.pointer_ready_at
            .set(active.then_some(f64::NEG_INFINITY));
    }
    fn cancel_pointer_timer(&self) {
        if let Some(id) = self.pointer_timer.take() {
            self.window.clear_timeout_with_handle(id);
        }
    }
    fn pointer_wait(&self) -> Option<f64> {
        if self.stopped.get() || self.processing.get() {
            return None;
        }
        Some(self.pointer_ready_at.get()? - self.window.performance()?.now())
    }
    fn run_pointer(&self) {
        if self.pointer_wait().is_some_and(|wait| wait <= 0.0)
            && let Some(id) = self.pending.take()
        {
            let _ = self.window.cancel_animation_frame(id);
            self.run();
        }
    }
    pub(super) fn prompt_pointer(self: &Rc<Self>) {
        if self.pointer_scheduled.get() || self.pointer_timer.get().is_some() {
            return;
        }
        let Some(wait) = self.pointer_wait() else {
            return;
        };
        if wait > 0.0 {
            // One paced wake per capture. If RAF wins, it handles the same
            // queued sample and retires the timer.
            let mut callback = self.pointer_callback.borrow_mut();
            let callback = callback.get_or_insert_with(|| {
                let frame = Rc::downgrade(self);
                Closure::new(move || {
                    if let Some(frame) = frame.upgrade() {
                        frame.pointer_timer.set(None);
                        frame.run_pointer();
                    }
                })
            });
            if let Ok(id) = self
                .window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    callback.as_ref().unchecked_ref(),
                    wait.ceil() as i32,
                )
            {
                self.pointer_timer.set(Some(id));
            }
            return;
        }
        self.pointer_scheduled.set(true);
        let frame = Rc::downgrade(self);
        // The browser listener has returned before any business code runs.
        wasm_bindgen_futures::spawn_local(async move {
            let Some(frame) = frame.upgrade() else { return };
            frame.pointer_scheduled.set(false);
            frame.run_pointer();
        });
    }
    pub fn attach_dom(&self, dom: &Rc<super::dom::Dom>) {
        *self.dom.borrow_mut() = Rc::downgrade(dom);
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
    pub fn has_pending_turn(&self) -> bool {
        self.pending.get().is_some() || self.processing.get() || self.again.get()
    }
    pub fn stop(&self) {
        self.stopped.set(true);
        self.cancel_pointer_timer();
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
