//! One mounted box, nearest alignment, current physical ancestry only.
use super::*;

fn nearest(start: f64, size: f64, viewport: f64) -> f64 {
    let end = start + size;
    if (start >= 0. && end <= viewport) || (start <= 0. && end >= viewport) {
        0.
    } else if start.abs() < (end - viewport).abs() {
        start
    } else {
        end - viewport
    }
}
pub(super) fn reveal(dom: &Dom, node: NodeId) -> Result<(), Error> {
    let target = dom.entry(node)?.element()?;
    if !target.is_connected()
        || target
            .closest("[hidden],[inert]")
            .map_err(backend)?
            .is_some()
        || !dom.document_layers.allows(Some(target.as_ref()))
    {
        return Err(Error::Unsupported(
            "scroll target is inactive or outside the current modal",
        ));
    }
    let layer = target
        .closest("[data-gly-layer]:popover-open")
        .map_err(backend)?;
    if let Some(layer) = layer {
        if target == layer {
            return Ok(());
        }
        let mut cursor = target.parent_element();
        while let Some(parent) = cursor {
            let rect = target.get_bounding_client_rect();
            let outer = parent.get_bounding_client_rect();
            let dx = nearest(
                rect.x() - outer.x() - f64::from(parent.client_left()),
                rect.width(),
                f64::from(parent.client_width()),
            );
            let dy = nearest(
                rect.y() - outer.y() - f64::from(parent.client_top()),
                rect.height(),
                f64::from(parent.client_height()),
            );
            js_sys::Reflect::set(
                &parent,
                &"scrollLeft".into(),
                &(dom::number(&parent, "scrollLeft") + dx).into(),
            )
            .map_err(backend)?;
            js_sys::Reflect::set(
                &parent,
                &"scrollTop".into(),
                &(dom::number(&parent, "scrollTop") + dy).into(),
            )
            .map_err(backend)?;
            if parent == layer {
                break;
            }
            cursor = parent.parent_element();
        }
    } else {
        let options = js_sys::Object::new();
        for (key, value) in [
            ("block", "nearest"),
            ("inline", "nearest"),
            ("behavior", "instant"),
        ] {
            js_sys::Reflect::set(&options, &key.into(), &value.into()).map_err(backend)?;
        }
        let function = js_sys::Reflect::get(&target, &"scrollIntoView".into())
            .map_err(backend)?
            .dyn_into::<js_sys::Function>()
            .map_err(|_| Error::Unsupported("element scrollIntoView"))?;
        function.call1(&target, &options).map_err(backend)?;
    }
    dom.frame.input();
    Ok(())
}
