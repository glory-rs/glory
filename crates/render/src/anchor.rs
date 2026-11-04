use crate::{Error, Rect};

/// Physical viewport side; independent of document writing direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnchorSide {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}
impl AnchorSide {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
    fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnchorAlign {
    #[default]
    Start,
    Center,
    End,
}
/// Finite arrow placement within the resolved panel edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnchorArrowPosition {
    /// Point at the physical center of the live anchor, clamped inside the panel.
    #[default]
    Center,
    /// Keep a fixed inset from the aligned panel edge.
    Side,
}

/// Host-rendered anchored-layer arrow, in viewport CSS pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnchorArrow {
    pub size: f64,
    pub radius: f64,
    pub offset: f64,
    pub position: AnchorArrowPosition,
}
impl Default for AnchorArrow {
    fn default() -> Self {
        Self {
            size: 8.0,
            radius: 1.0,
            offset: 10.0,
            position: AnchorArrowPosition::Center,
        }
    }
}
impl AnchorArrow {
    pub fn validate(self) -> Result<(), Error> {
        if [self.size, self.radius, self.offset]
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=4096.0).contains(value))
            || self.size == 0.0
            || self.radius > self.size / 2.0
        {
            return Err(Error::InvalidProps(
                "anchor arrow size, radius and offset must be finite; size must be positive and radius no greater than half the size",
            ));
        }
        Ok(())
    }
}
impl AnchorAlign {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
        }
    }
}
/// Finite anchored-layer policy, in viewport CSS pixels. Flip compares the
/// requested and opposite side; shift clamps both axes to viewport padding.
/// Neither policy changes the declared side or the logical anchor relation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnchorPlacement {
    pub side: AnchorSide,
    pub align: AnchorAlign,
    pub gap: f64,
    pub viewport_padding: f64,
    pub flip: bool,
    pub shift: bool,
    /// Size the panel border box to the live anchor width before placement.
    pub match_width: bool,
    /// Render a host-owned arrow using the collision-resolved side.
    pub arrow: Option<AnchorArrow>,
}
impl Default for AnchorPlacement {
    fn default() -> Self {
        Self::new(AnchorSide::Bottom, AnchorAlign::Start)
    }
}
impl AnchorPlacement {
    pub const fn new(side: AnchorSide, align: AnchorAlign) -> Self {
        Self {
            side,
            align,
            gap: 6.0,
            viewport_padding: 8.0,
            flip: true,
            shift: true,
            match_width: false,
            arrow: None,
        }
    }
    pub const fn gap(mut self, value: f64) -> Self {
        self.gap = value;
        self
    }
    pub fn validate(self) -> Result<(), Error> {
        if [self.gap, self.viewport_padding]
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=4096.0).contains(v))
        {
            return Err(Error::InvalidProps(
                "anchor gap and viewport padding must be finite and within 4096 CSS pixels",
            ));
        }
        if let Some(arrow) = self.arrow {
            arrow.validate()?;
        }
        Ok(())
    }
    /// Pure finite geometry. `panel` contributes dimensions only. The host
    /// supplies the current anchor, including any pending ancestor-layer delta.
    pub fn resolve(
        self,
        anchor: Rect,
        panel: Rect,
        viewport: Rect,
    ) -> Result<AnchorPosition, Error> {
        self.validate()?;
        for rect in [anchor, panel, viewport] {
            if ![
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                rect.x + rect.width,
                rect.y + rect.height,
            ]
            .iter()
            .all(|v| v.is_finite())
                || rect.width < 0.0
                || rect.height < 0.0
            {
                return Err(Error::InvalidProps(
                    "anchor geometry must be finite with nonnegative dimensions",
                ));
            }
        }
        let candidate = |side| {
            let vertical = matches!(side, AnchorSide::Top | AnchorSide::Bottom);
            let (start, extent, size) = if vertical {
                (anchor.x, anchor.width, panel.width)
            } else {
                (anchor.y, anchor.height, panel.height)
            };
            let cross = match self.align {
                AnchorAlign::Start => start,
                AnchorAlign::Center => start + (extent - size) / 2.0,
                AnchorAlign::End => start + extent - size,
            };
            let (x, y) = match side {
                AnchorSide::Top => (cross, anchor.y - panel.height - self.gap),
                AnchorSide::Bottom => (cross, anchor.y + anchor.height + self.gap),
                AnchorSide::Left => (anchor.x - panel.width - self.gap, cross),
                AnchorSide::Right => (anchor.x + anchor.width + self.gap, cross),
            };
            AnchorPosition { x, y, side }
        };
        let min_x = viewport.x + self.viewport_padding;
        let min_y = viewport.y + self.viewport_padding;
        let max_x = viewport.x + viewport.width - self.viewport_padding;
        let max_y = viewport.y + viewport.height - self.viewport_padding;
        let overflow = |p: AnchorPosition| {
            (min_x - p.x).max(0.0)
                + (p.x + panel.width - max_x).max(0.0)
                + (min_y - p.y).max(0.0)
                + (p.y + panel.height - max_y).max(0.0)
        };
        let mut result = candidate(self.side);
        let opposite = candidate(self.side.opposite());
        if ![
            result.x,
            result.y,
            opposite.x,
            opposite.y,
            min_x,
            min_y,
            max_x,
            max_y,
            overflow(result),
            overflow(opposite),
        ]
        .iter()
        .all(|v| v.is_finite())
        {
            return Err(Error::InvalidProps("anchor geometry arithmetic overflow"));
        }
        if self.flip && overflow(opposite) < overflow(result) {
            result = opposite;
        }
        if self.shift {
            result.x = result.x.clamp(min_x, (max_x - panel.width).max(min_x));
            result.y = result.y.clamp(min_y, (max_y - panel.height).max(min_y));
        }
        Ok(result)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnchorPosition {
    pub x: f64,
    pub y: f64,
    pub side: AnchorSide,
}
