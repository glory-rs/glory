use crate::{Error, GraphicPoint, MAX_GRAPHIC_COMMANDS, MAX_GRAPHIC_COORDINATE, PathCommand};

/// A finite transition between compatible paths. Creation paints the endpoint;
/// only a later accepted path replacement supplies a previous geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphicTransition {
    pub duration_millis: u16,
}
impl GraphicTransition {
    pub fn validate(self) -> Result<(), Error> {
        if self.duration_millis > 500 {
            return Err(Error::InvalidProps(
                "graphic transition duration exceeds 500 milliseconds",
            ));
        }
        Ok(())
    }
    pub fn compatible(from: &[PathCommand], to: &[PathCommand]) -> bool {
        from.len() == to.len()
            && from.len() <= MAX_GRAPHIC_COMMANDS
            && from
                .iter()
                .zip(to)
                .all(|(a, b)| std::mem::discriminant(a) == std::mem::discriminant(b))
    }
    /// One allocation is useful when interrupting a transition, never per paint.
    pub fn sample(
        from: &[PathCommand],
        to: &[PathCommand],
        progress: f64,
    ) -> Result<Vec<PathCommand>, Error> {
        if !Self::compatible(from, to) {
            return Err(Error::InvalidProps(
                "graphic transition requires compatible command topology",
            ));
        }
        from.iter()
            .zip(to)
            .map(|(a, b)| Self::command(*a, *b, progress))
            .collect()
    }
    pub fn command(
        from: PathCommand,
        to: PathCommand,
        progress: f64,
    ) -> Result<PathCommand, Error> {
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
            return Err(Error::InvalidProps("invalid graphic transition progress"));
        }
        let point = |a: GraphicPoint, b: GraphicPoint| -> Result<GraphicPoint, Error> {
            if [a.x, a.y, b.x, b.y]
                .iter()
                .any(|v| !v.is_finite() || v.abs() > MAX_GRAPHIC_COORDINATE)
            {
                return Err(Error::InvalidProps("invalid graphic transition coordinate"));
            }
            Ok(GraphicPoint::new(
                a.x * (1.0 - progress) + b.x * progress,
                a.y * (1.0 - progress) + b.y * progress,
            ))
        };
        Ok(match (from, to) {
            (PathCommand::Move(a), PathCommand::Move(b)) => PathCommand::Move(point(a, b)?),
            (PathCommand::Line(a), PathCommand::Line(b)) => PathCommand::Line(point(a, b)?),
            (
                PathCommand::Cubic {
                    first: a,
                    second: b,
                    end: c,
                },
                PathCommand::Cubic {
                    first: x,
                    second: y,
                    end: z,
                },
            ) => PathCommand::Cubic {
                first: point(a, x)?,
                second: point(b, y)?,
                end: point(c, z)?,
            },
            (PathCommand::Close, PathCommand::Close) => PathCommand::Close,
            _ => {
                return Err(Error::InvalidProps(
                    "graphic transition requires compatible command topology",
                ));
            }
        })
    }
}
