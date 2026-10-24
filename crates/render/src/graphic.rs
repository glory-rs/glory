//! Finite vector geometry. No host markup, fonts, resources or GPU objects.
use crate::Error;
use glory_style::Color;

pub const MAX_GRAPHIC_COMMANDS: usize = 256;
pub const MAX_GRAPHIC_COORDINATE: f64 = 1_000_000.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphicPoint {
    pub x: f64,
    pub y: f64,
}
impl GraphicPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    fn validate(self) -> Result<(), Error> {
        if [self.x, self.y]
            .into_iter()
            .any(|value| !value.is_finite() || value.abs() > MAX_GRAPHIC_COORDINATE)
        {
            return Err(Error::InvalidProps(
                "graphic coordinates must be finite and within +/-1000000",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCommand {
    Move(GraphicPoint),
    Line(GraphicPoint),
    Cubic {
        first: GraphicPoint,
        second: GraphicPoint,
        end: GraphicPoint,
    },
    Close,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StrokeCap {
    #[default]
    Butt,
    Round,
    Square,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphicStroke {
    pub color: Color,
    pub width: f64,
    pub cap: StrokeCap,
}

/// One nonzero-winding path, uniformly scaled and centered in its content box.
/// Geometry is clipped to that box. Multiple colors are separate semantic nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphicProps {
    pub width: f64,
    pub height: f64,
    pub commands: Vec<PathCommand>,
    pub fill: Option<Color>,
    pub stroke: Option<GraphicStroke>,
    /// `None` explicitly decorates the graphic; text must not be announced twice.
    pub alternative: Option<String>,
    pub animation: Option<crate::GraphicAnimation>,
    pub transition: Option<crate::GraphicTransition>,
}
impl Default for GraphicProps {
    fn default() -> Self {
        Self {
            width: 24.0,
            height: 24.0,
            commands: Vec::new(),
            fill: None,
            stroke: None,
            alternative: None,
            animation: None,
            transition: None,
        }
    }
}
impl GraphicProps {
    pub(crate) fn transition_from(&self, previous: &Self) -> Result<(), Error> {
        if self.transition.is_some()
            && !crate::GraphicTransition::compatible(&previous.commands, &self.commands)
        {
            return Err(Error::InvalidProps(
                "graphic transition requires compatible command topology",
            ));
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(transition) = self.transition {
            transition.validate()?;
            if self.animation.is_some() {
                return Err(Error::InvalidProps(
                    "graphic animation and transition are mutually exclusive",
                ));
            }
        }
        if let Some(animation) = self.animation {
            animation.validate()?;
        }
        if [self.width, self.height].into_iter().any(|value| {
            !value.is_finite() || !(0.000001..=MAX_GRAPHIC_COORDINATE).contains(&value)
        }) {
            return Err(Error::InvalidProps(
                "graphic viewport dimensions must be finite and in 0.000001..=1000000",
            ));
        }
        if self.commands.len() > MAX_GRAPHIC_COMMANDS {
            return Err(Error::InvalidProps(
                "graphic path exceeds its 256 command budget",
            ));
        }
        if self
            .alternative
            .as_ref()
            .is_some_and(|text| text.trim().is_empty() || text.len() > 1024)
        {
            return Err(Error::InvalidProps(
                "graphic alternative must be nonempty and at most 1024 UTF-8 bytes",
            ));
        }
        if self.stroke.is_some_and(|stroke| {
            !stroke.width.is_finite() || !(0.000001..=10000.0).contains(&stroke.width)
        }) {
            return Err(Error::InvalidProps(
                "graphic stroke width must be finite and in 0.000001..=10000",
            ));
        }
        let mut open = false;
        for command in &self.commands {
            match command {
                PathCommand::Move(point) => {
                    point.validate()?;
                    open = true;
                }
                PathCommand::Line(point) => {
                    if !open {
                        return Err(Error::InvalidProps(
                            "graphic segments require a preceding Move",
                        ));
                    }
                    point.validate()?;
                }
                PathCommand::Cubic { first, second, end } => {
                    if !open {
                        return Err(Error::InvalidProps(
                            "graphic segments require a preceding Move",
                        ));
                    }
                    first.validate()?;
                    second.validate()?;
                    end.validate()?;
                }
                PathCommand::Close => {
                    if !open {
                        return Err(Error::InvalidProps(
                            "graphic Close requires an open subpath",
                        ));
                    }
                    open = false;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point() -> GraphicPoint {
        GraphicPoint::new(1.0, 2.0)
    }
    #[test]
    fn every_command_checks_order_and_all_coordinates() {
        let mut props = GraphicProps::default();
        for command in [
            PathCommand::Line(point()),
            PathCommand::Cubic {
                first: point(),
                second: point(),
                end: point(),
            },
            PathCommand::Close,
        ] {
            props.commands = vec![command];
            assert!(props.validate().is_err());
        }
        for value in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            1_000_001.0,
            -1_000_001.0,
        ] {
            for bad in [GraphicPoint::new(value, 0.0), GraphicPoint::new(0.0, value)] {
                for command in [
                    PathCommand::Move(bad),
                    PathCommand::Line(bad),
                    PathCommand::Cubic {
                        first: bad,
                        second: point(),
                        end: point(),
                    },
                    PathCommand::Cubic {
                        first: point(),
                        second: bad,
                        end: point(),
                    },
                    PathCommand::Cubic {
                        first: point(),
                        second: point(),
                        end: bad,
                    },
                ] {
                    props.commands = vec![PathCommand::Move(point()), command];
                    assert!(props.validate().is_err());
                }
            }
        }
        props.commands = vec![
            PathCommand::Move(point()),
            PathCommand::Line(point()),
            PathCommand::Cubic {
                first: point(),
                second: point(),
                end: point(),
            },
            PathCommand::Close,
            PathCommand::Move(point()),
        ];
        props.validate().unwrap();
        props.commands.push(PathCommand::Close);
        props.commands.push(PathCommand::Close);
        assert!(props.validate().is_err());
    }
    #[test]
    fn viewport_stroke_alternative_and_command_budgets_are_independent() {
        for value in [0.0, -1.0, f64::NAN, f64::INFINITY, 0.0000001, 1_000_001.0] {
            assert!(
                GraphicProps {
                    width: value,
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
            assert!(
                GraphicProps {
                    height: value,
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
        }
        for width in [0.0, -1.0, f64::NAN, f64::INFINITY, 0.0000001, 10001.0] {
            assert!(
                GraphicProps {
                    stroke: Some(GraphicStroke {
                        color: Color(0, 0, 0, 255),
                        width,
                        cap: StrokeCap::Butt
                    }),
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
        }
        for cap in [StrokeCap::Butt, StrokeCap::Round, StrokeCap::Square] {
            GraphicProps {
                stroke: Some(GraphicStroke {
                    color: Color(0, 0, 0, 255),
                    width: 1.0,
                    cap,
                }),
                ..Default::default()
            }
            .validate()
            .unwrap();
        }
        for alternative in ["".to_string(), " ".to_string(), "x".repeat(1025)] {
            assert!(
                GraphicProps {
                    alternative: Some(alternative),
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
        }
        let mut props = GraphicProps {
            commands: vec![PathCommand::Move(point()); 256],
            ..Default::default()
        };
        props.validate().unwrap();
        props.commands.push(PathCommand::Move(point()));
        assert!(props.validate().is_err());
    }
}
