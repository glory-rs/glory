use super::{Error, ErrorKind, custom};
use crate::*;
use cssparser::{ParseError, Parser, Token};
type Result<T> = std::result::Result<T, ParseError<Error>>;

fn invalid<'i>(input: &Parser<'i>, expected: &str) -> ParseError<Error> {
    custom(input, ErrorKind::Value(expected.into()))
}
fn length<'i>(input: &mut Parser<'i>) -> Result<Length> {
    match input.next()?.clone() {
        Token::Ident(name) if name.eq_ignore_ascii_case("auto") => Ok(Length::Auto),
        Token::Percentage { unit_value, .. } => Ok(Length::Percent(f64::from(unit_value) * 100.0)),
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("px") => {
            Ok(Length::Px(value.into()))
        }
        Token::Number { value: 0.0, .. } => Ok(Length::Px(0.0)),
        _ => Err(invalid(input, "expected px, %, auto or unitless zero")),
    }
}
fn limit<'i>(input: &mut Parser<'i>) -> Result<Limit> {
    if input.try_parse(|p| p.expect_ident_matching("none")).is_ok() {
        return Ok(Limit::None);
    }
    match length(input)? {
        Length::Px(v) => Ok(Limit::Px(v)),
        Length::Percent(v) => Ok(Limit::Percent(v)),
        Length::Auto => Err(invalid(input, "max sizes do not accept auto")),
    }
}
fn px<'i>(input: &mut Parser<'i>) -> Result<f64> {
    match length(input)? {
        Length::Px(v) => Ok(v),
        _ => Err(invalid(input, "expected px or unitless zero")),
    }
}
fn edges<'i>(input: &mut Parser<'i>) -> Result<Edges> {
    let mut values = [0.0; 4];
    let mut n = 0;
    while !input.is_exhausted() {
        if n == 4 {
            return Err(invalid(input, "expected one to four px lengths"));
        }
        values[n] = px(input)?;
        n += 1;
    }
    Ok(match n {
        1 => Edges::all(values[0]),
        2 => Edges {
            top: values[0],
            right: values[1],
            bottom: values[0],
            left: values[1],
        },
        3 => Edges {
            top: values[0],
            right: values[1],
            bottom: values[2],
            left: values[1],
        },
        4 => Edges {
            top: values[0],
            right: values[1],
            bottom: values[2],
            left: values[3],
        },
        _ => return Err(invalid(input, "expected a length")),
    })
}
fn color<'i>(input: &mut Parser<'i>) -> Result<Color> {
    let token = input.next()?.clone();
    if let Token::Ident(name) = &token {
        return match name.to_ascii_lowercase().as_str() {
            "transparent" => Ok(Color(0, 0, 0, 0)),
            "black" => Ok(Color(0, 0, 0, 255)),
            "white" => Ok(Color(255, 255, 255, 255)),
            "red" => Ok(Color(255, 0, 0, 255)),
            "green" => Ok(Color(0, 128, 0, 255)),
            "blue" => Ok(Color(0, 0, 255, 255)),
            _ => Err(invalid(
                input,
                "expected hex color or a supported color keyword",
            )),
        };
    }
    let (Token::Hash(hex) | Token::IDHash(hex)) = token else {
        return Err(invalid(input, "expected hex color"));
    };
    let mut out = [0, 0, 0, 255];
    if !matches!(hex.len(), 3 | 4 | 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid(input, "invalid hex color"));
    }
    let mut digits = hex
        .chars()
        .map(|c| c.to_digit(16).expect("validated hex") as u8);
    let count = if hex.len() <= 4 {
        hex.len()
    } else {
        hex.len() / 2
    };
    for value in &mut out[..count] {
        let high = digits.next().expect("validated hex length");
        *value = high * 16
            + if hex.len() <= 4 {
                high
            } else {
                digits.next().expect("validated hex length")
            };
    }
    Ok(Color(out[0], out[1], out[2], out[3]))
}

fn box_shadow<'i>(input: &mut Parser<'i>) -> Result<BoxShadow> {
    if input.try_parse(|p| p.expect_ident_matching("none")).is_ok() {
        return Ok(BoxShadow::None);
    }
    let x = px(input)?;
    let y = px(input)?;
    let blur = input.try_parse(px).unwrap_or(0.0);
    let spread = input.try_parse(px).unwrap_or(0.0);
    let color = color(input)?;
    Ok(BoxShadow::Outset(Shadow {
        x,
        y,
        blur,
        spread,
        color,
    }))
}
macro_rules! keyword {
    ($input:expr, $($name:literal => $value:expr),+ $(,)?) => {{
        match $input.expect_ident()?.to_ascii_lowercase().as_str() {
            $($name => $value,)+ _ => return Err(invalid($input, "unsupported keyword")),
        }
    }};
}
fn overflow<'i>(input: &mut Parser<'i>) -> Result<Overflow> {
    Ok(
        keyword!(input, "visible"=>Overflow::Visible,"hidden"=>Overflow::Hidden,"clip"=>Overflow::Clip,"auto"=>Overflow::Auto,"scroll"=>Overflow::Scroll),
    )
}
fn families<'i>(input: &mut Parser<'i>) -> Result<Vec<Family>> {
    input.parse_comma_separated(|input| {
        if let Ok(name) = input.try_parse(|p| p.expect_string_cloned()) {
            input.expect_exhausted()?;
            return Ok(Family::Named(name.to_string()));
        }
        let first = input.expect_ident_cloned()?.to_string();
        let generic = match first.to_ascii_lowercase().as_str() {
            "serif" => Some(Family::Serif),
            "sans-serif" => Some(Family::Sans),
            "monospace" => Some(Family::Mono),
            "system-ui" => Some(Family::System),
            "cursive" => Some(Family::Cursive),
            "fantasy" => Some(Family::Fantasy),
            _ => None,
        };
        if let Some(family) = generic {
            input.expect_exhausted()?;
            return Ok(family);
        }
        let mut names = vec![first];
        while !input.is_exhausted() {
            names.push(input.expect_ident_cloned()?.to_string());
        }
        if names.iter().any(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                "initial"
                    | "inherit"
                    | "unset"
                    | "revert"
                    | "revert-layer"
                    | "default"
                    | "serif"
                    | "sans-serif"
                    | "monospace"
                    | "system-ui"
                    | "cursive"
                    | "fantasy"
                    | "ui-serif"
                    | "ui-sans-serif"
                    | "ui-monospace"
                    | "ui-rounded"
                    | "math"
                    | "fangsong"
            )
        }) {
            return Err(invalid(input, "unsupported or reserved font family"));
        }
        Ok(Family::Named(names.join(" ")))
    })
}
pub(super) fn parse<'i>(name: &str, input: &mut Parser<'i>) -> Result<Style> {
    let mut style = Style::new();
    match name {
        "width" => style.width = Some(length(input)?),
        "height" => style.height = Some(length(input)?),
        "min-width" => style.min_width = Some(length(input)?),
        "min-height" => style.min_height = Some(length(input)?),
        "max-width" => style.max_width = Some(limit(input)?),
        "max-height" => style.max_height = Some(limit(input)?),
        "top" => style.top = Some(length(input)?),
        "left" => style.left = Some(length(input)?),
        "gap" => style.gap = Some(px(input)?),
        "padding" => style.padding = Some(edges(input)?),
        "margin" => style.margin = Some(edges(input)?),
        "font-size" => style.font_size = Some(px(input)?),
        "line-height" => style.line_height = Some(px(input)?),
        "flex-grow" => style.grow = Some(input.expect_number()?.into()),
        "flex-shrink" => style.shrink = Some(input.expect_number()?.into()),
        "color" => style.color = Some(color(input)?),
        "background-color" => style.background = Some(color(input)?),
        "font-family" => style.font_family = Some(families(input)?),
        "font-weight" => {
            style.font_weight = Some(if let Ok(value) = input.try_parse(|p| p.expect_number()) {
                value.into()
            } else {
                keyword!(input, "normal"=>400.0, "bold"=>700.0)
            });
        }
        "field-sizing" => {
            style.field_sizing = Some(keyword!(input,
                "fixed" => FieldSizing::Fixed, "content" => FieldSizing::Content));
        }
        "white-space" => {
            style.white_space = Some(keyword!(input,
            "normal"=>WhiteSpace::Normal, "nowrap"=>WhiteSpace::NoWrap,
            "pre"=>WhiteSpace::Pre, "pre-wrap"=>WhiteSpace::PreWrap))
        }
        "border-width" => style.border_width = Some(edges(input)?),
        "border-color" => style.border_color = Some(color(input)?),
        "border-style" => {
            style.border_style =
                Some(keyword!(input,"none"=>BorderStyle::None,"solid"=>BorderStyle::Solid))
        }
        "border-radius" => style.radius = Some(px(input)?),
        "opacity" => style.opacity = Some(input.expect_number()?.into()),
        "box-shadow" => style.box_shadow = Some(box_shadow(input)?),
        "text-overflow" => {
            style.text_overflow =
                Some(keyword!(input,"clip"=>TextOverflow::Clip,"ellipsis"=>TextOverflow::Ellipsis))
        }
        "position" => {
            style.position = Some(
                keyword!(input,"static"=>Position::Static,"relative"=>Position::Relative,"absolute"=>Position::Absolute,"sticky"=>Position::Sticky),
            )
        }
        "display" => {
            style.display = Some(
                keyword!(input,"block"=>Display::Block,"flex"=>Display::Flex,"none"=>Display::None),
            )
        }
        "flex-direction" => {
            style.flex_direction =
                Some(keyword!(input,"row"=>FlexDirection::Row,"column"=>FlexDirection::Column))
        }
        "flex-wrap" => {
            style.flex_wrap =
                Some(keyword!(input,"nowrap"=>FlexWrap::NoWrap,"wrap"=>FlexWrap::Wrap))
        }
        "align-items" => {
            style.align = Some(
                keyword!(input,"flex-start"=>Align::Start,"start"=>Align::Start,"flex-end"=>Align::End,"end"=>Align::End,"center"=>Align::Center,"stretch"=>Align::Stretch),
            )
        }
        "justify-content" => {
            style.justify = Some(
                keyword!(input,"flex-start"=>Justify::Start,"start"=>Justify::Start,"flex-end"=>Justify::End,"end"=>Justify::End,"center"=>Justify::Center,"space-between"=>Justify::Between,"space-around"=>Justify::Around,"space-evenly"=>Justify::Evenly),
            )
        }
        "overflow-x" => style.overflow_x = Some(overflow(input)?),
        "overflow-y" => style.overflow_y = Some(overflow(input)?),
        "overflow" => {
            let x = overflow(input)?;
            let y = if input.is_exhausted() {
                x
            } else {
                overflow(input)?
            };
            style.overflow_x = Some(x);
            style.overflow_y = Some(y);
        }
        "overflow-anchor" => style.scroll_anchor = Some(keyword!(input,"auto"=>true,"none"=>false)),
        _ => return Err(custom(input, ErrorKind::Property(name.to_owned()))),
    }
    input.expect_exhausted()?;
    Ok(style)
}
