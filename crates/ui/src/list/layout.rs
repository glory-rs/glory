use std::collections::HashMap;
use std::ops::Range;

use super::{Error, nonnegative, positive, valid_extent};

/// Fixed heights allocate no per-item storage. Variable heights use a Fenwick
/// tree: one measurement or a prefix/offset lookup costs O(log n), not O(n).
#[derive(Debug)]
pub struct Heights {
    mode: HeightMode,
}

#[derive(Debug)]
enum HeightMode {
    Fixed {
        len: usize,
        height: f64,
    },
    Variable {
        values: Vec<f64>,
        tree: Vec<f64>,
    },
    Sparse {
        len: usize,
        estimate: f64,
        values: HashMap<usize, f64>,
        tree: HashMap<usize, Deviation>,
    },
}
#[derive(Clone, Copy, Debug, Default)]
struct Deviation {
    sum: f64,
    count: usize,
}

impl Heights {
    pub fn fixed(len: usize, height: f64) -> Result<Self, Error> {
        if !positive(height) || !valid_extent(len as f64 * height) {
            return Err(Error::InvalidSize);
        }
        Ok(Self {
            mode: HeightMode::Fixed { len, height },
        })
    }

    pub fn variable(len: usize, estimate: f64) -> Result<Self, Error> {
        if !positive(estimate) || !valid_extent(len as f64 * estimate) {
            return Err(Error::InvalidSize);
        }
        let mut values = Vec::new();
        let mut tree = Vec::new();
        values
            .try_reserve_exact(len)
            .map_err(|_| Error::Allocation)?;
        tree.try_reserve_exact(len).map_err(|_| Error::Allocation)?;
        values.resize(len, estimate);
        // Uniform initialization is O(n), rather than n separate point updates.
        tree.extend((1..=len).map(|i| (i & i.wrapping_neg()) as f64 * estimate));
        Ok(Self {
            mode: HeightMode::Variable { values, tree },
        })
    }

    /// Uniform estimates need no per-item allocation. Only measured deviations
    /// and their Fenwick ancestors are stored. A widget can bound retained
    /// measurements by resetting old ones to `estimate` with `measure`.
    pub fn sparse(len: usize, estimate: f64) -> Result<Self, Error> {
        if !positive(estimate) || !valid_extent(len as f64 * estimate) {
            return Err(Error::InvalidSize);
        }
        Ok(Self {
            mode: HeightMode::Sparse {
                len,
                estimate,
                values: HashMap::new(),
                tree: HashMap::new(),
            },
        })
    }

    pub fn len(&self) -> usize {
        match &self.mode {
            HeightMode::Fixed { len, .. } => *len,
            HeightMode::Variable { values, .. } => values.len(),
            HeightMode::Sparse { len, .. } => *len,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn storage_len(&self) -> usize {
        match &self.mode {
            HeightMode::Fixed { .. } => 0,
            HeightMode::Variable { values, tree } => values.len() + tree.len(),
            HeightMode::Sparse { values, tree, .. } => values.len() + tree.len(),
        }
    }
    /// Reserved entries, for detecting retained allocation after long scrolling.
    /// HashMap capacities are entry counts, not byte or bucket-size estimates.
    pub fn storage_capacity(&self) -> usize {
        match &self.mode {
            HeightMode::Fixed { .. } => 0,
            HeightMode::Variable { values, tree } => values.capacity() + tree.capacity(),
            HeightMode::Sparse { values, tree, .. } => values.capacity() + tree.capacity(),
        }
    }

    pub fn height(&self, index: usize) -> Option<f64> {
        match &self.mode {
            HeightMode::Fixed { len, height } => (index < *len).then_some(*height),
            HeightMode::Variable { values, .. } => values.get(index).copied(),
            HeightMode::Sparse {
                len,
                estimate,
                values,
                ..
            } => (index < *len).then(|| values.get(&index).copied().unwrap_or(*estimate)),
        }
    }

    /// Offset of the boundary preceding `count`, with count==len at the end.
    pub fn prefix(&self, count: usize) -> Result<f64, Error> {
        if count > self.len() {
            return Err(Error::OutOfBounds);
        }
        Ok(self.prefix_unchecked(count))
    }

    fn prefix_unchecked(&self, mut count: usize) -> f64 {
        match &self.mode {
            HeightMode::Fixed { height, .. } => count as f64 * height,
            HeightMode::Variable { tree, .. } => {
                let mut sum = 0.0;
                while count > 0 {
                    sum += tree[count - 1];
                    count &= count - 1;
                }
                sum
            }
            HeightMode::Sparse { estimate, tree, .. } => {
                let mut sum = count as f64 * estimate;
                while count > 0 {
                    sum += tree.get(&count).map_or(0.0, |node| node.sum);
                    count &= count - 1;
                }
                sum
            }
        }
    }

    pub fn total(&self) -> f64 {
        self.prefix_unchecked(self.len())
    }

    /// Number of complete rows ending at or before the offset.
    fn complete_before(&self, offset: f64) -> usize {
        match &self.mode {
            HeightMode::Fixed { len, height } => ((offset / height).floor() as usize).min(*len),
            HeightMode::Variable { tree, .. } => {
                let mut count = 0;
                let mut sum = 0.0;
                let mut bit = if tree.is_empty() {
                    0
                } else {
                    1usize << tree.len().ilog2()
                };
                while bit != 0 {
                    let next = count + bit;
                    if next <= tree.len() && sum + tree[next - 1] <= offset {
                        count = next;
                        sum += tree[next - 1];
                    }
                    bit >>= 1;
                }
                count
            }
            HeightMode::Sparse {
                len,
                estimate,
                tree,
                ..
            } => {
                let mut count = 0_usize;
                let mut sum = 0.0;
                let mut bit = if *len == 0 { 0 } else { 1usize << len.ilog2() };
                while bit != 0 {
                    if let Some(next) = count.checked_add(bit).filter(|next| *next <= *len) {
                        let block = (next & next.wrapping_neg()) as f64 * estimate
                            + tree.get(&next).map_or(0.0, |node| node.sum);
                        if sum + block <= offset {
                            count = next;
                            sum += block;
                        }
                    }
                    bit >>= 1;
                }
                count
            }
        }
    }

    /// Zero-height entries must be filtered from the source rather than creating
    /// an unbounded number of "visible" rows. Measurements must be positive.
    pub fn measure(&mut self, index: usize, height: f64) -> Result<bool, Error> {
        if !positive(height) {
            return Err(Error::InvalidSize);
        }
        let total = self.total();
        match &mut self.mode {
            HeightMode::Fixed { .. } => Err(Error::InvalidSize),
            HeightMode::Variable { values, tree } => {
                let value = values.get_mut(index).ok_or(Error::OutOfBounds)?;
                let delta = height - *value;
                if delta == 0.0 {
                    return Ok(false);
                }
                if !valid_extent(total + delta) {
                    return Err(Error::InvalidSize);
                }
                *value = height;
                let mut i = index + 1;
                while i <= tree.len() {
                    tree[i - 1] += delta;
                    let Some(next) = i.checked_add(i & i.wrapping_neg()) else {
                        break;
                    };
                    i = next;
                }
                Ok(true)
            }
            HeightMode::Sparse {
                len,
                estimate,
                values,
                tree,
            } => {
                if index >= *len {
                    return Err(Error::OutOfBounds);
                }
                let delta = height - values.get(&index).copied().unwrap_or(*estimate);
                if delta == 0.0 {
                    return Ok(false);
                }
                if !valid_extent(total + delta) {
                    return Err(Error::InvalidSize);
                }
                let had = usize::from(values.contains_key(&index));
                let has = usize::from(height != *estimate);
                if has == 0 {
                    values.remove(&index);
                } else {
                    values.insert(index, height);
                }
                let mut i = index + 1;
                while i <= *len {
                    let previous = tree.get(&i).copied().unwrap_or_default();
                    let count = previous.count + has - had;
                    if count == 0 {
                        tree.remove(&i);
                    } else {
                        tree.insert(
                            i,
                            Deviation {
                                sum: previous.sum + delta,
                                count,
                            },
                        );
                    }
                    let Some(next) = i.checked_add(i & i.wrapping_neg()) else {
                        break;
                    };
                    i = next;
                }
                Ok(true)
            }
        }
    }

    /// Explicit data edits, never called for a scroll. Fixed-size edits are
    /// O(1); variable-size edits rebuild O(n), preserving unaffected measures.
    pub fn splice(
        &mut self,
        range: Range<usize>,
        inserted: usize,
        estimate: f64,
    ) -> Result<(), Error> {
        if range.start > range.end || range.end > self.len() {
            return Err(Error::InvalidRange);
        }
        if !positive(estimate) {
            return Err(Error::InvalidSize);
        }
        let len = self
            .len()
            .checked_sub(range.len())
            .and_then(|len| len.checked_add(inserted))
            .ok_or(Error::InvalidSize)?;
        match &mut self.mode {
            HeightMode::Fixed {
                len: current,
                height,
            } => {
                if !valid_extent(len as f64 * *height) {
                    return Err(Error::InvalidSize);
                }
                *current = len;
            }
            HeightMode::Variable { values, tree } => {
                let total = values[..range.start]
                    .iter()
                    .chain(&values[range.end..])
                    .sum::<f64>()
                    + inserted as f64 * estimate;
                if !valid_extent(total) {
                    return Err(Error::InvalidSize);
                }
                values
                    .try_reserve(len.saturating_sub(values.len()))
                    .map_err(|_| Error::Allocation)?;
                tree.try_reserve(len.saturating_sub(tree.len()))
                    .map_err(|_| Error::Allocation)?;
                values.splice(range, std::iter::repeat_n(estimate, inserted));
                tree.clear();
                tree.extend_from_slice(values);
                for i in 1..=len {
                    if let Some(parent) = i
                        .checked_add(i & i.wrapping_neg())
                        .filter(|parent| *parent <= len)
                    {
                        tree[parent - 1] += tree[i - 1];
                    }
                }
            }
            HeightMode::Sparse {
                estimate: base,
                values,
                ..
            } => {
                // Explicit edits cost O(retained measurements * log n), never
                // O(total rows). An inserted range must use the uniform base;
                // callers with individual known heights can measure them next.
                if inserted > 0 && estimate != *base {
                    return Err(Error::InvalidSize);
                }
                let mut next = Self::sparse(len, *base)?;
                for (&index, &height) in values.iter() {
                    if index < range.start {
                        next.measure(index, height)?;
                    } else if index >= range.end {
                        next.measure(index - (range.end - range.start) + inserted, height)?;
                    }
                }
                *self = next;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub index: usize,
    pub within: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Window {
    pub visible: Range<usize>,
    pub render: Range<usize>,
    pub offset: f64,
    pub total: f64,
    pub top: f64,
    pub bottom: f64,
}

#[derive(Debug)]
pub struct Layout {
    heights: Heights,
    overscan: usize,
}

impl Layout {
    pub fn new(heights: Heights, overscan: usize) -> Self {
        Self { heights, overscan }
    }
    pub fn heights(&self) -> &Heights {
        &self.heights
    }
    pub fn heights_mut(&mut self) -> &mut Heights {
        &mut self.heights
    }

    pub fn window(&self, offset: f64, viewport: f64) -> Result<Window, Error> {
        if !nonnegative(offset) || !nonnegative(viewport) {
            return Err(Error::InvalidSize);
        }
        let len = self.heights.len();
        let total = self.heights.total();
        let offset = offset.min((total - viewport).max(0.0));
        if len == 0 || viewport == 0.0 {
            return Ok(Window {
                offset,
                total,
                bottom: total,
                ..Window::default()
            });
        }
        let start = self.heights.complete_before(offset).min(len);
        let end_offset = (offset + viewport).min(total);
        let complete = self.heights.complete_before(end_offset);
        let end =
            (complete + usize::from(self.heights.prefix_unchecked(complete) < end_offset)).min(len);
        let range = start.saturating_sub(self.overscan)..end.saturating_add(self.overscan).min(len);
        let top = self.heights.prefix_unchecked(range.start);
        let bottom = (total - self.heights.prefix_unchecked(range.end)).max(0.0);
        Ok(Window {
            visible: start..end,
            render: range,
            offset,
            total,
            top,
            bottom,
        })
    }

    pub fn anchor(&self, offset: f64) -> Result<Option<Anchor>, Error> {
        if !nonnegative(offset) {
            return Err(Error::InvalidSize);
        }
        if self.heights.is_empty() {
            return Ok(None);
        }
        let index = self
            .heights
            .complete_before(offset)
            .min(self.heights.len() - 1);
        Ok(Some(Anchor {
            index,
            within: (offset - self.heights.prefix_unchecked(index))
                .clamp(0.0, self.heights.height(index).expect("known row")),
        }))
    }

    /// For prepend/reorder, the data adapter first resolves the anchor's stable
    /// key to its new index. No full data scan is performed during scrolling.
    pub fn restore(&self, anchor: Anchor) -> Result<f64, Error> {
        if !nonnegative(anchor.within) {
            return Err(Error::InvalidSize);
        }
        let height = self
            .heights
            .height(anchor.index)
            .ok_or(Error::OutOfBounds)?;
        Ok(self.heights.prefix_unchecked(anchor.index) + anchor.within.min(height))
    }

    pub fn measure(
        &mut self,
        index: usize,
        height: f64,
        anchor: Option<Anchor>,
    ) -> Result<(bool, Option<f64>), Error> {
        if let Some(anchor) = anchor {
            self.restore(anchor)?;
        }
        let changed = self.heights.measure(index, height)?;
        Ok((
            changed,
            anchor.map(|anchor| self.restore(anchor)).transpose()?,
        ))
    }

    /// Preserve the item under the viewport across a positional data splice.
    /// If the anchored item is removed, choose the next surviving row, or the
    /// final row when removing the tail. Arbitrary reorder uses stable keys.
    pub fn splice(
        &mut self,
        range: Range<usize>,
        inserted: usize,
        estimate: f64,
        anchor: Option<Anchor>,
    ) -> Result<Option<f64>, Error> {
        if let Some(anchor) = anchor {
            self.restore(anchor)?;
        }
        let removed = range
            .end
            .checked_sub(range.start)
            .ok_or(Error::InvalidRange)?;
        self.heights.splice(range.clone(), inserted, estimate)?;
        let Some(anchor) = anchor else {
            return Ok(None);
        };
        if self.heights.is_empty() {
            return Ok(None);
        }
        let next = if anchor.index < range.start {
            anchor
        } else if anchor.index >= range.end {
            Anchor {
                index: anchor.index - removed + inserted,
                ..anchor
            }
        } else {
            Anchor {
                index: range.start.min(self.heights.len() - 1),
                within: 0.0,
            }
        };
        self.restore(next).map(Some)
    }
}
