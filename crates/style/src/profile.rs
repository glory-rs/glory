//! Machine-readable acceptance of the typed inline style contract. This is not
//! a CSS parser, cascade engine, or claim that every HTML stylesheet is portable.
use crate::{
    Align, BorderStyle, Display, Family, FieldSizing, FlexDirection, FlexWrap, Justify, Length,
    Limit, Overflow, Position, Style, TextOverflow, WhiteSpace,
};

macro_rules! properties {
    ($($variant:ident => $field:ident => $css:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u8)]
        pub enum Property { $($variant),+ }
        impl Property {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn name(self) -> &'static str { match self { $(Self::$variant => $css),+ } }
        }
        impl Style {
            /// Explicit declarations only, including an explicitly set default.
            pub fn properties(&self) -> impl Iterator<Item = Property> {
                // Exhaustive: adding a Style field requires updating its profile.
                let Style { $($field),+ } = self;
                [$($field.as_ref().map(|_| Property::$variant)),+].into_iter().flatten()
            }
        }
    };
}
properties! {
    Position => position => "position", Top => top => "top", Left => left => "left",
    Display => display => "display", Direction => flex_direction => "flex-direction",
    FlexWrap => flex_wrap => "flex-wrap",
    Width => width => "width", Height => height => "height",
    MinWidth => min_width => "min-width", MinHeight => min_height => "min-height",
    MaxWidth => max_width => "max-width", MaxHeight => max_height => "max-height",
    Gap => gap => "gap", Padding => padding => "padding", Margin => margin => "margin",
    Grow => grow => "flex-grow", Shrink => shrink => "flex-shrink",
    Align => align => "align-items", Justify => justify => "justify-content",
    OverflowX => overflow_x => "overflow-x", OverflowY => overflow_y => "overflow-y",
    ScrollAnchor => scroll_anchor => "overflow-anchor", Color => color => "color",
    Background => background => "background-color", FontSize => font_size => "font-size",
    LineHeight => line_height => "line-height",
    FontFamily => font_family => "font-family", FontWeight => font_weight => "font-weight",
    WhiteSpace => white_space => "white-space", TextOverflow => text_overflow => "text-overflow", FieldSizing => field_sizing => "field-sizing", BorderWidth => border_width => "border-width",
    BorderColor => border_color => "border-color", BorderStyle => border_style => "border-style",
    Radius => radius => "border-radius", Opacity => opacity => "opacity", BoxShadow => box_shadow => "box-shadow",
}

macro_rules! values {
    ($($variant:ident),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u8)]
        pub enum Value { $($variant),+ }
        impl Value { pub const ALL: &'static [Self] = &[$(Self::$variant),+]; }
    };
}
values! {
    Auto, Px, Percent, Block, Flex, None, Static, Relative, Absolute, Sticky,
    Visible, Hidden, Clip, OverflowAuto, Scroll, AnchorAuto, AnchorNone,
    AlignStart, AlignEnd, AlignCenter, AlignStretch, JustifyStart, JustifyEnd,
    JustifyCenter, Between, Around, Evenly, Row, Column,
    Normal, NoWrap, Pre, PreWrap, BorderNone, BorderSolid,
    FontNamed, Serif, Sans, Mono, System, Cursive, Fantasy, FieldFixed, FieldContent,
    ShadowNone, ShadowOutset, FlexNoWrap, FlexWrap,
    TextClip, TextEllipsis,
}
macro_rules! convert {
    ($ty:ty, $($from:ident => $to:ident),+ $(,)?) => {
        impl From<$ty> for Value {
            fn from(value: $ty) -> Self { match value { $(<$ty>::$from => Self::$to),+ } }
        }
    };
}
convert!(Display, Block => Block, Flex => Flex, None => None);
convert!(Position, Static => Static, Relative => Relative, Absolute => Absolute, Sticky => Sticky);
convert!(Overflow, Visible => Visible, Hidden => Hidden, Clip => Clip, Auto => OverflowAuto, Scroll => Scroll);
convert!(Align, Start => AlignStart, End => AlignEnd, Center => AlignCenter, Stretch => AlignStretch);
convert!(Justify, Start => JustifyStart, End => JustifyEnd, Center => JustifyCenter, Between => Between, Around => Around, Evenly => Evenly);
convert!(FlexDirection, Row => Row, Column => Column);
convert!(FlexWrap, NoWrap => FlexNoWrap, Wrap => FlexWrap);
convert!(WhiteSpace, Normal => Normal, NoWrap => NoWrap, Pre => Pre, PreWrap => PreWrap);
convert!(TextOverflow, Clip => TextClip, Ellipsis => TextEllipsis);
convert!(FieldSizing, Fixed => FieldFixed, Content => FieldContent);
convert!(BorderStyle, None => BorderNone, Solid => BorderSolid);
impl From<&Family> for Value {
    fn from(value: &Family) -> Self {
        match value {
            Family::Named(_) => Self::FontNamed,
            Family::Serif => Self::Serif,
            Family::Sans => Self::Sans,
            Family::Mono => Self::Mono,
            Family::System => Self::System,
            Family::Cursive => Self::Cursive,
            Family::Fantasy => Self::Fantasy,
        }
    }
}
impl From<Length> for Value {
    fn from(value: Length) -> Self {
        match value {
            Length::Auto => Self::Auto,
            Length::Px(_) => Self::Px,
            Length::Percent(_) => Self::Percent,
        }
    }
}
impl From<Limit> for Value {
    fn from(value: Limit) -> Self {
        match value {
            Limit::None => Self::None,
            Limit::Px(_) => Self::Px,
            Limit::Percent(_) => Self::Percent,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unsupported {
    pub property: Property,
    /// None means the entire property is unsupported; Some identifies its value.
    pub value: Option<Value>,
}

/// A conservative acceptance declaration. Advertising a property accepts its
/// valid numeric domain; finite enum/unit restrictions can be declared separately.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Support {
    properties: u64,
    values: u64,
}
impl Support {
    pub const NONE: Self = Self {
        properties: 0,
        values: 0,
    };
    pub const ALL: Self = Self {
        properties: u64::MAX >> (64 - Property::ALL.len()),
        values: u64::MAX >> (64 - Value::ALL.len()),
    };
    pub const fn supports_property(self, property: Property) -> bool {
        self.properties & (1 << property as u8) != 0
    }
    pub const fn supports_value(self, value: Value) -> bool {
        self.values & (1 << value as u8) != 0
    }
    pub const fn with_property(self, property: Property) -> Self {
        Self {
            properties: self.properties | (1 << property as u8),
            ..self
        }
    }
    pub const fn without_property(self, property: Property) -> Self {
        Self {
            properties: self.properties & !(1 << property as u8),
            ..self
        }
    }
    pub const fn with_value(self, value: Value) -> Self {
        Self {
            values: self.values | (1 << value as u8),
            ..self
        }
    }
    pub const fn without_value(self, value: Value) -> Self {
        Self {
            values: self.values & !(1 << value as u8),
            ..self
        }
    }

    /// Check support, not numeric validity; call Style::validate for the latter.
    pub fn check(self, style: &Style) -> Result<(), Unsupported> {
        if self == Self::ALL {
            return Ok(());
        }
        for property in style.properties() {
            if !self.supports_property(property) {
                return Err(Unsupported {
                    property,
                    value: None,
                });
            }
        }
        let values = [
            style.position.map(|v| (Property::Position, v.into())),
            style.top.map(|v| (Property::Top, v.into())),
            style.left.map(|v| (Property::Left, v.into())),
            style.display.map(|v| (Property::Display, v.into())),
            style
                .flex_direction
                .map(|v| (Property::Direction, v.into())),
            style.flex_wrap.map(|v| (Property::FlexWrap, v.into())),
            style.width.map(|v| (Property::Width, v.into())),
            style.height.map(|v| (Property::Height, v.into())),
            style.min_width.map(|v| (Property::MinWidth, v.into())),
            style.min_height.map(|v| (Property::MinHeight, v.into())),
            style.max_width.map(|v| (Property::MaxWidth, v.into())),
            style.max_height.map(|v| (Property::MaxHeight, v.into())),
            style.align.map(|v| (Property::Align, v.into())),
            style.justify.map(|v| (Property::Justify, v.into())),
            style.overflow_x.map(|v| (Property::OverflowX, v.into())),
            style.overflow_y.map(|v| (Property::OverflowY, v.into())),
            style.scroll_anchor.map(|v| {
                (
                    Property::ScrollAnchor,
                    if v {
                        Value::AnchorAuto
                    } else {
                        Value::AnchorNone
                    },
                )
            }),
            style.gap.map(|_| (Property::Gap, Value::Px)),
            style.padding.map(|_| (Property::Padding, Value::Px)),
            style.margin.map(|_| (Property::Margin, Value::Px)),
            style.font_size.map(|_| (Property::FontSize, Value::Px)),
            style.line_height.map(|_| (Property::LineHeight, Value::Px)),
            style.white_space.map(|v| (Property::WhiteSpace, v.into())),
            style
                .text_overflow
                .map(|v| (Property::TextOverflow, v.into())),
            style
                .field_sizing
                .map(|v| (Property::FieldSizing, v.into())),
            style
                .border_style
                .map(|v| (Property::BorderStyle, v.into())),
            style
                .border_width
                .map(|_| (Property::BorderWidth, Value::Px)),
            style.radius.map(|_| (Property::Radius, Value::Px)),
            style.box_shadow.map(|shadow| {
                (
                    Property::BoxShadow,
                    match shadow {
                        crate::BoxShadow::None => Value::ShadowNone,
                        crate::BoxShadow::Outset(_) => Value::ShadowOutset,
                    },
                )
            }),
            style.box_shadow.and_then(|shadow| {
                matches!(shadow, crate::BoxShadow::Outset(_))
                    .then_some((Property::BoxShadow, Value::Px))
            }),
        ];
        for (property, value) in values.into_iter().flatten() {
            if !self.supports_value(value) {
                return Err(Unsupported {
                    property,
                    value: Some(value),
                });
            }
        }
        for family in style.font_family.iter().flatten() {
            let value = family.into();
            if !self.supports_value(value) {
                return Err(Unsupported {
                    property: Property::FontFamily,
                    value: Some(value),
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Css {
    #[default]
    None,
    /// HTML-family CSS, interpreted by the selected engine with its own coverage.
    Html,
    /// A named toolkit dialect; it must not be presented as HTML/portable CSS.
    Toolkit(&'static str),
}

/// Per-renderer, immutable for an App lifetime. Version identifies this profile,
/// not an upstream engine release or a promise of cross-engine pixel equality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profile {
    pub name: &'static str,
    pub version: u32,
    pub inline: Support,
    pub css: Css,
}
impl Profile {
    pub const NONE: Self = Self {
        name: "none",
        version: 1,
        inline: Support::NONE,
        css: Css::None,
    };
    /// Reference storage accepts typed declarations but performs no CSS/layout.
    pub const RECORD: Self = Self {
        name: "record",
        version: 5,
        inline: Support::ALL,
        css: Css::None,
    };
    /// Common HTML lowering accepts the current typed subset. Host CSS coverage
    /// remains engine-specific; this is not a complete stylesheet validator.
    pub const HTML: Self = Self {
        name: "html",
        version: 5,
        inline: Support::ALL,
        css: Css::Html,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, Edges};

    #[test]
    fn maximum_limit_keywords_and_units_obey_the_selected_profile() {
        for (limit, value) in [
            (Limit::None, Value::None),
            (Limit::Px(20.0), Value::Px),
            (Limit::Percent(50.0), Value::Percent),
        ] {
            for (style, property) in [
                (Style::new().max_width(limit), Property::MaxWidth),
                (Style::new().max_height(limit), Property::MaxHeight),
            ] {
                assert!(Profile::HTML.inline.check(&style).is_ok());
                assert!(Profile::RECORD.inline.check(&style).is_ok());
                assert_eq!(
                    Support::ALL.without_value(value).check(&style),
                    Err(Unsupported {
                        property,
                        value: Some(value)
                    })
                );
                assert_eq!(
                    Support::ALL.without_property(property).check(&style),
                    Err(Unsupported {
                        property,
                        value: None
                    })
                );
            }
        }
        assert_eq!((Profile::HTML.version, Profile::RECORD.version), (5, 5));
    }

    #[test]
    fn every_declared_property_can_be_queried_and_rejected_independently() {
        let style = Style {
            position: Some(Position::Relative),
            top: Some(Length::Auto),
            left: Some(Length::Px(-1.0)),
            display: Some(Display::Flex),
            flex_direction: Some(FlexDirection::Row),
            flex_wrap: Some(FlexWrap::Wrap),
            width: Some(Length::Percent(100.0)),
            height: Some(Length::Auto),
            min_width: Some(Length::Px(1.0)),
            min_height: Some(Length::Auto),
            max_width: Some(Limit::None),
            max_height: Some(Limit::Px(999.0)),
            gap: Some(2.0),
            padding: Some(Edges::all(2.0)),
            margin: Some(Edges::all(-1.0)),
            grow: Some(1.0),
            shrink: Some(1.0),
            align: Some(Align::Center),
            justify: Some(Justify::Evenly),
            overflow_x: Some(Overflow::Clip),
            overflow_y: Some(Overflow::Auto),
            scroll_anchor: Some(false),
            color: Some(Color(0, 0, 0, 255)),
            background: Some(Color(255, 255, 255, 255)),
            font_size: Some(16.0),
            line_height: Some(24.0),
            font_family: Some(vec![Family::System]),
            font_weight: Some(450.5),
            white_space: Some(WhiteSpace::PreWrap),
            text_overflow: Some(TextOverflow::Ellipsis),
            field_sizing: Some(FieldSizing::Content),
            border_width: Some(Edges::all(1.0)),
            border_color: Some(Color(0, 0, 0, 255)),
            border_style: Some(BorderStyle::Solid),
            radius: Some(4.0),
            opacity: Some(0.5),
            box_shadow: Some(crate::BoxShadow::None),
        };
        style.validate().unwrap();
        assert_eq!(style.properties().collect::<Vec<_>>(), Property::ALL);
        assert_eq!(Support::ALL.check(&style), Ok(()));
        for property in Property::ALL {
            assert!(!property.name().is_empty());
            assert_eq!(
                Support::ALL.without_property(*property).check(&style),
                Err(Unsupported {
                    property: *property,
                    value: None
                })
            );
        }
        assert_eq!(Support::NONE.check(&Style::default()), Ok(()));
        assert_eq!(Style::default().properties().count(), 0);
    }

    #[test]
    fn finite_enum_values_and_length_units_are_all_checked() {
        for value in Value::ALL {
            let (property, style) = match value {
                Value::TextClip => (
                    Property::TextOverflow,
                    Style::new().text_overflow(TextOverflow::Clip),
                ),
                Value::TextEllipsis => (
                    Property::TextOverflow,
                    Style::new().text_overflow(TextOverflow::Ellipsis),
                ),
                Value::FlexNoWrap => (Property::FlexWrap, Style::new().flex_wrap(FlexWrap::NoWrap)),
                Value::FlexWrap => (Property::FlexWrap, Style::new().flex_wrap(FlexWrap::Wrap)),
                Value::ShadowNone => (
                    Property::BoxShadow,
                    Style::new().box_shadow(crate::BoxShadow::None),
                ),
                Value::ShadowOutset => (
                    Property::BoxShadow,
                    Style::new().box_shadow(crate::BoxShadow::Outset(crate::Shadow {
                        x: 0.0,
                        y: 2.0,
                        blur: 4.0,
                        spread: 0.0,
                        color: Color(0, 0, 0, 30),
                    })),
                ),
                Value::FieldFixed => (
                    Property::FieldSizing,
                    Style::new().field_sizing(FieldSizing::Fixed),
                ),
                Value::FieldContent => (
                    Property::FieldSizing,
                    Style::new().field_sizing(FieldSizing::Content),
                ),
                Value::Auto => (Property::Width, Style::new().width(Length::Auto)),
                Value::Px => (Property::Width, Style::new().width(Length::Px(1.0))),
                Value::Percent => (Property::Width, Style::new().width(Length::Percent(1.0))),
                Value::Block => (Property::Display, Style::new().display(Display::Block)),
                Value::Flex => (Property::Display, Style::new().display(Display::Flex)),
                Value::None => (Property::Display, Style::new().display(Display::None)),
                Value::Static => (Property::Position, Style::new().position(Position::Static)),
                Value::Relative => (
                    Property::Position,
                    Style::new().position(Position::Relative),
                ),
                Value::Absolute => (
                    Property::Position,
                    Style::new().position(Position::Absolute),
                ),
                Value::Sticky => (Property::Position, Style::new().position(Position::Sticky)),
                Value::Visible => (
                    Property::OverflowY,
                    Style::new().overflow_y(Overflow::Visible),
                ),
                Value::Hidden => (
                    Property::OverflowY,
                    Style::new().overflow_y(Overflow::Hidden),
                ),
                Value::Clip => (Property::OverflowY, Style::new().overflow_y(Overflow::Clip)),
                Value::OverflowAuto => {
                    (Property::OverflowY, Style::new().overflow_y(Overflow::Auto))
                }
                Value::Scroll => (
                    Property::OverflowY,
                    Style::new().overflow_y(Overflow::Scroll),
                ),
                Value::AnchorAuto => (Property::ScrollAnchor, Style::new().scroll_anchor(true)),
                Value::AnchorNone => (Property::ScrollAnchor, Style::new().scroll_anchor(false)),
                Value::AlignStart => (
                    Property::Align,
                    Style {
                        align: Some(Align::Start),
                        ..Style::new()
                    },
                ),
                Value::AlignEnd => (
                    Property::Align,
                    Style {
                        align: Some(Align::End),
                        ..Style::new()
                    },
                ),
                Value::AlignCenter => (
                    Property::Align,
                    Style {
                        align: Some(Align::Center),
                        ..Style::new()
                    },
                ),
                Value::AlignStretch => (
                    Property::Align,
                    Style {
                        align: Some(Align::Stretch),
                        ..Style::new()
                    },
                ),
                Value::JustifyStart => (
                    Property::Justify,
                    Style {
                        justify: Some(Justify::Start),
                        ..Style::new()
                    },
                ),
                Value::JustifyEnd => (
                    Property::Justify,
                    Style {
                        justify: Some(Justify::End),
                        ..Style::new()
                    },
                ),
                Value::JustifyCenter => (
                    Property::Justify,
                    Style {
                        justify: Some(Justify::Center),
                        ..Style::new()
                    },
                ),
                Value::Between => (
                    Property::Justify,
                    Style {
                        justify: Some(Justify::Between),
                        ..Style::new()
                    },
                ),
                Value::Around => (
                    Property::Justify,
                    Style {
                        justify: Some(Justify::Around),
                        ..Style::new()
                    },
                ),
                Value::Evenly => (
                    Property::Justify,
                    Style {
                        justify: Some(Justify::Evenly),
                        ..Style::new()
                    },
                ),
                Value::Row => (
                    Property::Direction,
                    Style::new().flex_direction(FlexDirection::Row),
                ),
                Value::Column => (
                    Property::Direction,
                    Style::new().flex_direction(FlexDirection::Column),
                ),
                Value::Normal => (
                    Property::WhiteSpace,
                    Style::new().white_space(WhiteSpace::Normal),
                ),
                Value::NoWrap => (
                    Property::WhiteSpace,
                    Style::new().white_space(WhiteSpace::NoWrap),
                ),
                Value::Pre => (
                    Property::WhiteSpace,
                    Style::new().white_space(WhiteSpace::Pre),
                ),
                Value::PreWrap => (
                    Property::WhiteSpace,
                    Style::new().white_space(WhiteSpace::PreWrap),
                ),
                Value::BorderNone => (
                    Property::BorderStyle,
                    Style::new().border_style(BorderStyle::None),
                ),
                Value::BorderSolid => (
                    Property::BorderStyle,
                    Style::new().border_style(BorderStyle::Solid),
                ),
                Value::FontNamed => (
                    Property::FontFamily,
                    Style::new().font_family([Family::Named("serif".into())]),
                ),
                Value::Serif => (
                    Property::FontFamily,
                    Style::new().font_family([Family::Serif]),
                ),
                Value::Sans => (
                    Property::FontFamily,
                    Style::new().font_family([Family::Sans]),
                ),
                Value::Mono => (
                    Property::FontFamily,
                    Style::new().font_family([Family::Mono]),
                ),
                Value::System => (
                    Property::FontFamily,
                    Style::new().font_family([Family::System]),
                ),
                Value::Cursive => (
                    Property::FontFamily,
                    Style::new().font_family([Family::Cursive]),
                ),
                Value::Fantasy => (
                    Property::FontFamily,
                    Style::new().font_family([Family::Fantasy]),
                ),
            };
            assert_eq!(
                Support::ALL.without_value(*value).check(&style),
                Err(Unsupported {
                    property,
                    value: Some(*value)
                })
            );
        }
        let px_only = Support::NONE
            .with_property(Property::Width)
            .with_value(Value::Px);
        assert!(px_only.check(&Style::new().width(Length::Px(1.0))).is_ok());
        assert!(
            px_only
                .check(&Style::new().width(Length::Percent(1.0)))
                .is_err()
        );
        assert_eq!(Profile::RECORD.css, Css::None);
        assert_eq!(Profile::HTML.css, Css::Html);
    }

    #[test]
    fn implicit_pixel_declarations_obey_the_same_unit_support() {
        let support = Support::ALL.without_value(Value::Px);
        for (property, style) in [
            (
                Property::BorderWidth,
                Style::new().border_width(Edges::all(2.0)),
            ),
            (Property::Radius, Style::new().radius(4.0)),
            (
                Property::Gap,
                Style {
                    gap: Some(1.0),
                    ..Style::new()
                },
            ),
            (
                Property::Padding,
                Style {
                    padding: Some(Edges::all(1.0)),
                    ..Style::new()
                },
            ),
            (
                Property::Margin,
                Style {
                    margin: Some(Edges::all(-1.0)),
                    ..Style::new()
                },
            ),
            (
                Property::FontSize,
                Style {
                    font_size: Some(16.0),
                    ..Style::new()
                },
            ),
            (
                Property::LineHeight,
                Style {
                    line_height: Some(24.0),
                    ..Style::new()
                },
            ),
        ] {
            assert_eq!(
                support.check(&style),
                Err(Unsupported {
                    property,
                    value: Some(Value::Px)
                })
            );
        }
    }
}
