use crate::{Error, Rect};

/// A manually positioned viewport layer. Position is independent of its logical
/// ancestors and never implies a popup anchor or a source lookup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportPlacement {
    pub x: f64,
    pub y: f64,
    pub viewport_padding: f64,
    pub constrain: bool,
}
impl Default for ViewportPlacement {
    fn default() -> Self {
        Self {
            x: 48.,
            y: 48.,
            viewport_padding: 8.,
            constrain: true,
        }
    }
}
impl ViewportPlacement {
    pub fn validate(self) -> Result<(), Error> {
        if ![self.x, self.y]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
            || !self.viewport_padding.is_finite()
            || !(0. ..=4096.).contains(&self.viewport_padding)
        {
            return Err(Error::InvalidProps(
                "viewport layer position and padding exceed finite bounds",
            ));
        }
        Ok(())
    }
    /// Preserve dimensions; oversized panels start at the padded viewport edge.
    pub fn resolve(self, panel: Rect, viewport: Rect) -> Result<Rect, Error> {
        self.validate()?;
        for rect in [panel, viewport] {
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
                || rect.width < 0.
                || rect.height < 0.
            {
                return Err(Error::InvalidProps(
                    "viewport layer geometry must be finite and nonnegative",
                ));
            }
        }
        let min_x = viewport.x + self.viewport_padding;
        let min_y = viewport.y + self.viewport_padding;
        let max_x = viewport.x + viewport.width - self.viewport_padding - panel.width;
        let max_y = viewport.y + viewport.height - self.viewport_padding - panel.height;
        if ![min_x, min_y, max_x, max_y].iter().all(|v| v.is_finite()) {
            return Err(Error::InvalidProps(
                "viewport layer geometry arithmetic overflow",
            ));
        }
        Ok(Rect {
            x: if self.constrain {
                self.x.clamp(min_x, max_x.max(min_x))
            } else {
                self.x
            },
            y: if self.constrain {
                self.y.clamp(min_y, max_y.max(min_y))
            } else {
                self.y
            },
            ..panel
        })
    }
}
