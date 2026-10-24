use glory_style::*;
use std::fmt::Write;

fn length(value: Length) -> String {
    match value {
        Length::Auto => "auto".into(),
        Length::Px(value) => format!("{value}px"),
        Length::Percent(value) => format!("{value}%"),
    }
}
fn limit(value: Limit) -> String {
    match value {
        Limit::None => "none".into(),
        Limit::Px(value) => format!("{value}px"),
        Limit::Percent(value) => format!("{value}%"),
    }
}
fn color(value: Color) -> String {
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        value.0, value.1, value.2, value.3
    )
}
// CSS strings also occur inside a <style> raw-text body. Hex-escape '<'
// regardless of quoting, so a face name cannot terminate that HTML element.
fn family(value: &Family) -> String {
    let Family::Named(name) = value else {
        return match value {
            Family::Serif => "serif",
            Family::Sans => "sans-serif",
            Family::Mono => "monospace",
            Family::System => "system-ui",
            Family::Cursive => "cursive",
            Family::Fantasy => "fantasy",
            Family::Named(_) => unreachable!(),
        }
        .into();
    };
    let mut out = String::from("\"");
    for c in name.chars() {
        match c {
            '\0' => out.push('\u{fffd}'),
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if c == '<' || c.is_control() => {
                write!(out, "\\{:x} ", c as u32).expect("string write");
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
pub fn style(style: &Style) -> String {
    let mut out = String::new();
    let mut declaration = |name: &str, value: String| {
        write!(out, "{name}:{value};").expect("string write");
    };
    if let Some(value) = style.pointer_events {
        declaration(
            "pointer-events",
            match value {
                PointerEvents::Auto => "auto",
                PointerEvents::None => "none",
            }
            .into(),
        );
    }
    for (name, value) in [
        ("grid-template-columns", style.grid_columns.as_ref()),
        ("grid-template-rows", style.grid_rows.as_ref()),
    ] {
        if let Some(value) = value {
            declaration(
                name,
                match value {
                    GridTracks::None => "none".into(),
                    GridTracks::Tracks(tracks) => tracks
                        .iter()
                        .map(|track| match track {
                            GridTrack::Auto => "auto".into(),
                            GridTrack::Px(value) => format!("{value}px"),
                            GridTrack::Percent(value) => format!("{value}%"),
                            GridTrack::Fr(value) => format!("{value}fr"),
                        })
                        .collect::<Vec<_>>()
                        .join(" "),
                },
            );
        }
    }
    for (name, value) in [
        ("grid-column", style.grid_column),
        ("grid-row", style.grid_row),
    ] {
        if let Some(value) = value {
            declaration(
                name,
                match value {
                    GridPlacement::Auto => "auto".into(),
                    GridPlacement::Span(span) => format!("span {span}"),
                    GridPlacement::At { start, span } => format!("{start} / span {span}"),
                },
            );
        }
    }
    if let Some(value) = style.object_fit {
        declaration(
            "object-fit",
            match value {
                ObjectFit::Fill => "fill",
                ObjectFit::Contain => "contain",
                ObjectFit::Cover => "cover",
                ObjectFit::None => "none",
                ObjectFit::ScaleDown => "scale-down",
            }
            .into(),
        );
    }
    if let Some(value) = style.aspect_ratio {
        declaration(
            "aspect-ratio",
            match value {
                AspectRatio::Auto => "auto".into(),
                AspectRatio::Ratio(value) => format!("{value}"),
            },
        );
    }
    if let Some(value) = style.flex_wrap {
        declaration(
            "flex-wrap",
            match value {
                FlexWrap::NoWrap => "nowrap",
                FlexWrap::Wrap => "wrap",
            }
            .into(),
        );
    }
    for (name, value) in [
        ("width", style.width),
        ("height", style.height),
        ("min-width", style.min_width),
        ("min-height", style.min_height),
    ] {
        if let Some(value) = value {
            declaration(name, length(value));
        }
    }
    for (name, value) in [
        ("max-width", style.max_width),
        ("max-height", style.max_height),
    ] {
        if let Some(value) = value {
            declaration(name, limit(value));
        }
    }
    for (name, value) in [
        ("top", style.top),
        ("left", style.left),
        ("right", style.right),
        ("bottom", style.bottom),
    ] {
        if let Some(value) = value {
            declaration(name, length(value));
        }
    }
    if let Some(value) = style.position {
        declaration(
            "position",
            match value {
                Position::Static => "static",
                Position::Relative => "relative",
                Position::Absolute => "absolute",
                Position::Sticky => "sticky",
                Position::Fixed => "fixed",
            }
            .into(),
        );
    }
    if let Some(value) = style.display {
        declaration(
            "display",
            match value {
                Display::Block => "block",
                Display::Flex => "flex",
                Display::InlineFlex => "inline-flex",
                Display::Grid => "grid",
                Display::None => "none",
            }
            .into(),
        );
    }
    if let Some(value) = style.flex_direction {
        declaration(
            "flex-direction",
            match value {
                FlexDirection::Row => "row",
                FlexDirection::Column => "column",
            }
            .into(),
        );
    }
    for (name, value) in [
        ("gap", style.gap),
        ("font-size", style.font_size),
        ("line-height", style.line_height),
        ("border-radius", style.radius),
    ] {
        if let Some(value) = value {
            declaration(name, format!("{value}px"));
        }
    }
    for (name, value) in [
        ("padding", style.padding),
        ("margin", style.margin),
        ("border-width", style.border_width),
    ] {
        if let Some(value) = value {
            declaration(
                name,
                format!(
                    "{}px {}px {}px {}px",
                    value.top, value.right, value.bottom, value.left
                ),
            );
        }
    }
    for (name, value) in [
        ("flex-grow", style.grow),
        ("flex-shrink", style.shrink),
        ("font-weight", style.font_weight),
        ("opacity", style.opacity),
    ] {
        if let Some(value) = value {
            declaration(name, value.to_string());
        }
    }
    for (name, value) in [
        ("color", style.color),
        ("background-color", style.background),
        ("border-color", style.border_color),
    ] {
        if let Some(value) = value {
            declaration(name, color(value));
        }
    }
    if let Some(value) = &style.font_family {
        declaration(
            "font-family",
            value.iter().map(family).collect::<Vec<_>>().join(","),
        );
    }
    if let Some(value) = style.field_sizing {
        declaration(
            "field-sizing",
            match value {
                FieldSizing::Fixed => "fixed",
                FieldSizing::Content => "content",
            }
            .into(),
        );
    }
    if let Some(value) = style.text_overflow {
        declaration(
            "text-overflow",
            match value {
                TextOverflow::Clip => "clip",
                TextOverflow::Ellipsis => "ellipsis",
            }
            .into(),
        );
    }
    if let Some(value) = style.white_space {
        declaration(
            "white-space",
            match value {
                WhiteSpace::Normal => "normal",
                WhiteSpace::NoWrap => "nowrap",
                WhiteSpace::Pre => "pre",
                WhiteSpace::PreWrap => "pre-wrap",
            }
            .into(),
        );
    }
    if let Some(value) = style.border_style {
        declaration(
            "border-style",
            match value {
                BorderStyle::None => "none",
                BorderStyle::Solid => "solid",
            }
            .into(),
        );
    }
    for (name, value) in [
        ("overflow-x", style.overflow_x),
        ("overflow-y", style.overflow_y),
    ] {
        if let Some(value) = value {
            declaration(
                name,
                match value {
                    Overflow::Visible => "visible",
                    Overflow::Hidden => "hidden",
                    Overflow::Clip => "clip",
                    Overflow::Auto => "auto",
                    Overflow::Scroll => "scroll",
                }
                .into(),
            );
        }
    }
    if let Some(value) = style.align {
        declaration(
            "align-items",
            match value {
                Align::Start => "flex-start",
                Align::End => "flex-end",
                Align::Center => "center",
                Align::Stretch => "stretch",
            }
            .into(),
        );
    }
    if let Some(value) = style.scroll_anchor {
        declaration(
            "overflow-anchor",
            if value { "auto" } else { "none" }.into(),
        );
    }
    if let Some(value) = style.justify {
        declaration(
            "justify-content",
            match value {
                Justify::Start => "flex-start",
                Justify::End => "flex-end",
                Justify::Center => "center",
                Justify::Between => "space-between",
                Justify::Around => "space-around",
                Justify::Evenly => "space-evenly",
            }
            .into(),
        );
    }
    if let Some(shadow) = style.box_shadow {
        declaration(
            "box-shadow",
            match shadow {
                BoxShadow::None => "none".into(),
                BoxShadow::Outset(shadow) => format!(
                    "{}px {}px {}px {}px {}",
                    shadow.x,
                    shadow.y,
                    shadow.blur,
                    shadow.spread,
                    color(shadow.color)
                ),
            },
        );
    }
    out
}
