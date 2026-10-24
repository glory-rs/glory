//! Bounded host-owned sheet geometry. Fractions describe available viewport
//! extent, capped in logical pixels; no browser units or engine types cross here.
use crate::{Error, Layer, LayerPlacement};

pub const MAX_SHEET_SNAPS: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct SheetConfig {
    pub fractions: Vec<f64>,
    pub current: usize,
    pub max_extent: f64,
    pub settle_millis: u16,
    pub dismissible: bool,
    pub revision: u64,
}
impl SheetConfig {
    pub fn validate(&self) -> Result<(), Error> {
        if self.fractions.is_empty()
            || self.fractions.len() > MAX_SHEET_SNAPS
            || self.current >= self.fractions.len()
            || !self.max_extent.is_finite()
            || !(128.0..=4096.0).contains(&self.max_extent)
            || self.settle_millis > 500
            || self
                .fractions
                .iter()
                .any(|v| !v.is_finite() || *v <= 0.0 || *v > 1.0)
            || self.fractions.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(Error::InvalidProps("invalid bounded sheet configuration"));
        }
        Ok(())
    }
    /// Pure geometry shared by the host and tests. Tiny viewports remain finite;
    /// the host does not enforce an artificial minimum larger than the viewport.
    pub fn extent(&self, viewport: f64, index: usize) -> Result<f64, Error> {
        self.validate()?;
        if !viewport.is_finite() || viewport < 0.0 || index >= self.fractions.len() {
            return Err(Error::InvalidEvent("invalid sheet viewport or snap index"));
        }
        Ok(viewport.min(self.max_extent) * self.fractions[index])
    }
    pub fn nearest(&self, viewport: f64, extent: f64) -> Result<SheetCommand, Error> {
        self.validate()?;
        if !extent.is_finite() || !viewport.is_finite() || viewport < 0.0 {
            return Err(Error::InvalidEvent("invalid sheet release geometry"));
        }
        let base = viewport.min(self.max_extent);
        if self.dismissible && extent < base * self.fractions[0] * 0.5 {
            return Ok(SheetCommand::Dismiss);
        }
        let index = self
            .fractions
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (base * **a - extent)
                    .abs()
                    .total_cmp(&(base * **b - extent).abs())
            })
            .map(|(index, _)| index)
            .expect("validated nonempty snaps");
        Ok(SheetCommand::Snap(index))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetCommand {
    Snap(usize),
    Dismiss,
}

pub(crate) fn revision(previous: Option<&Layer>, next: Option<&Layer>) -> Result<(), Error> {
    let Some(old) = previous.and_then(|layer| layer.sheet.as_ref()) else {
        return Ok(());
    };
    let Some(layer) = next else {
        return Err(Error::InvalidProps(
            "a live sheet declaration cannot be removed",
        ));
    };
    let Some(new) = &layer.sheet else {
        return Err(Error::InvalidProps(
            "a live sheet declaration cannot be removed",
        ));
    };
    if new.revision < old.revision
        || new.revision == old.revision
            && (new != old || previous.map(|layer| layer.placement) != Some(layer.placement))
    {
        return Err(Error::SheetRevision {
            current: old.revision,
            incoming: new.revision,
        });
    }
    Ok(())
}

pub(crate) fn edge(placement: LayerPlacement) -> bool {
    matches!(
        placement,
        LayerPlacement::Left | LayerPlacement::Right | LayerPlacement::Top | LayerPlacement::Bottom
    )
}
