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
    BreakReady {
        focus_elapsed_100ns: u64,
        break_earned_100ns: i128,
    },
    BreakRunning {
        focus_elapsed_100ns: u64,
        break_earned_100ns: i128,
        remaining_at_start_100ns: i128,
        started_at_100ns: u64,
    },
    BreakPaused {
        focus_elapsed_100ns: u64,
        break_earned_100ns: i128,
        remaining_100ns: i128,
    },
    BreakCompleted {
        focus_elapsed_100ns: u64,
        break_earned_100ns: i128,
        remaining_100ns: i128,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PersistedFocusPhase {
    Idle,
    Running,
    Paused,
    BreakReady,
    BreakRunning,
    BreakPaused,
    BreakCompleted,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedTimer {
    pub phase: PersistedFocusPhase,
    pub elapsed_100ns: u64,
    pub ratio: u8,
    #[serde(default)]
    pub break_earned_100ns: i128,
    #[serde(default)]
    pub break_remaining_100ns: i128,
    #[serde(default)]
    pub break_elapsed_100ns: u64,
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
    break_balance_100ns: i128,
    break_elapsed_100ns: u64,
    focus_balance_anchor_100ns: u64,
    break_elapsed_anchor_100ns: Option<u64>,
}

impl Default for FocusTimer {
    fn default() -> Self {
        Self {
            state: TimerState::Idle,
            ratio: FocusRatio::Three,
            break_balance_100ns: 0,
            break_elapsed_100ns: 0,
            focus_balance_anchor_100ns: 0,
            break_elapsed_anchor_100ns: None,
        }
    }
}

impl FocusTimer {
    pub fn from_state(state: TimerState) -> Self {
        Self {
            state,
            ratio: FocusRatio::Three,
            break_balance_100ns: 0,
            break_elapsed_100ns: 0,
            focus_balance_anchor_100ns: 0,
            break_elapsed_anchor_100ns: None,
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
            PersistedFocusPhase::BreakReady
                if persisted.elapsed_100ns > 0 && persisted.break_earned_100ns > 0 =>
            {
                TimerState::BreakReady {
                    focus_elapsed_100ns: persisted.elapsed_100ns,
                    break_earned_100ns: persisted.break_earned_100ns,
                }
            }
            PersistedFocusPhase::BreakReady => return Err(TimerError::InvalidPersistedState),
            PersistedFocusPhase::BreakRunning | PersistedFocusPhase::BreakPaused
                if persisted.elapsed_100ns > 0
                    && persisted.break_earned_100ns > 0
                    && persisted.break_remaining_100ns <= persisted.break_earned_100ns =>
            {
                TimerState::BreakPaused {
                    focus_elapsed_100ns: persisted.elapsed_100ns,
                    break_earned_100ns: persisted.break_earned_100ns,
                    remaining_100ns: persisted.break_remaining_100ns,
                }
            }
            PersistedFocusPhase::BreakCompleted
                if persisted.elapsed_100ns > 0
                    && persisted.break_earned_100ns > 0
                    && persisted.break_remaining_100ns <= persisted.break_earned_100ns =>
            {
                TimerState::BreakCompleted {
                    focus_elapsed_100ns: persisted.elapsed_100ns,
                    break_earned_100ns: persisted.break_earned_100ns,
                    remaining_100ns: persisted.break_remaining_100ns,
                }
            }
            PersistedFocusPhase::BreakRunning
            | PersistedFocusPhase::BreakPaused
            | PersistedFocusPhase::BreakCompleted => return Err(TimerError::InvalidPersistedState),
        };

        Ok(Self {
            state,
            ratio,
            break_balance_100ns: persisted.break_earned_100ns,
            break_elapsed_100ns: persisted.break_elapsed_100ns,
            focus_balance_anchor_100ns: persisted.elapsed_100ns,
            break_elapsed_anchor_100ns: None,
        })
    }

    pub fn persisted_at(&self, now_100ns: u64) -> Result<PersistedTimer, TimerError> {
        let phase = match self.state {
            TimerState::Idle => PersistedFocusPhase::Idle,
            TimerState::FocusRunning { .. } => PersistedFocusPhase::Running,
            TimerState::FocusPaused { .. } => PersistedFocusPhase::Paused,
            TimerState::BreakReady { .. } => PersistedFocusPhase::BreakReady,
            TimerState::BreakRunning { .. } => PersistedFocusPhase::BreakRunning,
            TimerState::BreakPaused { .. } => PersistedFocusPhase::BreakPaused,
            TimerState::BreakCompleted { .. } => PersistedFocusPhase::BreakCompleted,
        };

        Ok(PersistedTimer {
            phase,
            elapsed_100ns: self.effective_at(now_100ns)?,
            ratio: self.ratio.value(),
            break_earned_100ns: self.earned_break_at(now_100ns)?,
            break_remaining_100ns: self.break_remaining_at(now_100ns)?,
            break_elapsed_100ns: self.break_elapsed_at(now_100ns)?,
        })
    }

    pub fn ratio(&self) -> FocusRatio {
        self.ratio
    }

    pub fn set_ratio(&mut self, ratio: FocusRatio) {
        self.ratio = ratio;
        if let TimerState::FocusPaused {
            accumulated_100ns, ..
        } = self.state
        {
            self.break_balance_100ns = i128::from(accumulated_100ns) / i128::from(ratio.value());
            self.focus_balance_anchor_100ns = accumulated_100ns;
        }
    }

    pub fn set_ratio_at(&mut self, ratio: FocusRatio, now_100ns: u64) -> Result<(), TimerError> {
        if matches!(self.state, TimerState::FocusRunning { .. }) {
            let focus_elapsed_100ns = self.effective_at(now_100ns)?;
            self.ratio = ratio;
            self.break_balance_100ns = i128::from(focus_elapsed_100ns) / i128::from(ratio.value());
            self.focus_balance_anchor_100ns = focus_elapsed_100ns;
        } else {
            self.set_ratio(ratio);
        }
        Ok(())
    }

    pub fn start(&mut self, now_100ns: u64) -> Result<(), TimerError> {
        if !matches!(
            self.state,
            TimerState::Idle | TimerState::BreakCompleted { .. }
        ) {
            return Err(TimerError::InvalidTransition);
        }

        let focus_elapsed_100ns = self.effective_at(now_100ns)?;
        self.break_balance_100ns = self.break_remaining_at(now_100ns)?;
        self.focus_balance_anchor_100ns = focus_elapsed_100ns;

        self.state = TimerState::FocusRunning {
            accumulated_100ns: focus_elapsed_100ns,
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
        self.break_balance_100ns = self.earned_break_at(now_100ns)?;
        self.focus_balance_anchor_100ns = accumulated_100ns;

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
        self.focus_balance_anchor_100ns = accumulated_100ns;
        Ok(())
    }

    pub fn finish(&mut self, now_100ns: u64) -> Result<(), TimerError> {
        let focus_elapsed_100ns = match self.state {
            TimerState::FocusRunning { .. } | TimerState::FocusPaused { .. } => {
                self.effective_at(now_100ns)?
            }
            _ => return Err(TimerError::InvalidTransition),
        };
        if focus_elapsed_100ns == 0 {
            self.state = TimerState::Idle;
            return Ok(());
        }
        let break_earned_100ns = self.earned_break_at(now_100ns)?;
        self.break_balance_100ns = break_earned_100ns;

        self.state = TimerState::BreakReady {
            focus_elapsed_100ns,
            break_earned_100ns,
        };
        Ok(())
    }

    pub fn reset(&mut self, now_100ns: u64) -> Result<(), TimerError> {
        if matches!(self.state, TimerState::Idle) {
            return Err(TimerError::InvalidTransition);
        }
        self.effective_at(now_100ns)?;
        self.state = TimerState::Idle;
        self.break_balance_100ns = 0;
        self.break_elapsed_100ns = 0;
        self.focus_balance_anchor_100ns = 0;
        self.break_elapsed_anchor_100ns = None;
        Ok(())
    }

    pub fn start_break(&mut self, now_100ns: u64) -> Result<(), TimerError> {
        let TimerState::BreakReady {
            focus_elapsed_100ns,
            break_earned_100ns,
        } = self.state
        else {
            return Err(TimerError::InvalidTransition);
        };
        if break_earned_100ns <= 0 {
            self.state = TimerState::BreakCompleted {
                focus_elapsed_100ns,
                break_earned_100ns,
                remaining_100ns: break_earned_100ns,
            };
        } else {
            self.state = TimerState::BreakRunning {
                focus_elapsed_100ns,
                break_earned_100ns,
                remaining_at_start_100ns: break_earned_100ns,
                started_at_100ns: now_100ns,
            };
            self.break_elapsed_anchor_100ns = Some(now_100ns);
        }
        Ok(())
    }

    pub fn pause_break(&mut self, now_100ns: u64) -> Result<(), TimerError> {
        let TimerState::BreakRunning {
            focus_elapsed_100ns,
            break_earned_100ns,
            ..
        } = self.state
        else {
            return Err(TimerError::InvalidTransition);
        };
        let remaining_100ns = self.break_remaining_at(now_100ns)?;
        let break_elapsed_100ns = self.break_elapsed_at(now_100ns)?;
        self.state = if remaining_100ns <= 0 {
            TimerState::BreakCompleted {
                focus_elapsed_100ns,
                break_earned_100ns,
                remaining_100ns,
            }
        } else {
            TimerState::BreakPaused {
                focus_elapsed_100ns,
                break_earned_100ns,
                remaining_100ns,
            }
        };
        self.break_elapsed_100ns = break_elapsed_100ns;
        self.break_elapsed_anchor_100ns = None;
        Ok(())
    }

    pub fn resume_break(&mut self, now_100ns: u64) -> Result<(), TimerError> {
        let TimerState::BreakPaused {
            focus_elapsed_100ns,
            break_earned_100ns,
            remaining_100ns,
        } = self.state
        else {
            return Err(TimerError::InvalidTransition);
        };
        self.state = TimerState::BreakRunning {
            focus_elapsed_100ns,
            break_earned_100ns,
            remaining_at_start_100ns: remaining_100ns,
            started_at_100ns: now_100ns,
        };
        self.break_elapsed_anchor_100ns = Some(now_100ns);
        Ok(())
    }

    pub fn end_break(&mut self, now_100ns: u64) -> Result<(), TimerError> {
        let remaining_100ns = self.break_remaining_at(now_100ns)?;
        let break_elapsed_100ns = self.break_elapsed_at(now_100ns)?;
        let (focus_elapsed_100ns, break_earned_100ns) = match self.state {
            TimerState::BreakReady {
                focus_elapsed_100ns,
                break_earned_100ns,
            }
            | TimerState::BreakRunning {
                focus_elapsed_100ns,
                break_earned_100ns,
                ..
            }
            | TimerState::BreakPaused {
                focus_elapsed_100ns,
                break_earned_100ns,
                ..
            } => (focus_elapsed_100ns, break_earned_100ns),
            _ => return Err(TimerError::InvalidTransition),
        };
        self.state = TimerState::BreakCompleted {
            focus_elapsed_100ns,
            break_earned_100ns,
            remaining_100ns,
        };
        self.break_elapsed_100ns = break_elapsed_100ns;
        self.break_elapsed_anchor_100ns = None;
        Ok(())
    }

    pub fn normalize_break(&mut self, now_100ns: u64) -> Result<bool, TimerError> {
        let TimerState::BreakRunning {
            focus_elapsed_100ns,
            ..
        } = self.state
        else {
            return Ok(false);
        };
        let remaining_100ns = self.break_remaining_at(now_100ns)?;
        if remaining_100ns > 0 {
            return Ok(false);
        }
        let break_elapsed_100ns = self.break_elapsed_at(now_100ns)?;
        self.break_balance_100ns = remaining_100ns;
        self.focus_balance_anchor_100ns = focus_elapsed_100ns;
        self.state = TimerState::FocusRunning {
            accumulated_100ns: focus_elapsed_100ns,
            started_at_100ns: now_100ns,
        };
        self.break_elapsed_100ns = break_elapsed_100ns;
        self.break_elapsed_anchor_100ns = None;
        Ok(true)
    }

    pub fn effective_at(&self, now_100ns: u64) -> Result<u64, TimerError> {
        match self.state {
            TimerState::Idle => Ok(0),
            TimerState::FocusPaused {
                accumulated_100ns, ..
            } => Ok(accumulated_100ns),
            TimerState::BreakReady {
                focus_elapsed_100ns,
                ..
            } => Ok(focus_elapsed_100ns),
            TimerState::BreakRunning {
                focus_elapsed_100ns,
                ..
            }
            | TimerState::BreakPaused {
                focus_elapsed_100ns,
                ..
            }
            | TimerState::BreakCompleted {
                focus_elapsed_100ns,
                ..
            } => Ok(focus_elapsed_100ns),
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

    pub fn earned_break_at(&self, now_100ns: u64) -> Result<i128, TimerError> {
        match self.state {
            TimerState::BreakReady {
                break_earned_100ns, ..
            }
            | TimerState::BreakRunning {
                break_earned_100ns, ..
            }
            | TimerState::BreakPaused {
                break_earned_100ns, ..
            }
            | TimerState::BreakCompleted {
                break_earned_100ns, ..
            } => Ok(break_earned_100ns),
            TimerState::FocusRunning { .. } => {
                let focus_elapsed = self.effective_at(now_100ns)?;
                let new_earned = i128::from(
                    focus_elapsed
                        .checked_sub(self.focus_balance_anchor_100ns)
                        .ok_or(TimerError::TimeWentBackwards)?,
                ) / i128::from(self.ratio.value());
                self.break_balance_100ns
                    .checked_add(new_earned)
                    .ok_or(TimerError::DurationOverflow)
            }
            TimerState::FocusPaused { .. } => Ok(self.break_balance_100ns),
            TimerState::Idle => Ok(0),
        }
    }

    pub fn break_remaining_at(&self, now_100ns: u64) -> Result<i128, TimerError> {
        match self.state {
            TimerState::BreakReady {
                break_earned_100ns, ..
            } => Ok(break_earned_100ns),
            TimerState::BreakRunning {
                remaining_at_start_100ns,
                started_at_100ns,
                ..
            } => Ok(remaining_at_start_100ns
                - i128::from(
                    now_100ns
                        .checked_sub(started_at_100ns)
                        .ok_or(TimerError::TimeWentBackwards)?,
                )),
            TimerState::BreakPaused {
                remaining_100ns, ..
            } => Ok(remaining_100ns),
            TimerState::BreakCompleted {
                remaining_100ns, ..
            } => Ok(remaining_100ns),
            TimerState::Idle | TimerState::FocusRunning { .. } | TimerState::FocusPaused { .. } => {
                Ok(0)
            }
        }
    }

    pub fn break_elapsed_at(&self, now_100ns: u64) -> Result<u64, TimerError> {
        let active = match self.break_elapsed_anchor_100ns {
            Some(started_at) => now_100ns
                .checked_sub(started_at)
                .ok_or(TimerError::TimeWentBackwards)?,
            None => 0,
        };
        self.break_elapsed_100ns
            .checked_add(active)
            .ok_or(TimerError::DurationOverflow)
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

        assert_eq!(
            timer.earned_break_at(60 * MINUTE).unwrap(),
            i128::from(20 * MINUTE)
        );
        timer.set_ratio(FocusRatio::Four);
        assert_eq!(
            timer.earned_break_at(60 * MINUTE).unwrap(),
            i128::from(15 * MINUTE)
        );
        timer.set_ratio(FocusRatio::Five);
        assert_eq!(
            timer.earned_break_at(60 * MINUTE).unwrap(),
            i128::from(12 * MINUTE)
        );
    }

    #[test]
    fn changing_ratio_recalculates_the_entire_current_focus() {
        let mut timer = FocusTimer::default();
        timer.start(0).unwrap();
        timer.pause(30 * MINUTE).unwrap();
        assert_eq!(
            timer.earned_break_at(30 * MINUTE).unwrap(),
            i128::from(10 * MINUTE)
        );

        timer.set_ratio(FocusRatio::Five);

        assert_eq!(
            timer.earned_break_at(30 * MINUTE).unwrap(),
            i128::from(6 * MINUTE)
        );
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
            break_earned_100ns: 0,
            break_remaining_100ns: 0,
            break_elapsed_100ns: 0,
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
                break_earned_100ns: 133,
                break_remaining_100ns: 0,
                break_elapsed_100ns: 0,
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
                break_earned_100ns: 0,
                break_remaining_100ns: 0,
                break_elapsed_100ns: 0,
            },
            0,
        );

        assert_eq!(result, Err(TimerError::InvalidPersistedState));
    }

    #[test]
    fn finishing_focus_freezes_its_duration_and_earned_break() {
        let mut timer = FocusTimer::default();
        timer.start(0).unwrap();

        timer.finish(30 * MINUTE).unwrap();

        assert_eq!(
            timer.state(),
            TimerState::BreakReady {
                focus_elapsed_100ns: 30 * MINUTE,
                break_earned_100ns: i128::from(10 * MINUTE),
            }
        );
        assert_eq!(timer.effective_at(60 * MINUTE).unwrap(), 30 * MINUTE);
        assert_eq!(
            timer.earned_break_at(60 * MINUTE).unwrap(),
            i128::from(10 * MINUTE)
        );
    }

    #[test]
    fn resetting_focus_discards_elapsed_time_and_break() {
        let mut timer = FocusTimer::default();
        timer.start(0).unwrap();

        timer.reset(30 * MINUTE).unwrap();

        assert_eq!(timer.state(), TimerState::Idle);
        assert_eq!(timer.effective_at(30 * MINUTE).unwrap(), 0);
        assert_eq!(timer.earned_break_at(30 * MINUTE).unwrap(), 0);
    }

    #[test]
    fn resetting_paused_focus_discards_its_consolidated_time() {
        let mut timer = FocusTimer::default();
        timer.start(0).unwrap();
        timer.pause(30 * MINUTE).unwrap();

        timer.reset(45 * MINUTE).unwrap();

        assert_eq!(timer.state(), TimerState::Idle);
    }

    #[test]
    fn break_counts_down_only_after_manual_start_and_starts_focus_on_expiry() {
        let mut timer = FocusTimer::default();
        timer.start(0).unwrap();
        timer.finish(30 * MINUTE).unwrap();

        timer.start_break(45 * MINUTE).unwrap();
        assert_eq!(
            timer.break_remaining_at(50 * MINUTE).unwrap(),
            i128::from(5 * MINUTE)
        );
        timer.pause_break(50 * MINUTE).unwrap();
        assert_eq!(
            timer.break_remaining_at(70 * MINUTE).unwrap(),
            i128::from(5 * MINUTE)
        );
        timer.resume_break(70 * MINUTE).unwrap();
        assert_eq!(timer.break_remaining_at(75 * MINUTE).unwrap(), 0);
        assert!(timer.normalize_break(75 * MINUTE).unwrap());
        assert!(matches!(timer.state(), TimerState::FocusRunning { .. }));
        assert_eq!(timer.effective_at(75 * MINUTE).unwrap(), 30 * MINUTE);
        assert_eq!(timer.break_remaining_at(75 * MINUTE).unwrap(), 0);
    }

    #[test]
    fn ending_break_discards_the_remainder_without_erasing_focus() {
        let mut timer = FocusTimer::default();
        timer.start(0).unwrap();
        timer.finish(30 * MINUTE).unwrap();
        timer.start_break(30 * MINUTE).unwrap();

        timer.end_break(32 * MINUTE).unwrap();

        assert!(matches!(timer.state(), TimerState::BreakCompleted { .. }));
        assert_eq!(timer.effective_at(32 * MINUTE).unwrap(), 30 * MINUTE);
        assert_eq!(
            timer.break_remaining_at(32 * MINUTE).unwrap(),
            i128::from(8 * MINUTE)
        );
    }
}
