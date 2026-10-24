//! Style values shared by semantic UI and rendering adapters.
//!
//! This crate has no reactive runtime, CSS engine, window or platform dependencies.
//! CSS cascade and geometry belong to the selected engine, not to these values.

mod grid;
mod profile;
pub use grid::{
    AspectRatio, GridPlacement, GridTrack, GridTracks, MAX_ASPECT_RATIO, MAX_GRID_TRACKS,
    MIN_ASPECT_RATIO,
};
pub use profile::{Css, Profile, Property, Support, Unsupported, Value};
#[cfg(feature = "css")]
pub mod sheet;
#[cfg(feature = "css")]
pub use sheet::Sheet;

/// A logical CSS length. Coordinates use f64 so very long lists do not lose
/// individual pixel increments before reaching the platform adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
    #[default]
    Auto,
    Px(f64),
    Percent(f64),
    /// Advance of the current font's zero glyph, resolved by the host font engine.
    Ch(f64),
}

impl Length {
    pub fn is_finite(self) -> bool {
        match self {
            Self::Auto => true,
            Self::Px(value) | Self::Percent(value) | Self::Ch(value) => value.is_finite(),
        }
    }

    fn is_size(self) -> bool {
        self.is_finite()
            && match self {
                Self::Auto => true,
                Self::Px(value) | Self::Percent(value) | Self::Ch(value) => value >= 0.0,
            }
    }
}

/// A maximum box size. `None` explicitly removes a stylesheet's upper bound;
/// an absent `Style::max_width`/`max_height` leaves the stylesheet in charge.
/// Maximum sizes do not accept the `auto` keyword used by preferred/minimum
/// sizes. Keep that distinction in Rust instead of emitting ignored CSS.
///
/// ```
/// use glory_style::{Limit, Style};
/// let style = Style::new().max_width(Limit::None).max_height(Limit::Px(400.0));
/// assert!(style.validate().is_ok());
/// ```
/// ```compile_fail
/// use glory_style::{Length, Style};
/// let style = Style::new().max_width(Length::Auto);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Limit {
    #[default]
    None,
    Px(f64),
    Percent(f64),
}

impl Limit {
    pub fn is_finite(self) -> bool {
        match self {
            Self::None => true,
            Self::Px(value) | Self::Percent(value) => value.is_finite(),
        }
    }

    fn is_size(self) -> bool {
        self.is_finite()
            && match self {
                Self::None => true,
                Self::Px(value) | Self::Percent(value) => value >= 0.0,
            }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Display {
    #[default]
    Block,
    Flex,
    InlineFlex,
    Grid,
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    /// Clip overflowing content without creating an inner scroll container.
    Clip,
    Auto,
    Scroll,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
    Sticky,
    Fixed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    Start,
    End,
    Center,
    #[default]
    Stretch,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    End,
    Center,
    Between,
    Around,
    Evenly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Edges {
    pub const fn all(value: f64) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    fn is_finite(self) -> bool {
        [self.top, self.right, self.bottom, self.left]
            .into_iter()
            .all(f64::is_finite)
    }

    fn is_nonnegative(self) -> bool {
        self.is_finite()
            && [self.top, self.right, self.bottom, self.left]
                .into_iter()
                .all(|v| v >= 0.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

/// One finite outset shadow, expressed in CSS pixels. Insets, multiple layers
/// and currentColor are outside this explicit portable declaration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub x: f64,
    pub y: f64,
    pub blur: f64,
    pub spread: f64,
    pub color: Color,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum BoxShadow {
    /// Explicitly remove an author stylesheet's shadow.
    #[default]
    None,
    Outset(Shadow),
}

/// A named face is always serialized as a CSS string, distinct from a generic
/// family (including a face literally named "serif"). Resolution is host-owned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Family {
    Named(String),
    Serif,
    Sans,
    Mono,
    System,
    Cursive,
    Fantasy,
}

/// Whitespace collapsing and line wrapping, independently of word breaking.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WhiteSpace {
    #[default]
    Normal,
    NoWrap,
    Pre,
    PreWrap,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ObjectFit {
    #[default]
    Fill,
    Contain,
    Cover,
    None,
    ScaleDown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextOverflow {
    #[default]
    Clip,
    Ellipsis,
}

/// Physical pointer targeting. This does not disable focus or keyboard input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointerEvents {
    #[default]
    Auto,
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BorderStyle {
    #[default]
    None,
    Solid,
}

/// Intrinsic control sizing. Content sizing requires host support; native
/// adapters must reject it until their editors participate in intrinsic layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FieldSizing {
    #[default]
    Fixed,
    Content,
}

/// The portable v1 style subset. An adapter diagnoses unsupported features;
/// full HTML stylesheets are a separate resource and are not restricted to v1.
/// None means "no inline declaration", distinct from explicit `auto`/`block`.
/// An empty style must not override a stylesheet or turn an inline Text into a
/// block. Engine defaults and Glory's baseline stylesheet supply initial values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    pub position: Option<Position>,
    pub top: Option<Length>,
    pub left: Option<Length>,
    pub right: Option<Length>,
    pub bottom: Option<Length>,
    pub display: Option<Display>,
    pub grid_columns: Option<GridTracks>,
    pub grid_rows: Option<GridTracks>,
    pub grid_column: Option<GridPlacement>,
    pub grid_row: Option<GridPlacement>,
    pub aspect_ratio: Option<AspectRatio>,
    /// Replaced image content sizing within its existing box; centered position.
    pub object_fit: Option<ObjectFit>,
    pub pointer_events: Option<PointerEvents>,
    pub flex_direction: Option<FlexDirection>,
    pub flex_wrap: Option<FlexWrap>,
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub min_width: Option<Length>,
    pub min_height: Option<Length>,
    pub max_width: Option<Limit>,
    pub max_height: Option<Limit>,
    pub gap: Option<f64>,
    pub padding: Option<Edges>,
    pub margin: Option<Edges>,
    pub grow: Option<f64>,
    pub shrink: Option<f64>,
    pub align: Option<Align>,
    pub justify: Option<Justify>,
    pub overflow_x: Option<Overflow>,
    pub overflow_y: Option<Overflow>,
    /// Automatic engine scroll anchoring. Virtual lists manage their own anchor.
    pub scroll_anchor: Option<bool>,
    pub color: Option<Color>,
    pub background: Option<Color>,
    pub font_size: Option<f64>,
    pub line_height: Option<f64>,
    /// Ordered fallback list. An empty explicit list is invalid.
    pub font_family: Option<Vec<Family>>,
    /// Absolute CSS weight in [1, 1000], including variable-font fractions.
    pub font_weight: Option<f64>,
    pub white_space: Option<WhiteSpace>,
    pub text_overflow: Option<TextOverflow>,
    pub field_sizing: Option<FieldSizing>,
    pub border_width: Option<Edges>,
    pub border_color: Option<Color>,
    pub border_style: Option<BorderStyle>,
    /// Uniform circular corners in CSS pixels.
    pub radius: Option<f64>,
    /// Group opacity in [0, 1], applied by the engine after compositing children.
    pub opacity: Option<f64>,
    pub box_shadow: Option<BoxShadow>,
}

impl Style {
    pub fn pointer_events(mut self, value: PointerEvents) -> Self {
        self.pointer_events = Some(value);
        self
    }
    pub fn grid_columns(mut self, value: GridTracks) -> Self {
        self.grid_columns = Some(value);
        self
    }
    pub fn grid_rows(mut self, value: GridTracks) -> Self {
        self.grid_rows = Some(value);
        self
    }
    pub fn grid_column(mut self, value: GridPlacement) -> Self {
        self.grid_column = Some(value);
        self
    }
    pub fn grid_row(mut self, value: GridPlacement) -> Self {
        self.grid_row = Some(value);
        self
    }
    pub fn object_fit(mut self, value: ObjectFit) -> Self {
        self.object_fit = Some(value);
        self
    }
    pub fn aspect_ratio(mut self, value: AspectRatio) -> Self {
        self.aspect_ratio = Some(value);
        self
    }
    pub fn text_overflow(mut self, value: TextOverflow) -> Self {
        self.text_overflow = Some(value);
        self
    }
    pub fn flex_wrap(mut self, value: FlexWrap) -> Self {
        self.flex_wrap = Some(value);
        self
    }
    pub fn box_shadow(mut self, value: BoxShadow) -> Self {
        self.box_shadow = Some(value);
        self
    }
    pub fn field_sizing(mut self, value: FieldSizing) -> Self {
        self.field_sizing = Some(value);
        self
    }
    pub fn position(mut self, value: Position) -> Self {
        self.position = Some(value);
        self
    }
    pub fn top(mut self, value: Length) -> Self {
        self.top = Some(value);
        self
    }
    pub fn left(mut self, value: Length) -> Self {
        self.left = Some(value);
        self
    }
    pub fn right(mut self, value: Length) -> Self {
        self.right = Some(value);
        self
    }
    pub fn bottom(mut self, value: Length) -> Self {
        self.bottom = Some(value);
        self
    }
    pub fn new() -> Self {
        Self::default()
    }

    pub fn display(mut self, value: Display) -> Self {
        self.display = Some(value);
        self
    }
    pub fn flex_direction(mut self, value: FlexDirection) -> Self {
        self.flex_direction = Some(value);
        self
    }
    pub fn width(mut self, value: Length) -> Self {
        self.width = Some(value);
        self
    }
    pub fn height(mut self, value: Length) -> Self {
        self.height = Some(value);
        self
    }
    pub fn min_width(mut self, value: Length) -> Self {
        self.min_width = Some(value);
        self
    }
    pub fn min_height(mut self, value: Length) -> Self {
        self.min_height = Some(value);
        self
    }
    pub fn max_width(mut self, value: Limit) -> Self {
        self.max_width = Some(value);
        self
    }
    pub fn max_height(mut self, value: Limit) -> Self {
        self.max_height = Some(value);
        self
    }
    pub fn gap(mut self, value: f64) -> Self {
        self.gap = Some(value);
        self
    }
    pub fn padding(mut self, value: Edges) -> Self {
        self.padding = Some(value);
        self
    }
    pub fn margin(mut self, value: Edges) -> Self {
        self.margin = Some(value);
        self
    }
    pub fn overflow_x(mut self, value: Overflow) -> Self {
        self.overflow_x = Some(value);
        self
    }
    pub fn overflow_y(mut self, value: Overflow) -> Self {
        self.overflow_y = Some(value);
        self
    }
    pub fn scroll_anchor(mut self, value: bool) -> Self {
        self.scroll_anchor = Some(value);
        self
    }

    pub fn font_family(mut self, value: impl IntoIterator<Item = Family>) -> Self {
        self.font_family = Some(value.into_iter().collect());
        self
    }
    pub fn font_weight(mut self, value: f64) -> Self {
        self.font_weight = Some(value);
        self
    }
    pub fn white_space(mut self, value: WhiteSpace) -> Self {
        self.white_space = Some(value);
        self
    }
    pub fn border_width(mut self, value: Edges) -> Self {
        self.border_width = Some(value);
        self
    }
    pub fn border_color(mut self, value: Color) -> Self {
        self.border_color = Some(value);
        self
    }
    pub fn border_style(mut self, value: BorderStyle) -> Self {
        self.border_style = Some(value);
        self
    }
    pub fn radius(mut self, value: f64) -> Self {
        self.radius = Some(value);
        self
    }
    pub fn opacity(mut self, value: f64) -> Self {
        self.opacity = Some(value);
        self
    }

    /// Reject non-finite and invalid geometry at the semantic boundary instead
    /// of letting one engine serialize NaN and another panic inside layout.
    pub fn validate(&self) -> Result<(), &'static str> {
        for tracks in [self.grid_columns.as_ref(), self.grid_rows.as_ref()]
            .into_iter()
            .flatten()
        {
            tracks.validate()?;
        }
        for placement in [self.grid_column, self.grid_row].into_iter().flatten() {
            placement.validate()?;
        }
        if let Some(ratio) = self.aspect_ratio {
            ratio.validate()?;
        }
        if [self.top, self.left, self.right, self.bottom]
            .into_iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err("position offsets must be finite");
        }
        if ![self.width, self.height, self.min_width, self.min_height]
            .into_iter()
            .flatten()
            .all(Length::is_size)
            || ![self.max_width, self.max_height]
                .into_iter()
                .flatten()
                .all(Limit::is_size)
        {
            return Err("sizes must be finite and nonnegative");
        }
        if self.gap.is_some_and(|v| !v.is_finite() || v < 0.0)
            || self.padding.is_some_and(|v| !v.is_nonnegative())
            || self.margin.is_some_and(|v| !v.is_finite())
        {
            return Err("invalid gap, padding or margin");
        }
        if [self.grow, self.shrink]
            .into_iter()
            .flatten()
            .any(|v| !v.is_finite() || v < 0.0)
        {
            return Err("flex factors must be finite and nonnegative");
        }
        if [self.font_size, self.line_height]
            .into_iter()
            .flatten()
            .any(|v| !v.is_finite() || v <= 0.0)
        {
            return Err("text sizes must be finite and positive");
        }
        if self.font_family.as_ref().is_some_and(|families| {
            families.is_empty()
                || families
                    .iter()
                    .any(|family| matches!(family, Family::Named(name) if name.is_empty() || name.contains('\0')))
        }) {
            return Err("font families must be nonempty and names must not contain NUL");
        }
        if self
            .font_weight
            .is_some_and(|v| !v.is_finite() || !(1.0..=1000.0).contains(&v))
        {
            return Err("font weight must be finite and in [1, 1000]");
        }
        if self.border_width.is_some_and(|v| !v.is_nonnegative())
            || self.radius.is_some_and(|v| !v.is_finite() || v < 0.0)
        {
            return Err("border widths and radius must be finite and nonnegative");
        }
        if self
            .opacity
            .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
        {
            return Err("opacity must be finite and in [0, 1]");
        }
        if let Some(BoxShadow::Outset(shadow)) = self.box_shadow
            && (![shadow.x, shadow.y, shadow.blur, shadow.spread]
                .into_iter()
                .all(f64::is_finite)
                || shadow.blur < 0.0)
        {
            return Err("shadow offsets/spread must be finite and blur finite and nonnegative");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_and_decoration_values_validate_without_erasing_explicit_defaults() {
        assert!(
            Style::new()
                .font_family([Family::Named("serif".into()), Family::Sans])
                .validate()
                .is_ok()
        );
        assert!(Style::new().font_family([]).validate().is_err());
        for name in ["", "nul\0name"] {
            assert!(
                Style::new()
                    .font_family([Family::Named(name.into())])
                    .validate()
                    .is_err()
            );
        }
        for value in [1.0, 450.5, 1000.0] {
            assert!(Style::new().font_weight(value).validate().is_ok());
        }
        for value in [0.0, 1000.1, f64::NAN, f64::INFINITY] {
            assert!(Style::new().font_weight(value).validate().is_err());
        }
        for value in [0.0, 0.5, 1.0] {
            assert!(Style::new().opacity(value).validate().is_ok());
        }
        for value in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
            assert!(Style::new().opacity(value).validate().is_err());
        }
        for value in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(
                Style::new()
                    .border_width(Edges::all(value))
                    .validate()
                    .is_err()
            );
            assert!(Style::new().radius(value).validate().is_err());
        }
        assert_ne!(
            Style::new(),
            Style::new()
                .white_space(WhiteSpace::Normal)
                .border_style(BorderStyle::None)
                .radius(0.0)
                .opacity(1.0)
        );
    }

    #[test]
    fn maximum_limits_validate_both_axes_and_distinguish_absent_from_unbounded() {
        let absent = Style::default();
        let unlimited = Style::new().max_width(Limit::None).max_height(Limit::None);
        assert_ne!(absent, unlimited);
        assert_eq!(absent.properties().count(), 0);
        assert_eq!(
            unlimited.properties().collect::<Vec<_>>(),
            [Property::MaxWidth, Property::MaxHeight]
        );
        assert!(unlimited.validate().is_ok());
        for limit in [
            Limit::None,
            Limit::Px(0.0),
            Limit::Px(-0.0),
            Limit::Px(120.5),
            Limit::Percent(0.0),
            Limit::Percent(125.0),
        ] {
            assert!(
                Style::new()
                    .max_width(limit)
                    .max_height(limit)
                    .validate()
                    .is_ok()
            );
        }
        for value in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for limit in [Limit::Px(value), Limit::Percent(value)] {
                assert!(Style::new().max_width(limit).validate().is_err());
                assert!(Style::new().max_height(limit).validate().is_err());
            }
        }
        // A minimum larger than its maximum is valid CSS: the minimum wins.
        assert!(
            Style::new()
                .min_width(Length::Px(100.0))
                .max_width(Limit::Px(10.0))
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn invalid_geometry_is_rejected_but_negative_margins_are_valid() {
        let mut style = Style {
            margin: Some(Edges::all(-2.0)),
            ..Style::default()
        };
        assert!(style.validate().is_ok());
        style.height = Some(Length::Px(f64::INFINITY));
        assert!(style.validate().is_err());
        style.height = Some(Length::Px(-1.0));
        assert!(style.validate().is_err());
        style.height = Some(Length::Auto);
        style.padding = Some(Edges::all(-1.0));
        assert!(style.validate().is_err());
    }
}
