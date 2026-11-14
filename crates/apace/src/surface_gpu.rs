//! Native Hybrid surface ownership. The upstream renderer drives native resume
//! inline; retain its ready callback/complete_resume protocol rather than
//! assuming that creating a renderer makes a surface active.
use super::*;
use anyrender::RenderContext;
use anyrender_vello_hybrid::VelloHybridWindowRenderer;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// GPU renderer used by `shell`. No GPU instance is created until a nonzero
/// window surface is resumed. GPU failures are reported; there is no CPU fallback.
pub struct GpuRenderer {
    // Drop the surface/device before releasing our window handle.
    inner: Option<VelloHybridWindowRenderer>,
    window: Option<Arc<dyn WindowHandle>>,
    pub(super) error: Option<ShellError>,
    zero_ready: bool,
    frames: u64,
}
impl Default for GpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}
impl GpuRenderer {
    pub const BACKEND: WindowBackend = WindowBackend::VelloHybrid;

    pub fn new() -> Self {
        Self {
            inner: None,
            window: None,
            error: None,
            zero_ready: false,
            frames: 0,
        }
    }
    /// Recorded frames, not successful surface presents.
    pub fn frames(&self) -> u64 {
        self.frames
    }
    pub fn presented_frames(&self) -> Option<u64> {
        None
    }
    pub fn take_error(&mut self) -> Option<ShellError> {
        self.error.take()
    }
    /// The actual selected adapter, available after a successful resume. A
    /// software adapter is possible even though the Hybrid backend is selected.
    pub fn adapter_info(&self) -> Option<anyrender_vello_hybrid::wgpu::AdapterInfo> {
        Some(
            self.inner
                .as_ref()?
                .current_device_handle()?
                .adapter
                .get_info(),
        )
    }
    fn maximum_dimension(&self) -> u32 {
        self.inner
            .as_ref()
            .and_then(VelloHybridWindowRenderer::current_device_handle)
            .map(|handle| handle.device.limits().max_texture_dimension_2d)
            .unwrap_or_else(|| {
                anyrender_vello_hybrid::wgpu::Limits::default().max_texture_dimension_2d
            })
            .min(u32::from(u16::MAX))
    }
    fn valid_size(&mut self, width: u32, height: u32) -> bool {
        // Check zero first: minimization must release resources even if the
        // other dimension is outside the renderer's representable range.
        if width == 0 || height == 0 {
            self.inner = None;
            self.zero_ready = true;
            return false;
        }
        let maximum = self.maximum_dimension();
        if width > maximum || height > maximum {
            self.inner = None;
            self.zero_ready = false;
            self.error = Some(ShellError::GpuDimensions {
                width,
                height,
                maximum,
            });
            return false;
        }
        self.zero_ready = false;
        true
    }
    fn failed(&mut self, payload: Box<dyn std::any::Any + Send>) {
        // The pinned upstream API uses expect for adapter/surface failures.
        // Convert unwinding backend failures to the Shell's existing error path.
        // panic=abort and OOM cannot be recovered by this boundary.
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("GPU backend panicked");
        self.error = Some(ShellError::Renderer(message.to_owned()));
        self.inner = None;
        self.zero_ready = false;
    }
    fn start<F: FnOnce() + 'static>(&mut self, width: u32, height: u32, on_ready: F) {
        if !self.valid_size(width, height) {
            // A zero surface still needs a document poll waker, so that the
            // application can make progress before a later resize restores it.
            on_ready();
            return;
        }
        let Some(window) = self.window.clone() else {
            return;
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut inner = VelloHybridWindowRenderer::new();
            inner.resume(window, width, height, on_ready);
            inner
        }));
        match result {
            Ok(inner) => self.inner = Some(inner),
            Err(payload) => self.failed(payload),
        }
    }
}
impl RenderContext for GpuRenderer {
    fn try_register_custom_resource(
        &mut self,
        resource: Box<dyn std::any::Any>,
    ) -> Result<anyrender::ResourceId, anyrender::RegisterResourceError> {
        match &mut self.inner {
            Some(inner) => inner.try_register_custom_resource(resource),
            None => Err(anyrender::RegisterResourceErrorKind::Other.into()),
        }
    }
    fn unregister_resource(&mut self, resource_id: anyrender::ResourceId) {
        if let Some(inner) = &mut self.inner {
            inner.unregister_resource(resource_id);
        }
    }
    fn renderer_specific_context(&self) -> Option<Box<dyn std::any::Any>> {
        self.inner.as_ref()?.renderer_specific_context()
    }
}
impl WindowRenderer for GpuRenderer {
    type ScenePainter<'a> = <VelloHybridWindowRenderer as WindowRenderer>::ScenePainter<'a>;

    fn resume<F: FnOnce() + 'static>(
        &mut self,
        window: Arc<dyn WindowHandle>,
        width: u32,
        height: u32,
        on_ready: F,
    ) {
        self.suspend();
        self.window = Some(window);
        self.start(width, height, on_ready);
    }
    fn complete_resume(&mut self) -> bool {
        self.error.is_none()
            && (self.zero_ready
                || self
                    .inner
                    .as_mut()
                    .is_some_and(|inner| inner.complete_resume()))
    }
    fn suspend(&mut self) {
        self.inner = None;
        self.window = None;
        self.zero_ready = false;
    }
    fn is_active(&self) -> bool {
        self.inner.as_ref().is_some_and(WindowRenderer::is_active)
    }
    fn is_pending(&self) -> bool {
        self.inner.as_ref().is_some_and(WindowRenderer::is_pending)
    }
    fn set_size(&mut self, width: u32, height: u32) {
        if !self.valid_size(width, height) {
            return;
        }
        if let Some(inner) = &mut self.inner {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| inner.set_size(width, height))) {
                self.failed(payload);
            }
        } else if self.window.is_some() && self.error.is_none() {
            self.start(width, height, || {});
            // Native resume is synchronous in the pinned backend. The initial
            // Shell resume still uses its queued ready notification.
            self.complete_resume();
        }
    }
    fn render<F: FnOnce(&mut Self::ScenePainter<'_>)>(&mut self, draw: F) {
        let Some(inner) = &mut self.inner else {
            return;
        };
        if !inner.is_active() {
            return;
        }
        let mut drawn = false;
        let result = catch_unwind(AssertUnwindSafe(|| {
            inner.render(|scene| {
                drawn = true;
                draw(scene);
            });
        }));
        match result {
            Ok(()) if drawn => self.frames += 1,
            Ok(()) => {}
            Err(payload) => self.failed(payload),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wingy::raw_window_handle::{
        DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle as RawWindow,
    };

    struct NoSurface;
    impl HasWindowHandle for NoSurface {
        fn window_handle(&self) -> Result<RawWindow<'_>, HandleError> {
            panic!("zero-size resume must not request a native handle")
        }
    }
    impl HasDisplayHandle for NoSurface {
        fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
            panic!("zero-size resume must not request a display handle")
        }
    }

    #[test]
    fn zero_resume_notifies_once_and_suspend_releases_the_window() {
        let window = Arc::new(NoSurface);
        let weak = Arc::downgrade(&window);
        let ready = Rc::new(Cell::new(0));
        let notified = ready.clone();
        let mut renderer = GpuRenderer::new();
        renderer.resume(window, 0, 500, move || notified.set(notified.get() + 1));
        assert_eq!(ready.get(), 1);
        assert!(renderer.complete_resume());
        assert!(renderer.complete_resume());
        assert!(renderer.take_error().is_none());
        assert!(!renderer.is_active());
        assert!(renderer.inner.is_none());
        assert!(weak.upgrade().is_some());
        renderer.suspend();
        assert!(weak.upgrade().is_none());
        assert!(!renderer.complete_resume());
    }

    #[test]
    fn dimensions_and_zero_size_are_checked_before_gpu_initialization() {
        let mut renderer = GpuRenderer::new();
        assert!(renderer.inner.is_none());
        renderer.set_size(u32::MAX, u32::MAX);
        assert!(matches!(
            renderer.error.take(),
            Some(ShellError::GpuDimensions { .. })
        ));
        renderer.set_size(0, u32::MAX);
        assert!(renderer.complete_resume());
        assert!(!renderer.is_active());
        assert!(!renderer.is_pending());
        assert!(renderer.inner.is_none());
        renderer.suspend();
        assert!(!renderer.complete_resume());
        assert_eq!(renderer.frames(), 0);
        assert_eq!(renderer.presented_frames(), None);
    }
}
