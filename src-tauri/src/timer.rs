use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerState {
    Idle,
    FocusRunning {
        accumulated_100ns: u64,
        started_at_100ns: u64,
    },
    FocusPaused {
        accumulated_100ns: u64,
        paused_at_100ns: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerError {
    InvalidTransition,
    TimeWentBackwards,
    DurationOverflow,
}

impl fmt::Display for TimerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransition => formatter.write_str("invalid timer transition"),
            Self::TimeWentBackwards => formatter.write_str("timer timestamp moved backwards"),
            Self::DurationOverflow => formatter.write_str("timer duration overflowed"),
        }
    }
}

impl Error for TimerError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusTimer {
    state: TimerState,
}

impl Default for FocusTimer {
    fn default() -> Self {
        Self {
            state: TimerState::Idle,
        }
    }
}

impl FocusTimer {
    pub fn from_state(state: TimerState) -> Self {
        Self { state }
    }

    pub fn state(&self) -> TimerState {
        self.state
    }

    pub fn start(&mut self, now_100ns: u64) -> Result<(), TimerError> {
        if self.state != TimerState::Idle {
            return Err(TimerError::InvalidTransition);
        }

        self.state = TimerState::FocusRunning {
            accumulated_100ns: 0,
            started_at_100ns: now_100ns,
        };
        Ok(())
    }

    pub fn pause(&mut self, now_100ns: u64) -> Result<(), TimerError> {
        let TimerState::FocusRunning {
            accumulated_100ns,
            started_at_100ns,
        } = self.state
        else {
            return Err(TimerError::InvalidTransition);
        };
        let elapsed = now_100ns
            .checked_sub(started_at_100ns)
            .ok_or(TimerError::TimeWentBackwards)?;
        let accumulated_100ns = accumulated_100ns
            .checked_add(elapsed)
            .ok_or(TimerError::DurationOverflow)?;

        self.state = TimerState::FocusPaused {
            accumulated_100ns,
            paused_at_100ns: now_100ns,
        };
        Ok(())
    }

    pub fn resume(&mut self, now_100ns: u64) -> Result<(), TimerError> {
        let TimerState::FocusPaused {
            accumulated_100ns,
            paused_at_100ns,
        } = self.state
        else {
            return Err(TimerError::InvalidTransition);
        };
        if now_100ns < paused_at_100ns {
            return Err(TimerError::TimeWentBackwards);
        }

        self.state = TimerState::FocusRunning {
            accumulated_100ns,
            started_at_100ns: now_100ns,
        };
        Ok(())
    }

    pub fn effective_at(&self, now_100ns: u64) -> Result<u64, TimerError> {
        match self.state {
            TimerState::Idle => Ok(0),
            TimerState::FocusPaused {
                accumulated_100ns, ..
            } => Ok(accumulated_100ns),
            TimerState::FocusRunning {
                accumulated_100ns,
                started_at_100ns,
            } => {
                let elapsed = now_100ns
                    .checked_sub(started_at_100ns)
                    .ok_or(TimerError::TimeWentBackwards)?;
                accumulated_100ns
                    .checked_add(elapsed)
                    .ok_or(TimerError::DurationOverflow)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FocusTimer, TimerError, TimerState};

    const MINUTE: u64 = 60 * 10_000_000;

    #[test]
    fn pauses_do_not_count_toward_effective_focus() {
        let mut timer = FocusTimer::default();
        timer.start(0).unwrap();
        timer.pause(10 * MINUTE).unwrap();
        timer.resume(15 * MINUTE).unwrap();

        assert_eq!(timer.effective_at(35 * MINUTE).unwrap(), 30 * MINUTE);
    }

    #[test]
    fn projection_uses_timestamp_instead_of_update_count() {
        let mut timer = FocusTimer::default();
        timer.start(100).unwrap();

        assert_eq!(timer.effective_at(500).unwrap(), 400);
        assert_eq!(timer.effective_at(5_000).unwrap(), 4_900);
    }

    #[test]
    fn repeated_or_out_of_order_commands_do_not_change_state() {
        let mut timer = FocusTimer::default();
        assert_eq!(timer.pause(0), Err(TimerError::InvalidTransition));

        timer.start(10).unwrap();
        let running = timer.state();
        assert_eq!(timer.start(20), Err(TimerError::InvalidTransition));
        assert_eq!(timer.state(), running);

        timer.pause(30).unwrap();
        let paused = timer.state();
        assert_eq!(timer.pause(40), Err(TimerError::InvalidTransition));
        assert_eq!(timer.state(), paused);
    }

    #[test]
    fn timestamps_cannot_move_backwards() {
        let mut timer = FocusTimer::default();
        timer.start(100).unwrap();

        assert_eq!(timer.effective_at(99), Err(TimerError::TimeWentBackwards));
        assert_eq!(timer.pause(99), Err(TimerError::TimeWentBackwards));
        assert_eq!(
            timer.state(),
            TimerState::FocusRunning {
                accumulated_100ns: 0,
                started_at_100ns: 100,
            }
        );
    }

    #[test]
    fn overflowing_duration_is_rejected_without_changing_state() {
        let mut timer = FocusTimer::from_state(TimerState::FocusRunning {
            accumulated_100ns: u64::MAX,
            started_at_100ns: 10,
        });

        assert_eq!(timer.pause(11), Err(TimerError::DurationOverflow));
        assert_eq!(
            timer.state(),
            TimerState::FocusRunning {
                accumulated_100ns: u64::MAX,
                started_at_100ns: 10,
            }
        );
    }
}
