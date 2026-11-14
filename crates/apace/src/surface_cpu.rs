use super::*;

type VelloCpuWindowRenderer = softbuffer_window_renderer::SoftbufferWindowRenderer<
    anyrender_vello_cpu::VelloCpuImageRenderer,
>;

/// Guard the pinned CPU backend's u16 dimensions and its RGBA allocation.
/// Zero-size releases the surface; a later resize resumes the same document.
pub struct CpuRenderer {
    inner: VelloCpuWindowRenderer,
    window: Option<Arc<dyn WindowHandle>>,
    pub(super) error: Option<ShellError>,
    limit: usize,
    frames: u64,
}
impl Default for CpuRenderer {
    fn default() -> Self {
        Self::new()
    }
}
impl CpuRenderer {
    pub const BACKEND: WindowBackend = WindowBackend::VelloCpu;
    pub fn new() -> Self {
        Self::with_limit(64 * 1024 * 1024)
    }
    pub fn with_limit(limit: usize) -> Self {
        Self {
            inner: VelloCpuWindowRenderer::new(),
            window: None,
            error: None,
            limit,
            frames: 0,
        }
    }
    pub fn frames(&self) -> u64 {
        self.frames
    }
    pub fn presented_frames(&self) -> Option<u64> {
        Some(self.frames)
    }
    pub fn take_error(&mut self) -> Option<ShellError> {
        self.error.take()
    }
    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            self.inner.suspend();
            self.inner = VelloCpuWindowRenderer::new();
            return;
        }
        // Check dimensions first: arbitrary u32 width*height*4 can overflow u64.
        if width > u32::from(u16::MAX)
            || height > u32::from(u16::MAX)
            || u64::from(width) * u64::from(height) * 4 > self.limit as u64
        {
            self.inner.suspend();
            self.error = Some(ShellError::Surface {
                width,
                height,
                limit: self.limit,
            });
            return;
        }
        if self.inner.is_active() {
            self.inner.set_size(width, height);
        } else if let Some(window) = &self.window {
            self.inner.resume(window.clone(), width, height, || {});
            self.inner.complete_resume();
        }
    }
}
impl anyrender::RenderContext for CpuRenderer {}
impl WindowRenderer for CpuRenderer {
    type ScenePainter<'a> = <VelloCpuWindowRenderer as WindowRenderer>::ScenePainter<'a>;
    fn resume<F: FnOnce() + 'static>(
        &mut self,
        window: Arc<dyn WindowHandle>,
        width: u32,
        height: u32,
        on_ready: F,
    ) {
        self.window = Some(window);
        self.resize(width, height);
        on_ready();
    }
    fn complete_resume(&mut self) -> bool {
        // Retain a waker for an owned zero-size window, but do not reinstall
        // one when a queued readiness notification outlives surface suspension.
        self.window.is_some()
    }
    fn suspend(&mut self) {
        self.inner.suspend();
        self.inner = VelloCpuWindowRenderer::new();
        self.window = None;
    }
    fn is_active(&self) -> bool {
        self.inner.is_active()
    }
    fn set_size(&mut self, width: u32, height: u32) {
        self.resize(width, height);
    }
    fn render<F: FnOnce(&mut Self::ScenePainter<'_>)>(&mut self, draw: F) {
        if self.is_active() {
            let mut painted = false;
            self.inner.render(|scene| {
                painted = true;
                draw(scene);
            });
            // The pinned softbuffer backend skips draw if buffer acquisition
            // fails, and returns normally after a successful present otherwise.
            if painted {
                self.frames += 1;
            }
        }
    }
}
