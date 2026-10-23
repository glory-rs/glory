use super::{Error, nonnegative, positive, valid_extent};

/// A bounded physical scroll surface with logical coordinates. Re-centering
/// preserves pixel-sized wheel movement instead of multiplying every wheel
/// delta by the ratio of a huge data set to a browser's safe scroll extent.
#[derive(Debug)]
pub struct ScrollSpace {
    total: f64,
    viewport: f64,
    limit: f64,
    origin: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollUpdate {
    pub logical: f64,
    pub physical: f64,
    /// Host must set scrollTop and reposition rows together when true.
    pub rebased: bool,
}

impl ScrollSpace {
    pub fn new(total: f64, viewport: f64, limit: f64) -> Result<Self, Error> {
        if !valid_extent(total)
            || !nonnegative(viewport)
            || !positive(limit)
            || (total > limit && viewport >= limit)
        {
            return Err(Error::InvalidSize);
        }
        Ok(Self {
            total,
            viewport,
            limit,
            origin: 0.0,
        })
    }

    pub fn extent(&self) -> f64 {
        self.total.min(self.limit)
    }
    pub fn origin(&self) -> f64 {
        self.origin
    }
    pub fn project(&self, logical: f64) -> f64 {
        logical - self.origin
    }

    /// Programmatic index/end/home navigation uses a logical target. The native
    /// scrollbar represents the current segment, not a misleading scaled total.
    pub fn seek(&mut self, logical: f64) -> Result<ScrollUpdate, Error> {
        if !nonnegative(logical) {
            return Err(Error::InvalidSize);
        }
        let logical = logical.min((self.total - self.viewport).max(0.0));
        let previous = self.origin;
        let span = (self.extent() - self.viewport).max(0.0);
        self.origin = (logical - span / 2.0).clamp(0.0, (self.total - self.extent()).max(0.0));
        Ok(ScrollUpdate {
            logical,
            physical: logical - self.origin,
            rebased: self.origin != previous,
        })
    }

    /// Process one coalesced host scroll position. Rebase near the segment edges
    /// before the physical end prevents further wheel/key movement.
    pub fn scrolled(&mut self, physical: f64) -> Result<ScrollUpdate, Error> {
        if !physical.is_finite() {
            return Err(Error::InvalidSize);
        }
        let span = (self.extent() - self.viewport).max(0.0);
        let physical = physical.clamp(0.0, span);
        let logical = (self.origin + physical).min((self.total - self.viewport).max(0.0));
        if self.total > self.limit && (physical < span * 0.2 || physical > span * 0.8) {
            self.seek(logical)
        } else {
            Ok(ScrollUpdate {
                logical,
                physical,
                rebased: false,
            })
        }
    }

    /// Preserve the old logical anchor when measurements or viewport change.
    pub fn resize(
        &mut self,
        total: f64,
        viewport: f64,
        logical: f64,
    ) -> Result<ScrollUpdate, Error> {
        let replacement = Self::new(total, viewport, self.limit)?;
        if !nonnegative(logical) {
            return Err(Error::InvalidSize);
        }
        let previous = self.origin;
        *self = replacement;
        let mut update = self.seek(logical)?;
        update.rebased = self.origin != previous;
        Ok(update)
    }
}
