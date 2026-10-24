use super::*;

/// Metadata for a recorded frame, never proof of display/presentation. Logical
/// UI mutations and visual invalidations use separate watermarks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameInfo {
    pub runtime: RuntimeId,
    /// Process-local engine document identity. Distinct consumers can share a
    /// RuntimeId; a clone of the same Renderer shares this document identity.
    pub document: usize,
    pub revision: u64,
    pub applied: u64,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}
impl FrameInfo {
    #[cfg(feature = "raster")]
    fn same_pixels(self, other: Self) -> bool {
        self.runtime == other.runtime
            && self.document == other.document
            && self.revision == other.revision
            && self.width == other.width
            && self.height == other.height
            && self.scale == other.scale
    }
}
impl Renderer {
    pub fn frame_info(&self) -> FrameInfo {
        let state = self.0.borrow();
        frame_info(&state)
    }
    /// Record a complete document frame into the selected drawing backend.
    /// Resets the scene first; frame commands do not accumulate across calls.
    /// This resolves layout but does not flush an App's uncommitted mutations.
    pub fn paint(&self, scene: &mut impl anyrender::PaintScene) -> FrameInfo {
        scene.reset();
        let mut state = self.0.borrow_mut();
        if let Err(error) = layers::sync(&mut state, &[]) {
            state.input_error = Some(error);
        }
        state.dom.resolve();
        let info = frame_info(&state);
        if info.width > 0 && info.height > 0 {
            let _span = state
                .dom
                .profile
                .as_ref()
                .map(|profile| profile.span(Phase::Paint));
            blitz_paint::paint_scene(
                scene,
                &mut state.dom.doc,
                f64::from(info.scale),
                info.width,
                info.height,
                0,
                0,
            );
        }
        info
    }
}
fn frame_info(state: &State) -> FrameInfo {
    let viewport = state.dom.doc.viewport();
    FrameInfo {
        runtime: state.runtime,
        document: state.dom.doc.id(),
        revision: state.dom.revision,
        applied: state.receipt.applied,
        width: viewport.window_size.0,
        height: viewport.window_size.1,
        scale: viewport.scale(),
    }
}

#[cfg(feature = "raster")]
mod raster {
    use super::*;
    use anyrender::ImageRenderer;
    use anyrender_vello_cpu::VelloCpuImageRenderer;

    /// Errors leave the previous successfully rendered image intact.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum RasterError {
        Dimensions { width: u32, height: u32 },
        Limit { bytes: u64, limit: usize },
        Allocation,
    }
    impl std::fmt::Display for RasterError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{self:?}")
        }
    }
    impl std::error::Error for RasterError {}

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Draw {
        pub info: FrameInfo,
        /// False reuses the existing pixels without layout/paint/raster work.
        pub rendered: bool,
    }
    /// One reusable CPU renderer and one premultiplied sRGB RGBA8 backbuffer.
    /// Raster limits cover the backbuffer, not arbitrary engine/backend allocations.
    pub struct Raster {
        backend: Option<VelloCpuImageRenderer>,
        pixels: Vec<u8>,
        frame: Option<FrameInfo>,
        limit: usize,
        renders: u64,
    }
    impl Default for Raster {
        fn default() -> Self {
            Self::new()
        }
    }
    impl Raster {
        pub const DEFAULT_LIMIT: usize = 64 * 1024 * 1024;
        pub fn new() -> Self {
            Self::with_limit(Self::DEFAULT_LIMIT)
        }
        pub fn with_limit(limit: usize) -> Self {
            Self {
                backend: None,
                pixels: Vec::new(),
                frame: None,
                limit,
                renders: 0,
            }
        }
        pub fn pixels(&self) -> &[u8] {
            &self.pixels
        }
        pub fn frame(&self) -> Option<FrameInfo> {
            self.frame
        }
        pub fn renders(&self) -> u64 {
            self.renders
        }
        pub fn buffer_capacity(&self) -> usize {
            self.pixels.capacity()
        }
        /// Release frame, backbuffer, glyph/image caches and renderer resources.
        pub fn clear(&mut self) {
            self.backend = None;
            self.pixels = Vec::new();
            self.frame = None;
        }
        pub fn draw(&mut self, document: &Renderer) -> Result<Draw, RasterError> {
            document.poll_resources();
            let mut info = document.frame_info();
            if self
                .frame
                .is_some_and(|previous| previous.same_pixels(info))
            {
                self.frame = Some(info);
                return Ok(Draw {
                    info,
                    rendered: false,
                });
            }
            if info.width == 0 || info.height == 0 {
                self.clear();
                self.frame = Some(info);
                return Ok(Draw {
                    info,
                    rendered: false,
                });
            }
            // The pinned CPU backend casts dimensions to u16 internally. Reject
            // truncation before creating it, regardless of the byte budget.
            if info.width > u32::from(u16::MAX) || info.height > u32::from(u16::MAX) {
                return Err(RasterError::Dimensions {
                    width: info.width,
                    height: info.height,
                });
            }
            let bytes = u64::from(info.width) * u64::from(info.height) * 4;
            if bytes > self.limit as u64 {
                return Err(RasterError::Limit {
                    bytes,
                    limit: self.limit,
                });
            }
            let _span = document.profile().map(|profile| profile.span(Phase::Draw));
            let len = usize::try_from(bytes).map_err(|_| RasterError::Allocation)?;
            let resized = self.frame.is_none_or(|previous| {
                previous.document != info.document
                    || previous.runtime != info.runtime
                    || previous.width != info.width
                    || previous.height != info.height
            });
            if resized {
                let mut pixels = Vec::new();
                pixels
                    .try_reserve_exact(len)
                    .map_err(|_| RasterError::Allocation)?;
                pixels.resize(len, 0);
                // A resize also releases caches/tile storage from the old size.
                let backend = VelloCpuImageRenderer::new(info.width, info.height);
                self.backend = Some(backend);
                self.pixels = pixels;
            }
            self.pixels.fill(0);
            let backend = self.backend.as_mut().expect("nonempty raster backend");
            backend.render(
                |scene| {
                    info = document.paint(scene);
                },
                &mut self.pixels,
            );
            self.frame = Some(info);
            self.renders += 1;
            Ok(Draw {
                info,
                rendered: true,
            })
        }
    }
}
#[cfg(feature = "raster")]
pub use raster::*;
