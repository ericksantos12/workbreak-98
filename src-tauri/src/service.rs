use std::{
    error::Error,
    fmt,
    sync::{Mutex, MutexGuard},
};

use serde::{Deserialize, Serialize};

use crate::{
    clock::{Clock, SystemClock},
    storage::{JsonStorage, LoadResult},
    timer::{FocusRatio, FocusTimer, PersistedTimer, TimerError, TimerState},
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
pub enum PersistenceStatus {
    Available,
    Unavailable,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerSnapshot {
    pub revision: u32,
    pub phase: TimerPhase,
    pub focus_elapsed_ms: u64,
    pub ratio: u8,
    pub break_earned_ms: u64,
    pub persistence_status: PersistenceStatus,
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
    persistence_status: PersistenceStatus,
}

pub struct TimerService<C: Clock> {
    clock: C,
    state: Mutex<ServiceState>,
    storage: Option<JsonStorage>,
}

pub type AppTimerService = TimerService<SystemClock>;

impl<C: Clock> TimerService<C> {
    pub fn new(clock: C) -> Self {
        Self {
            clock,
            state: Mutex::new(ServiceState {
                timer: FocusTimer::default(),
                revision: 0,
                persistence_status: PersistenceStatus::Unavailable,
            }),
            storage: None,
        }
    }

    pub fn from_storage(clock: C, storage: JsonStorage) -> Self {
        let (timer, revision, persistence_status) = match storage.load::<AppDocument>() {
            Ok(LoadResult::Missing) => (FocusTimer::default(), 0, PersistenceStatus::Available),
            Ok(LoadResult::Loaded(document)) if document.revision < u32::MAX => {
                match clock
                    .now_100ns()
                    .ok()
                    .and_then(|now| FocusTimer::restore(document.timer, now).ok())
                {
                    Some(timer) => (timer, document.revision, PersistenceStatus::Available),
                    None => (FocusTimer::default(), 0, PersistenceStatus::Blocked),
                }
            }
            Ok(LoadResult::Loaded(_))
            | Ok(LoadResult::Invalid(_))
            | Ok(LoadResult::UnsupportedVersion(_)) => {
                (FocusTimer::default(), 0, PersistenceStatus::Blocked)
            }
            Err(_) => (FocusTimer::default(), 0, PersistenceStatus::Unavailable),
        };

        Self {
            clock,
            state: Mutex::new(ServiceState {
                timer,
                revision,
                persistence_status,
            }),
            storage: Some(storage),
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

    pub fn set_ratio(
        &self,
        expected_revision: u32,
        ratio: FocusRatio,
    ) -> Result<TimerSnapshot, ServiceError> {
        let mut state = self.lock_state()?;
        validate_revision(&state, expected_revision)?;
        let next_revision = next_revision(&state)?;
        let now_100ns = self.now_100ns()?;

        state.timer.set_ratio(ratio);
        state.revision = next_revision;
        self.persist_at(&mut state, now_100ns);
        snapshot_at(&state, now_100ns)
    }

    fn transition(
        &self,
        expected_revision: u32,
        action: fn(&mut FocusTimer, u64) -> Result<(), TimerError>,
    ) -> Result<TimerSnapshot, ServiceError> {
        let mut state = self.lock_state()?;
        validate_revision(&state, expected_revision)?;
        let next_revision = next_revision(&state)?;
        let now_100ns = self.now_100ns()?;
        action(&mut state.timer, now_100ns)?;
        state.revision = next_revision;
        self.persist_at(&mut state, now_100ns);
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

    fn persist_at(&self, state: &mut ServiceState, now_100ns: u64) {
        if state.persistence_status == PersistenceStatus::Blocked {
            return;
        }
        let Some(storage) = &self.storage else {
            state.persistence_status = PersistenceStatus::Unavailable;
            return;
        };
        let Ok(timer) = state.timer.persisted_at(now_100ns) else {
            state.persistence_status = PersistenceStatus::Unavailable;
            return;
        };
        let document = AppDocument {
            revision: state.revision,
            timer,
        };
        state.persistence_status = if storage.write(&document).is_ok() {
            PersistenceStatus::Available
        } else {
            PersistenceStatus::Unavailable
        };
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
        ratio: state.timer.ratio().value(),
        break_earned_ms: state.timer.earned_break_at(now_100ns)? / 10_000,
        persistence_status: state.persistence_status,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppDocument {
    revision: u32,
    timer: PersistedTimer,
}

fn validate_revision(state: &ServiceState, expected: u32) -> Result<(), ServiceError> {
    if state.revision == expected {
        Ok(())
    } else {
        Err(ServiceError::StaleRevision {
            expected,
            actual: state.revision,
        })
    }
}

fn next_revision(state: &ServiceState) -> Result<u32, ServiceError> {
    state
        .revision
        .checked_add(1)
        .ok_or(ServiceError::RevisionOverflow)
}

#[cfg(test)]
mod tests {
    use std::{
        fs, io,
        path::PathBuf,
        sync::{
            atomic::{AtomicBool, AtomicU64, Ordering},
            Arc,
        },
    };

    use crate::{clock::Clock, storage::JsonStorage, timer::FocusRatio};

    use super::{PersistenceStatus, ServiceError, TimerPhase, TimerService};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "workbreak-98-service-test-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn storage(&self) -> JsonStorage {
            JsonStorage::new(self.0.join("state.json"))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

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

    #[test]
    fn ratio_change_recalculates_earned_break_and_increments_revision() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());
        service.start_focus(0).unwrap();
        clock.set(30 * 60 * 10_000_000);

        let snapshot = service.set_ratio(1, FocusRatio::Five).unwrap();

        assert_eq!(snapshot.revision, 2);
        assert_eq!(snapshot.ratio, 5);
        assert_eq!(snapshot.break_earned_ms, 6 * 60 * 1_000);
    }

    #[test]
    fn persists_preference_and_recovers_running_focus_as_paused() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(10 * 60 * 10_000_000);
        service.set_ratio(1, FocusRatio::Five).unwrap();
        drop(service);

        clock.set(123);
        let recovered = TimerService::from_storage(clock, directory.storage());
        let snapshot = recovered.snapshot().unwrap();

        assert_eq!(snapshot.phase, TimerPhase::FocusPaused);
        assert_eq!(snapshot.focus_elapsed_ms, 10 * 60 * 1_000);
        assert_eq!(snapshot.ratio, 5);
        assert_eq!(snapshot.persistence_status, PersistenceStatus::Available);
    }

    #[test]
    fn invalid_document_blocks_automatic_overwrite() {
        let directory = TestDirectory::new();
        let path = directory.0.join("state.json");
        let invalid = br#"{"version":1,"data":{"revision":0,"timer":{"phase":"idle","elapsed100ns":1,"ratio":3}}}"#;
        fs::write(&path, invalid).unwrap();
        let service = TimerService::from_storage(FakeClock::default(), directory.storage());

        let started = service.start_focus(0).unwrap();

        assert_eq!(started.phase, TimerPhase::FocusRunning);
        assert_eq!(started.persistence_status, PersistenceStatus::Blocked);
        assert_eq!(fs::read(path).unwrap(), invalid);
    }

    #[test]
    fn write_failure_keeps_state_in_memory_and_reports_unavailable() {
        let directory = TestDirectory::new();
        fs::create_dir(directory.0.join("state.json")).unwrap();
        let service = TimerService::from_storage(FakeClock::default(), directory.storage());

        let snapshot = service.start_focus(0).unwrap();

        assert_eq!(snapshot.phase, TimerPhase::FocusRunning);
        assert_eq!(snapshot.persistence_status, PersistenceStatus::Unavailable);
    }
}
