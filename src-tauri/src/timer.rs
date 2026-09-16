use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusRatio {
    Three,
    Four,
    Five,
}

impl FocusRatio {
    pub fn value(self) -> u8 {
        match self {
            Self::Three => 3,
            Self::Four => 4,
            Self::Five => 5,
        }
    }
}

impl TryFrom<u8> for FocusRatio {
    type Error = TimerError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            3 => Ok(Self::Three),
            4 => Ok(Self::Four),
            5 => Ok(Self::Five),
            _ => Err(TimerError::InvalidRatio),
        }
    }
}

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

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PersistedFocusPhase {
    Idle,
    Running,
    Paused,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedTimer {
    pub phase: PersistedFocusPhase,
    pub elapsed_100ns: u64,
    pub ratio: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerError {
    InvalidTransition,
    InvalidRatio,
    InvalidPersistedState,
    TimeWentBackwards,
    DurationOverflow,
}

impl fmt::Display for TimerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransition => formatter.write_str("invalid timer transition"),
            Self::InvalidRatio => formatter.write_str("unsupported focus ratio"),
            Self::InvalidPersistedState => formatter.write_str("invalid persisted timer state"),
            Self::TimeWentBackwards => formatter.write_str("timer timestamp moved backwards"),
            Self::DurationOverflow => formatter.write_str("timer duration overflowed"),
        }
    }
}

impl Error for TimerError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusTimer {
    state: TimerState,
    ratio: FocusRatio,
}

impl Default for FocusTimer {
    fn default() -> Self {
        Self {
            state: TimerState::Idle,
            ratio: FocusRatio::Three,
        }
    }
}

impl FocusTimer {
    pub fn from_state(state: TimerState) -> Self {
        Self {
            state,
            ratio: FocusRatio::Three,
        }
    }

    pub fn state(&self) -> TimerState {
        self.state
    }

    pub fn restore(persisted: PersistedTimer, now_100ns: u64) -> Result<Self, TimerError> {
        let ratio = FocusRatio::try_from(persisted.ratio)?;
        let state = match persisted.phase {
            PersistedFocusPhase::Idle if persisted.elapsed_100ns == 0 => TimerState::Idle,
            PersistedFocusPhase::Idle => return Err(TimerError::InvalidPersistedState),
            PersistedFocusPhase::Running | PersistedFocusPhase::Paused => TimerState::FocusPaused {
                accumulated_100ns: persisted.elapsed_100ns,
                paused_at_100ns: now_100ns,
            },
        };

        Ok(Self { state, ratio })
    }

    pub fn persisted_at(&self, now_100ns: u64) -> Result<PersistedTimer, TimerError> {
        let phase = match self.state {
            TimerState::Idle => PersistedFocusPhase::Idle,
            TimerState::FocusRunning { .. } => PersistedFocusPhase::Running,
            TimerState::FocusPaused { .. } => PersistedFocusPhase::Paused,
        };

        Ok(PersistedTimer {
            phase,
            elapsed_100ns: self.effective_at(now_100ns)?,
            ratio: self.ratio.value(),
        })
    }

    pub fn ratio(&self) -> FocusRatio {
        self.ratio
    }

    pub fn set_ratio(&mut self, ratio: FocusRatio) {
        self.ratio = ratio;
    }

    pub fn earned_break_at(&self, now_100ns: u64) -> Result<u64, TimerError> {
        Ok(self.effective_at(now_100ns)? / u64::from(self.ratio.value()))
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
    use super::{
        FocusRatio, FocusTimer, PersistedFocusPhase, PersistedTimer, TimerError, TimerState,
    };

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

    #[test]
    fn calculates_earned_break_for_each_supported_ratio() {
        let mut timer = FocusTimer::default();
        timer.start(0).unwrap();
        timer.pause(60 * MINUTE).unwrap();

        assert_eq!(timer.earned_break_at(60 * MINUTE).unwrap(), 20 * MINUTE);
        timer.set_ratio(FocusRatio::Four);
        assert_eq!(timer.earned_break_at(60 * MINUTE).unwrap(), 15 * MINUTE);
        timer.set_ratio(FocusRatio::Five);
        assert_eq!(timer.earned_break_at(60 * MINUTE).unwrap(), 12 * MINUTE);
    }

    #[test]
    fn changing_ratio_recalculates_the_entire_current_focus() {
        let mut timer = FocusTimer::default();
        timer.start(0).unwrap();
        timer.pause(30 * MINUTE).unwrap();
        assert_eq!(timer.earned_break_at(30 * MINUTE).unwrap(), 10 * MINUTE);

        timer.set_ratio(FocusRatio::Five);

        assert_eq!(timer.earned_break_at(30 * MINUTE).unwrap(), 6 * MINUTE);
    }

    #[test]
    fn accepts_only_supported_ratio_values() {
        assert_eq!(FocusRatio::try_from(3), Ok(FocusRatio::Three));
        assert_eq!(FocusRatio::try_from(4), Ok(FocusRatio::Four));
        assert_eq!(FocusRatio::try_from(5), Ok(FocusRatio::Five));
        assert_eq!(FocusRatio::try_from(2), Err(TimerError::InvalidRatio));
    }

    #[test]
    fn restores_a_running_session_as_paused_at_the_new_clock() {
        let persisted = PersistedTimer {
            phase: PersistedFocusPhase::Running,
            elapsed_100ns: 12_345,
            ratio: 5,
        };

        let timer = FocusTimer::restore(persisted, 900).unwrap();

        assert_eq!(timer.ratio(), FocusRatio::Five);
        assert_eq!(timer.effective_at(900).unwrap(), 12_345);
        assert_eq!(
            timer.state(),
            TimerState::FocusPaused {
                accumulated_100ns: 12_345,
                paused_at_100ns: 900,
            }
        );
    }

    #[test]
    fn persists_only_consolidated_duration_not_the_process_anchor() {
        let mut timer = FocusTimer::default();
        timer.start(100).unwrap();

        assert_eq!(
            timer.persisted_at(500).unwrap(),
            PersistedTimer {
                phase: PersistedFocusPhase::Running,
                elapsed_100ns: 400,
                ratio: 3,
            }
        );
    }

    #[test]
    fn rejects_incoherent_persisted_idle_state() {
        let result = FocusTimer::restore(
            PersistedTimer {
                phase: PersistedFocusPhase::Idle,
                elapsed_100ns: 1,
                ratio: 3,
            },
            0,
        );

        assert_eq!(result, Err(TimerError::InvalidPersistedState));
    }
}
