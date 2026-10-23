//! Definite-size flex geometry shared by native and browser acceptance.
//! This finite matrix is not an intrinsic text/font or complete CSS oracle.
use crate::style::{Align, Display, FlexDirection, Justify, Length, Overflow, Style};
use crate::{Descriptor, Props, Rect, TextProps};

#[derive(Clone, Debug)]
pub struct FlexCase {
    pub name: String,
    pub parent: Style,
    pub child: Style,
    pub text: bool,
    /// Child border boxes, relative to the parent border-box origin.
    pub expected: [Rect; 2],
}
impl FlexCase {
    pub fn parent_descriptor(&self) -> Descriptor {
        let mut d = Descriptor::new(Props::View);
        d.classes.push("flex-parent".into());
        d.style = self.parent.clone();
        d
    }
    pub fn child_descriptor(&self) -> Descriptor {
        let mut d = Descriptor::new(if self.text {
            Props::Text(TextProps {
                content: "MMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMM".into(),
                ..TextProps::default()
            })
        } else {
            Props::View
        });
        d.classes.push("flex-child".into());
        d.style = self.child.clone();
        d
    }
}
fn rect(row: bool, main: f64, cross: f64, size: f64, cross_size: f64) -> Rect {
    if row {
        Rect {
            x: main,
            y: cross,
            width: size,
            height: cross_size,
        }
    } else {
        Rect {
            x: cross,
            y: main,
            width: cross_size,
            height: size,
        }
    }
}
pub fn flex_cases() -> Vec<FlexCase> {
    let mut cases = Vec::new();
    for direction in [FlexDirection::Row, FlexDirection::Column] {
        let row = direction == FlexDirection::Row;
        let main = if row { 300.0 } else { 200.0 };
        let cross = if row { 200.0 } else { 300.0 };
        let size = if row { 50.0 } else { 30.0 };
        let cross_size = if row { 30.0 } else { 50.0 };
        let parent = Style::new()
            .display(Display::Flex)
            .flex_direction(direction)
            .width(Length::Px(300.0))
            .height(Length::Px(200.0))
            .gap(10.0);
        let child = Style::new()
            .width(Length::Px(50.0))
            .height(Length::Px(30.0));
        let free = main - 2.0 * size - 10.0;
        for justify in [
            Justify::Start,
            Justify::End,
            Justify::Center,
            Justify::Between,
            Justify::Around,
            Justify::Evenly,
        ] {
            let (first, extra_gap) = match justify {
                Justify::Start => (0.0, 0.0),
                Justify::End => (free, 0.0),
                Justify::Center => (free / 2.0, 0.0),
                Justify::Between => (0.0, free),
                Justify::Around => (free / 4.0, free / 2.0),
                Justify::Evenly => (free / 3.0, free / 3.0),
            };
            cases.push(FlexCase {
                name: format!("{direction:?}-{justify:?}"),
                parent: Style {
                    justify: Some(justify),
                    ..parent.clone()
                },
                child: child.clone(),
                text: false,
                expected: [
                    rect(row, first, 0.0, size, cross_size),
                    rect(row, first + size + 10.0 + extra_gap, 0.0, size, cross_size),
                ],
            });
        }
        for align in [Align::Start, Align::End, Align::Center, Align::Stretch] {
            let cross_size = if align == Align::Stretch { cross } else { 0.0 };
            let offset = match align {
                Align::End => cross,
                Align::Center => cross / 2.0,
                _ => 0.0,
            };
            cases.push(FlexCase {
                name: format!("{direction:?}-{align:?}-align"),
                parent: Style {
                    align: Some(align),
                    ..parent.clone()
                },
                child: if row {
                    Style::new().width(Length::Px(size))
                } else {
                    Style::new().height(Length::Px(size))
                },
                text: false,
                expected: [
                    rect(row, 0.0, offset, size, cross_size),
                    rect(row, size + 10.0, offset, size, cross_size),
                ],
            });
        }
        for factor in [0.0, 0.25, 1.0, 2.0] {
            for grow in [true, false] {
                let (base, across) = if grow {
                    (size, cross_size)
                } else if row {
                    (250.0, 180.0)
                } else {
                    (180.0, 250.0)
                };
                // Fractional factors below a sum of 1 consume only that fraction
                // of initial free space. The fixed gap is already subtracted.
                let used = base + (main - 2.0 * base - 10.0) * (2.0_f64 * factor).min(1.0) / 2.0;
                let child = if grow {
                    Style {
                        grow: Some(factor),
                        ..child.clone()
                    }
                } else {
                    Style {
                        shrink: Some(factor),
                        ..Style::new()
                            .width(Length::Px(250.0))
                            .height(Length::Px(180.0))
                    }
                };
                cases.push(FlexCase {
                    name: format!(
                        "{direction:?}-{}-{factor}",
                        if grow { "grow" } else { "shrink" }
                    ),
                    parent: parent.clone(),
                    child,
                    text: false,
                    expected: [
                        rect(row, 0.0, 0.0, used, across),
                        rect(row, used + 10.0, 0.0, used, across),
                    ],
                });
            }
        }
        for overflow in [
            Overflow::Visible,
            Overflow::Hidden,
            Overflow::Clip,
            Overflow::Auto,
            Overflow::Scroll,
        ] {
            let base = if row { 250.0 } else { 180.0 };
            let used = if matches!(overflow, Overflow::Visible | Overflow::Clip) {
                base
            } else {
                (main - 10.0) / 2.0
            };
            let child = Style {
                font_size: Some(16.0),
                line_height: Some(400.0),
                ..Style::new()
                    .display(Display::Block)
                    .width(Length::Px(250.0))
                    .height(Length::Px(180.0))
                    .overflow_x(overflow)
                    .overflow_y(overflow)
            };
            cases.push(FlexCase {
                name: format!("{direction:?}-auto-min-{overflow:?}"),
                parent: parent.clone(),
                child,
                text: true,
                expected: [
                    rect(row, 0.0, 0.0, used, if row { 180.0 } else { 250.0 }),
                    rect(row, used + 10.0, 0.0, used, if row { 180.0 } else { 250.0 }),
                ],
            });
        }
    }
    cases
}
