use std::{
    error::Error,
    fmt,
    sync::{Mutex, MutexGuard},
};

use serde::Serialize;

use crate::{
    clock::{Clock, SystemClock},
    timer::{FocusTimer, TimerError, TimerState},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TimerPhase {
    Idle,
    FocusRunning,
    FocusPaused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerSnapshot {
    pub revision: u32,
    pub phase: TimerPhase,
    pub focus_elapsed_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceError {
    Clock(String),
    Timer(TimerError),
    StaleRevision { expected: u32, actual: u32 },
    RevisionOverflow,
    StateUnavailable,
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clock(error) => write!(formatter, "clock unavailable: {error}"),
            Self::Timer(error) => error.fmt(formatter),
            Self::StaleRevision { expected, actual } => write!(
                formatter,
                "stale timer revision: expected {expected}, current {actual}"
            ),
            Self::RevisionOverflow => formatter.write_str("timer revision overflowed"),
            Self::StateUnavailable => formatter.write_str("timer state is unavailable"),
        }
    }
}

impl Error for ServiceError {}

impl From<TimerError> for ServiceError {
    fn from(error: TimerError) -> Self {
        Self::Timer(error)
    }
}

struct ServiceState {
    timer: FocusTimer,
    revision: u32,
}

pub struct TimerService<C: Clock> {
    clock: C,
    state: Mutex<ServiceState>,
}

pub type AppTimerService = TimerService<SystemClock>;

impl<C: Clock> TimerService<C> {
    pub fn new(clock: C) -> Self {
        Self {
            clock,
            state: Mutex::new(ServiceState {
                timer: FocusTimer::default(),
                revision: 0,
            }),
        }
    }

    pub fn snapshot(&self) -> Result<TimerSnapshot, ServiceError> {
        let state = self.lock_state()?;
        let now_100ns = self.now_100ns()?;
        snapshot_at(&state, now_100ns)
    }

    pub fn start_focus(&self, expected_revision: u32) -> Result<TimerSnapshot, ServiceError> {
        self.transition(expected_revision, FocusTimer::start)
    }

    pub fn pause_focus(&self, expected_revision: u32) -> Result<TimerSnapshot, ServiceError> {
        self.transition(expected_revision, FocusTimer::pause)
    }

    pub fn resume_focus(&self, expected_revision: u32) -> Result<TimerSnapshot, ServiceError> {
        self.transition(expected_revision, FocusTimer::resume)
    }

    fn transition(
        &self,
        expected_revision: u32,
        action: fn(&mut FocusTimer, u64) -> Result<(), TimerError>,
    ) -> Result<TimerSnapshot, ServiceError> {
        let mut state = self.lock_state()?;
        if state.revision != expected_revision {
            return Err(ServiceError::StaleRevision {
                expected: expected_revision,
                actual: state.revision,
            });
        }

        let now_100ns = self.now_100ns()?;
        action(&mut state.timer, now_100ns)?;
        state.revision = state
            .revision
            .checked_add(1)
            .ok_or(ServiceError::RevisionOverflow)?;
        snapshot_at(&state, now_100ns)
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, ServiceState>, ServiceError> {
        self.state
            .lock()
            .map_err(|_| ServiceError::StateUnavailable)
    }

    fn now_100ns(&self) -> Result<u64, ServiceError> {
        self.clock
            .now_100ns()
            .map_err(|error| ServiceError::Clock(error.to_string()))
    }

    #[cfg(test)]
    fn revision_for_test(&self) -> u32 {
        self.state.lock().unwrap().revision
    }
}

fn snapshot_at(state: &ServiceState, now_100ns: u64) -> Result<TimerSnapshot, ServiceError> {
    let phase = match state.timer.state() {
        TimerState::Idle => TimerPhase::Idle,
        TimerState::FocusRunning { .. } => TimerPhase::FocusRunning,
        TimerState::FocusPaused { .. } => TimerPhase::FocusPaused,
    };
    Ok(TimerSnapshot {
        revision: state.revision,
        phase,
        focus_elapsed_ms: state.timer.effective_at(now_100ns)? / 10_000,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::{
            atomic::{AtomicBool, AtomicU64, Ordering},
            Arc,
        },
    };

    use crate::clock::Clock;

    use super::{ServiceError, TimerPhase, TimerService};

    #[derive(Clone, Default)]
    struct FakeClock {
        now: Arc<AtomicU64>,
        failing: Arc<AtomicBool>,
    }

    impl FakeClock {
        fn set(&self, value: u64) {
            self.now.store(value, Ordering::Relaxed);
        }

        fn fail(&self) {
            self.failing.store(true, Ordering::Relaxed);
        }
    }

    impl Clock for FakeClock {
        fn now_100ns(&self) -> io::Result<u64> {
            if self.failing.load(Ordering::Relaxed) {
                Err(io::Error::other("clock failed"))
            } else {
                Ok(self.now.load(Ordering::Relaxed))
            }
        }
    }

    #[test]
    fn transitions_increment_revision_and_snapshot_real_elapsed_time() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());

        let started = service.start_focus(0).unwrap();
        assert_eq!(started.revision, 1);
        assert_eq!(started.phase, TimerPhase::FocusRunning);

        clock.set(50_000);
        let snapshot = service.snapshot().unwrap();
        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.focus_elapsed_ms, 5);
    }

    #[test]
    fn stale_action_is_rejected_without_changing_state() {
        let service = TimerService::new(FakeClock::default());
        service.start_focus(0).unwrap();

        assert_eq!(
            service.pause_focus(0),
            Err(ServiceError::StaleRevision {
                expected: 0,
                actual: 1,
            })
        );
        assert_eq!(service.snapshot().unwrap().phase, TimerPhase::FocusRunning);
    }

    #[test]
    fn clock_failure_blocks_a_transition() {
        let clock = FakeClock::default();
        clock.fail();
        let service = TimerService::new(clock);

        assert!(matches!(
            service.start_focus(0),
            Err(ServiceError::Clock(_))
        ));
        assert_eq!(service.revision_for_test(), 0);
    }
}
