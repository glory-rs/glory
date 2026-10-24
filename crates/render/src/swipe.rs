//! Axis-intent swipe declarations. Completed gestures carry the captured revision.
use crate::{Error, Orientation};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwipeCapture {
    pub axis: Orientation,
    pub revision: u64,
}
impl SwipeCapture {
    pub fn axis_name(self) -> &'static str {
        match self.axis {
            Orientation::Horizontal => "horizontal",
            Orientation::Vertical => "vertical",
        }
    }
}
pub(crate) fn revision(old: Option<SwipeCapture>, next: Option<SwipeCapture>) -> Result<(), Error> {
    if let (Some(old), Some(next)) = (old, next) {
        if next.revision < old.revision {
            return Err(Error::InputRevision {
                current: old.revision,
                incoming: next.revision,
            });
        }
        if next.revision == old.revision && next.axis != old.axis {
            return Err(Error::InvalidProps(
                "equal swipe revision requires equal configuration",
            ));
        }
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub(crate) struct State {
    pub revision: u64,
}
impl State {
    pub fn new(capture: SwipeCapture) -> Box<Self> {
        Box::new(Self {
            revision: capture.revision,
        })
    }
}
