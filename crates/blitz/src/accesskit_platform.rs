//! UI-thread platform ownership. Foreign-thread callbacks only enqueue messages.
use super::*;
use accesskit_xplat::{Adapter, EventHandler, WindowEvent as AxEvent};
use winit::raw_window_handle::HasWindowHandle;
use winit::window::Window;

pub(super) struct Message {
    pub window: WindowId,
    pub runtime: RuntimeId,
    pub event: AxEvent,
}
struct Handler {
    window: WindowId,
    runtime: RuntimeId,
    proxy: BlitzShellProxy,
}
impl EventHandler for Handler {
    fn handle_accesskit_event(&self, event: AxEvent) {
        self.proxy
            .send_event(BlitzShellEvent::embedder_event(Message {
                window: self.window,
                runtime: self.runtime,
                event,
            }));
    }
}
pub(super) struct Platform {
    adapter: Adapter,
    // Keep the native handle alive until after Adapter removes its subclass.
    window: Arc<dyn Window>,
    active: bool,
    full: bool,
    revision: Option<u64>,
    title: String,
}
impl Platform {
    pub(super) fn new(view: &mut blitz_shell::View<CpuRenderer>) -> Self {
        let window = view.window.clone();
        let runtime = view.downcast_doc_mut::<WindowApp>().app.runtime().id();
        let adapter = Adapter::with_combined_handler(
            window.window_handle().expect("live native window").as_raw(),
            Arc::new(Handler {
                window: window.id(),
                runtime,
                proxy: view.proxy.clone(),
            }),
        );
        let requested = view.downcast_doc_mut::<WindowApp>().requested_visible;
        view.is_visible = requested;
        window.set_visible(requested);
        let mut platform = Self {
            adapter,
            window,
            active: false,
            full: true,
            revision: None,
            title: String::new(),
        };
        platform.bounds();
        platform
    }
    fn bounds(&mut self) {
        let outer: (f64, f64) = self
            .window
            .outer_position()
            .unwrap_or_default()
            .cast::<f64>()
            .into();
        let size: (f64, f64) = self.window.outer_size().cast::<f64>().into();
        let inner: (f64, f64) = self.window.surface_position().cast::<f64>().into();
        let inner_size: (f64, f64) = self.window.surface_size().cast::<f64>().into();
        self.adapter.set_window_bounds(
            ::accesskit::Rect::from_origin_size(outer, size),
            ::accesskit::Rect::from_origin_size(inner, inner_size),
        );
        self.revision = None;
    }
    pub(super) fn window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Focused(focused) => self.adapter.set_focus(*focused),
            WindowEvent::Moved(_)
            | WindowEvent::SurfaceResized(_)
            | WindowEvent::ScaleFactorChanged { .. } => self.bounds(),
            _ => {}
        }
    }
    pub(super) fn event(&mut self, view: &mut blitz_shell::View<CpuRenderer>, event: &AxEvent) {
        match event {
            AxEvent::InitialTreeRequested => {
                self.active = true;
                self.full = true;
                self.revision = None;
            }
            AxEvent::AccessibilityDeactivated => {
                self.active = false;
                self.revision = None;
                view.downcast_doc_mut::<WindowApp>()
                    .accessibility
                    .borrow_mut()
                    .clear();
            }
            AxEvent::ActionRequested(request) => {
                if self.active {
                    view.poll();
                    let app = view.downcast_doc_mut::<WindowApp>();
                    // Revalidate identity, connection, current visibility and
                    // enabled state. Refused/stale OS requests do not fault UI.
                    let accepted = app
                        .accessibility
                        .borrow_mut()
                        .action(&mut app.renderer.0.borrow_mut(), request)
                        .unwrap_or(false);
                    if accepted && request.action == ::accesskit::Action::Focus {
                        self.window.focus_window();
                    }
                    view.poll();
                }
            }
        }
    }
    pub(super) fn update(&mut self, view: &mut blitz_shell::View<CpuRenderer>) -> glory_ui::Result {
        if !self.active {
            return Ok(());
        }
        let insets = view.safe_area_insets;
        let title = self.window.title();
        let app = view.downcast_doc_mut::<WindowApp>();
        let mut state = app.renderer.0.borrow_mut();
        if self.revision == Some(state.dom.revision) && self.title == title {
            return Ok(());
        }
        // A moved/resized window invalidates geometry, not the platform tree's
        // synchronization state. Only activation needs a complete initial tree.
        let full = self.full;
        let update = app.accessibility.borrow_mut().build(
            &mut state,
            full,
            (f64::from(insets.left), f64::from(insets.top)),
            &title,
        )?;
        self.title = title;
        self.full = false;
        self.revision = Some(state.dom.revision);
        let needs_poll = !state.inputs.is_empty() || state.observation_pending;
        drop(state);
        self.adapter.update_if_active(|| update);
        if needs_poll {
            view.proxy.send_event(BlitzShellEvent::Poll {
                window_id: view.window.id(),
            });
        }
        Ok(())
    }
}
