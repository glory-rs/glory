//! Finite, paint-only translations. These declarations do not change layout
//! or allocate application frame tasks. Hosts own only the current phase.
use crate::Error;

pub const MAX_TRANSLATION_COORDINATE: f64 = 1_000_000.;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TranslationOffset {
    pub x: f64,
    pub y: f64,
}
impl TranslationOffset {
    pub const ZERO: Self = Self { x: 0., y: 0. };
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    fn validate(self) -> Result<(), Error> {
        if [self.x, self.y]
            .into_iter()
            .any(|value| !value.is_finite() || value.abs() > MAX_TRANSLATION_COORDINATE)
        {
            return Err(Error::InvalidProps(
                "translation coordinates must be finite and within +/-1000000",
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TranslationKind {
    /// Repeat from the explicit start. Replacing the endpoints restarts here.
    Loop {
        from: TranslationOffset,
        to: TranslationOffset,
        period_millis: u32,
    },
    /// First appearance is static. Later target changes start at current ink.
    To {
        target: TranslationOffset,
        duration_millis: u16,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TranslationMotion {
    pub kind: TranslationKind,
    /// Pausing retains the current phase; equal declarations do not restart it.
    pub paused: bool,
}
impl TranslationMotion {
    pub fn validate(self) -> Result<(), Error> {
        match self.kind {
            TranslationKind::Loop {
                from,
                to,
                period_millis,
            } => {
                from.validate()?;
                to.validate()?;
                if !(16..=60_000).contains(&period_millis) {
                    return Err(Error::InvalidProps(
                        "translation period must be in 16..=60000 milliseconds",
                    ));
                }
            }
            TranslationKind::To {
                target,
                duration_millis,
            } => {
                target.validate()?;
                if duration_millis > 2000 {
                    return Err(Error::InvalidProps(
                        "translation duration must be in 0..=2000 milliseconds",
                    ));
                }
            }
        }
        Ok(())
    }
    /// Static SSR, Reduce, and hidden position. It never describes a completion.
    pub fn resting_offset(self) -> TranslationOffset {
        match self.kind {
            TranslationKind::Loop { from, .. } => from,
            TranslationKind::To { target, .. } => target,
        }
    }
}

/// Deterministic current host phase. No identities, callbacks, or history.
/// Times are monotonic host seconds, independent from application state.
#[derive(Clone, Debug)]
pub struct TranslationPhase {
    config: TranslationMotion,
    from: TranslationOffset,
    elapsed: f64,
    started: f64,
    allowed: bool,
    settled: bool,
}
impl TranslationPhase {
    pub fn new(config: TranslationMotion, seconds: f64, allowed: bool) -> Result<Self, Error> {
        config.validate()?;
        clock(seconds, 0.)?;
        Ok(Self {
            config,
            from: config.resting_offset(),
            elapsed: 0.,
            started: seconds,
            allowed,
            settled: !matches!(config.kind, TranslationKind::Loop { .. }),
        })
    }
    fn elapsed(&self, seconds: f64) -> f64 {
        let elapsed = self.elapsed
            + if self.allowed && !self.config.paused && !self.settled {
                seconds - self.started
            } else {
                0.
            };
        match self.config.kind {
            TranslationKind::Loop { period_millis, .. } => {
                elapsed.rem_euclid(f64::from(period_millis) / 1000.)
            }
            TranslationKind::To {
                duration_millis, ..
            } => elapsed.min(f64::from(duration_millis) / 1000.),
        }
    }
    pub fn config(&self) -> TranslationMotion {
        self.config
    }
    /// Accepted start of a finite transition, sampled at the preceding turn.
    pub fn origin(&self) -> TranslationOffset {
        self.from
    }
    /// Whether the current declaration owns an admitted physical animation,
    /// including a paused or completed one retained at its final position.
    pub fn admitted(&self) -> bool {
        self.allowed && !self.settled
    }
    pub fn offset(&self, seconds: f64) -> Result<TranslationOffset, Error> {
        clock(seconds, self.started)?;
        if !self.allowed {
            return Ok(self.config.resting_offset());
        }
        let (from, to, fraction) = match self.config.kind {
            TranslationKind::Loop {
                from,
                to,
                period_millis,
            } => (
                from,
                to,
                self.elapsed(seconds) / (f64::from(period_millis) / 1000.),
            ),
            TranslationKind::To {
                target,
                duration_millis,
            } => {
                if self.settled || duration_millis == 0 {
                    return Ok(target);
                }
                (
                    self.from,
                    target,
                    self.elapsed(seconds) / (f64::from(duration_millis) / 1000.),
                )
            }
        };
        Ok(TranslationOffset {
            x: from.x + (to.x - from.x) * fraction,
            y: from.y + (to.y - from.y) * fraction,
        })
    }
    pub fn running(&self, seconds: f64) -> Result<bool, Error> {
        clock(seconds, self.started)?;
        if !self.allowed || self.config.paused {
            return Ok(false);
        }
        Ok(match self.config.kind {
            TranslationKind::Loop { from, to, .. } => from != to,
            TranslationKind::To {
                target,
                duration_millis,
            } => {
                !self.settled
                    && self.from != target
                    && self.elapsed(seconds) < f64::from(duration_millis) / 1000.
            }
        })
    }
    /// Validate first, then accept one complete configuration/environment turn.
    /// An old hidden/Reduce phase cannot admit an entrance transition.
    pub fn configure(
        &mut self,
        config: TranslationMotion,
        seconds: f64,
        allowed: bool,
    ) -> Result<(), Error> {
        config.validate()?;
        clock(seconds, self.started)?;
        if config == self.config && allowed == self.allowed {
            return Ok(());
        }
        let previous = self.offset(seconds)?;
        let previous_allowed = self.allowed;
        if config.kind == self.config.kind && allowed == previous_allowed {
            self.elapsed = self.elapsed(seconds);
        } else {
            self.from = previous;
            self.elapsed = 0.;
            self.settled = !previous_allowed || !allowed;
        }
        if allowed != previous_allowed {
            self.elapsed = 0.;
            self.from = config.resting_offset();
            self.settled = !matches!(config.kind, TranslationKind::Loop { .. });
        }
        self.config = config;
        self.started = seconds;
        self.allowed = allowed;
        Ok(())
    }
}
fn clock(seconds: f64, earliest: f64) -> Result<(), Error> {
    if !seconds.is_finite() || seconds < earliest {
        return Err(Error::InvalidProps(
            "translation clock must be finite, nonnegative and monotonic",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn to(x: f64) -> TranslationMotion {
        TranslationMotion {
            kind: TranslationKind::To {
                target: TranslationOffset::new(x, 0.),
                duration_millis: 1000,
            },
            paused: false,
        }
    }
    fn looping() -> TranslationMotion {
        TranslationMotion {
            kind: TranslationKind::Loop {
                from: TranslationOffset::ZERO,
                to: TranslationOffset::new(100., 0.),
                period_millis: 1000,
            },
            paused: false,
        }
    }
    #[test]
    fn finite_coordinates_durations_and_clock_are_independently_checked() {
        for value in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1_000_001.,
            1_000_001.,
        ] {
            for point in [
                TranslationOffset::new(value, 0.),
                TranslationOffset::new(0., value),
            ] {
                assert!(
                    TranslationMotion {
                        kind: TranslationKind::To {
                            target: point,
                            duration_millis: 0
                        },
                        paused: false
                    }
                    .validate()
                    .is_err()
                );
                for (from, to) in [
                    (point, TranslationOffset::ZERO),
                    (TranslationOffset::ZERO, point),
                ] {
                    assert!(
                        TranslationMotion {
                            kind: TranslationKind::Loop {
                                from,
                                to,
                                period_millis: 1000
                            },
                            paused: false
                        }
                        .validate()
                        .is_err()
                    );
                }
            }
        }
        for millis in [0, 15, 60_001, u32::MAX] {
            assert!(
                TranslationMotion {
                    kind: TranslationKind::Loop {
                        from: TranslationOffset::ZERO,
                        to: TranslationOffset::ZERO,
                        period_millis: millis
                    },
                    paused: false
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            TranslationMotion {
                kind: TranslationKind::To {
                    target: TranslationOffset::ZERO,
                    duration_millis: 2001
                },
                paused: false
            }
            .validate()
            .is_err()
        );
        for seconds in [-1., f64::NAN, f64::INFINITY] {
            assert!(TranslationPhase::new(to(0.), seconds, true).is_err());
        }
        let mut phase = TranslationPhase::new(to(0.), 1., true).unwrap();
        let before = phase.clone();
        assert!(phase.configure(to(f64::NAN), 2., true).is_err());
        assert_eq!(phase.offset(2.).unwrap(), before.offset(2.).unwrap());
        assert!(phase.configure(to(20.), 0.5, true).is_err());
    }
    #[test]
    fn once_first_static_interrupt_pause_clear_and_reduce_have_no_history() {
        let mut phase = TranslationPhase::new(to(10.), 0., true).unwrap();
        assert_eq!(phase.offset(0.5).unwrap().x, 10.);
        phase.configure(to(110.), 1., true).unwrap();
        assert_eq!(phase.offset(1.25).unwrap().x, 35.);
        phase
            .configure(
                TranslationMotion {
                    paused: true,
                    ..to(110.)
                },
                1.25,
                true,
            )
            .unwrap();
        assert_eq!(phase.offset(50.).unwrap().x, 35.);
        phase.configure(to(110.), 50., true).unwrap();
        assert_eq!(phase.offset(50.25).unwrap().x, 60.);
        phase.configure(to(-40.), 50.25, true).unwrap();
        assert_eq!(phase.offset(50.5).unwrap().x, 35.);
        phase.configure(to(-40.), 50.5, false).unwrap();
        assert_eq!(phase.offset(51.).unwrap().x, -40.);
        phase.configure(to(200.), 52., true).unwrap();
        assert_eq!(phase.offset(52.).unwrap().x, 200.);
        assert!(!phase.running(52.).unwrap());
        phase = TranslationPhase::new(to(0.), 53., true).unwrap();
        assert_eq!(phase.offset(53.).unwrap(), TranslationOffset::ZERO);
    }
    #[test]
    fn repeat_retains_paused_phase_and_restarts_only_for_explicit_new_endpoints() {
        let mut phase = TranslationPhase::new(looping(), 0., true).unwrap();
        assert_eq!(phase.offset(0.25).unwrap().x, 25.);
        phase.configure(looping(), 0.25, true).unwrap();
        assert_eq!(phase.offset(0.5).unwrap().x, 50.);
        phase
            .configure(
                TranslationMotion {
                    paused: true,
                    ..looping()
                },
                0.5,
                true,
            )
            .unwrap();
        assert_eq!(phase.offset(100.).unwrap().x, 50.);
        phase.configure(looping(), 100., true).unwrap();
        assert_eq!(phase.offset(100.25).unwrap().x, 75.);
        assert!(phase.offset(f64::MAX).unwrap().x.is_finite());
        phase.configure(looping(), 101., false).unwrap();
        assert_eq!(phase.offset(102.).unwrap(), TranslationOffset::ZERO);
        phase.configure(looping(), 103., true).unwrap();
        assert_eq!(phase.offset(103.25).unwrap().x, 25.);
        phase
            .configure(
                TranslationMotion {
                    kind: TranslationKind::Loop {
                        from: TranslationOffset::new(100., 0.),
                        to: TranslationOffset::ZERO,
                        period_millis: 1000,
                    },
                    paused: false,
                },
                104.,
                true,
            )
            .unwrap();
        assert_eq!(phase.offset(104.25).unwrap().x, 75.);
    }
}
