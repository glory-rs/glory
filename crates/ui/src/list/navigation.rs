use glory_render::{Navigation, NodeId};

use super::{Error, Layout};

#[derive(Clone, Copy)]
pub(super) struct Request {
    pub from: NodeId,
    pub command: Navigation,
}

pub(super) struct Target<K> {
    pub key: K,
    pub index: usize,
    pub focus: bool,
    pub focus_sequence: u64,
}

/// Pages use box-layout pixels, with at least one row of progress. No iteration
/// over intervening rows is needed, even for Home/End on a huge source.
pub(super) fn target(
    layout: &Layout,
    current: Option<usize>,
    logical: f64,
    viewport: f64,
    command: Navigation,
) -> Result<Option<usize>, Error> {
    let len = layout.heights().len();
    if len == 0 || viewport <= 0.0 {
        return Ok(None);
    }
    if command == Navigation::Reveal {
        return Ok(current.filter(|index| *index < len));
    }
    let index = current
        .unwrap_or(layout.anchor(logical)?.expect("nonempty list").index)
        .min(len - 1);
    Ok(Some(match command {
        Navigation::First => 0,
        Navigation::Last => len - 1,
        Navigation::Previous => {
            if current.is_some() {
                index.saturating_sub(1)
            } else {
                index
            }
        }
        Navigation::Next => {
            if current.is_some() {
                index.saturating_add(1).min(len - 1)
            } else {
                index
            }
        }
        Navigation::PagePrevious => layout
            .anchor((layout.heights().prefix(index)? - viewport).max(0.0))?
            .expect("nonempty list")
            .index
            .min(index.saturating_sub(1)),
        Navigation::PageNext => layout
            .anchor(layout.heights().prefix(index)? + viewport)?
            .expect("nonempty list")
            .index
            .max(index.saturating_add(1).min(len - 1)),
        Navigation::Reveal => unreachable!("reveal requires a mounted row above"),
    }))
}

pub(super) fn reveal(
    layout: &Layout,
    index: usize,
    logical: f64,
    viewport: f64,
) -> Result<f64, Error> {
    let top = layout.heights().prefix(index)?;
    let bottom = layout.heights().prefix(index + 1)?;
    Ok(if top < logical || bottom - top > viewport {
        top
    } else if bottom > logical + viewport {
        bottom - viewport
    } else {
        logical
    })
}
