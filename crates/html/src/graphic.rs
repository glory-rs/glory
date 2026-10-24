use glory_render::{GraphicProps, PathCommand, StrokeCap};
use std::collections::BTreeMap;
use std::fmt::Write;

fn color(color: glory_render::style::Color) -> String {
    format!(
        "rgba({},{},{},{})",
        color.0,
        color.1,
        color.2,
        f64::from(color.3) / 255.0
    )
}
pub(crate) fn path(graphic: &GraphicProps) -> String {
    let mut value = String::new();
    for command in &graphic.commands {
        match command {
            PathCommand::Move(point) => write!(value, "M{} {}", point.x, point.y).unwrap(),
            PathCommand::Line(point) => write!(value, "L{} {}", point.x, point.y).unwrap(),
            PathCommand::Cubic { first, second, end } => write!(
                value,
                "C{} {} {} {} {} {}",
                first.x, first.y, second.x, second.y, end.x, end.y
            )
            .unwrap(),
            PathCommand::Close => value.push('Z'),
        }
    }
    value
}
pub(crate) fn main(graphic: &GraphicProps, name: Option<&str>) -> BTreeMap<&'static str, String> {
    let mut attributes = BTreeMap::from([
        (
            "viewBox",
            format!("0 0 {} {}", graphic.width, graphic.height),
        ),
        ("preserveAspectRatio", "xMidYMid meet".into()),
        ("overflow", "hidden".into()),
        ("width", graphic.width.to_string()),
        ("height", graphic.height.to_string()),
    ]);
    if let Some(alternative) = &graphic.alternative {
        attributes.insert("role", "img".into());
        attributes.insert("aria-label", name.unwrap_or(alternative).into());
    } else {
        attributes.insert("aria-hidden", "true".into());
    }
    attributes
}
pub(crate) fn content(graphic: &GraphicProps) -> BTreeMap<&'static str, String> {
    let mut attributes = BTreeMap::from([
        ("d", path(graphic)),
        ("fill", graphic.fill.map_or_else(|| "none".into(), color)),
        ("fill-rule", "nonzero".into()),
        (
            "stroke",
            graphic
                .stroke
                .map_or_else(|| "none".into(), |stroke| color(stroke.color)),
        ),
    ]);
    if let Some(stroke) = graphic.stroke {
        attributes.insert("stroke-width", stroke.width.to_string());
        attributes.insert(
            "stroke-linecap",
            match stroke.cap {
                StrokeCap::Butt => "butt",
                StrokeCap::Round => "round",
                StrokeCap::Square => "square",
            }
            .into(),
        );
        attributes.insert("stroke-linejoin", "round".into());
    }
    if let Some(transition) = graphic.transition {
        attributes.insert("data-gly-path-transition", String::new());
        attributes.insert(
            "style",
            format!("transition-duration:{}ms;", transition.duration_millis),
        );
    }
    if let Some(glory_render::GraphicAnimation::Rotate { period_millis }) = graphic.animation {
        attributes.insert("data-gly-rotate", String::new());
        attributes.insert("style", format!("animation-duration:{period_millis}ms;"));
    }
    attributes
}
