use glory_render::{Error, Layer, LayerPlacement, NodeId, Rect};
use serde_json::{Value, json};

/// Resolve the WebView's physical observations against the current declaration.
/// Requests identify one opening/geometry revision, so stale replies never move a new layer.
pub(super) fn position(
    node: NodeId,
    request: u64,
    layer: &Layer,
    anchor: [f64; 4],
    panel: [f64; 2],
    viewport: [f64; 2],
) -> Result<Option<Value>, Error> {
    let rect = |[width, height]: [f64; 2]| Rect {
        width,
        height,
        ..Rect::default()
    };
    let viewport = rect(viewport);
    let panel = rect(panel);
    let anchor = Rect {
        x: anchor[0],
        y: anchor[1],
        width: anchor[2],
        height: anchor[3],
    };
    let (x, y, arrow) = match layer.placement {
        LayerPlacement::Anchor(placement) => {
            let point = placement.resolve(anchor, panel, viewport)?;
            let arrow = placement.arrow_cross(anchor, panel, point).map(|cross| {
                json!({
                    "side": point.side.as_str(), "cross": cross,
                })
            });
            (point.x, point.y, arrow)
        }
        LayerPlacement::Viewport(placement) => {
            let point = placement.resolve(panel, viewport)?;
            (point.x, point.y, None)
        }
        LayerPlacement::ContextPoint(point) => {
            if [panel.width, panel.height, viewport.width, viewport.height]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0)
            {
                return Err(Error::InvalidEvent("invalid context layer bounds"));
            }
            (
                point
                    .x
                    .clamp(8., (viewport.width - panel.width - 8.).max(8.)),
                point
                    .y
                    .clamp(8., (viewport.height - panel.height - 8.).max(8.)),
                None,
            )
        }
        _ => return Ok(None),
    };
    Ok(Some(
        json!({"op":"layer_position","id":super::protocol::id(node),"request":request.to_string(),"x":x,"y":y,"arrow":arrow}),
    ))
}
