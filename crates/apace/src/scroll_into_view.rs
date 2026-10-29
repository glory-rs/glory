//! Shared physical reveal, independent of virtual-row or accessibility policy.
use super::*;

pub(super) fn reveal(state: &mut State, id: NodeId) -> Result<(), Error> {
    let tree = state.mapper.tree();
    tree.node(id)?;
    if !tree.active(id) || tree.inert(id) || !state.layers.allowed(tree, id) {
        return Err(Error::Unsupported(
            "scroll target is inactive or outside the current modal",
        ));
    }
    let boundary = state
        .layers
        .boundary(tree, id)
        .map(|id| state.dom.main(id))
        .transpose()?;
    reveal_within(state, id, boundary)
}

/// Reveal only through this live physical ancestor. It must never scroll an
/// enclosing document/layer when an option belongs to a local listbox.
pub(super) fn reveal_within(
    state: &mut State,
    id: NodeId,
    boundary: Option<EngineId>,
) -> Result<(), Error> {
    state.dom.resolve();
    let target = state.dom.main(id)?;
    if boundary == Some(target) {
        return Ok(());
    }
    if !state.dom.focus_visible(target) {
        return Ok(());
    }
    let viewport_before = state.dom.doc.viewport_scroll();
    let mut changed = false;
    let mut cursor = state.dom.doc.get_node(target).and_then(|n| n.parent);
    let mut inside_layer = false;
    while let Some(parent) = cursor {
        let host = state.dom.doc.get_node(parent).ok_or(Error::NodeGone(id))?;
        cursor = host.parent;
        if !host.has_boxes() {
            continue;
        }
        let (Some(mut rect), Some(mut outer)) = (
            state.dom.doc.get_client_bounding_rect(target),
            state.dom.doc.get_client_bounding_rect(parent),
        ) else {
            continue;
        };
        let own = *state
            .dom
            .doc
            .get_node(target)
            .ok_or(Error::NodeGone(id))?
            .scroll_offset();
        rect.x += own.x;
        rect.y += own.y;
        let before = *host.scroll_offset();
        let layout = host.final_layout();
        outer.x += before.x + f64::from(layout.border.left);
        outer.y += before.y + f64::from(layout.border.top);
        let width = f64::from(
            (layout.size.width
                - layout.border.left
                - layout.border.right
                - layout.scrollbar_size.width)
                .max(0.0),
        );
        let height = f64::from(
            (layout.size.height
                - layout.border.top
                - layout.border.bottom
                - layout.scrollbar_size.height)
                .max(0.0),
        );
        state.dom.doc.scroll_by(
            parent,
            nearest(rect.x - outer.x, rect.width, width),
            nearest(rect.y - outer.y, rect.height, height),
            ScrollBehavior::Instant,
        );
        changed |= before
            != *state
                .dom
                .doc
                .get_node(parent)
                .ok_or(Error::NodeGone(id))?
                .scroll_offset();
        if boundary == Some(parent) {
            inside_layer = true;
            break;
        }
    }
    if !inside_layer && boundary.is_none() {
        state.dom.doc.scroll_into_view(
            target,
            ScrollBehavior::Instant,
            apace_dom::ScrollLogicalPosition::Nearest,
            apace_dom::ScrollLogicalPosition::Nearest,
        );
    }
    if changed || viewport_before != state.dom.doc.viewport_scroll() {
        state.observation_pending = true;
        state.dom.invalidate();
    }
    Ok(())
}

pub(super) fn nearest(start: f64, size: f64, viewport: f64) -> f64 {
    let end = start + size;
    if (start >= 0.0 && end <= viewport) || (start <= 0.0 && end >= viewport) {
        0.0
    } else if start.abs() < (end - viewport).abs() {
        start
    } else {
        end - viewport
    }
}
