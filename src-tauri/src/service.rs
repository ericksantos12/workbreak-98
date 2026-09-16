use std::{
    error::Error,
    fmt,
    sync::{Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    clock::{Clock, SystemClock},
    storage::{JsonStorage, LoadResult},
    timer::{FocusRatio, FocusTimer, PersistedTimer, TimerError, TimerState},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TimerPhase {
    Idle,
    FocusRunning,
    FocusPaused,
    BreakReady,
    BreakRunning,
    BreakPaused,
    BreakCompleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PersistenceStatus {
    Available,
    Unavailable,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerSnapshot {
    pub revision: u32,
    pub phase: TimerPhase,
    pub focus_elapsed_ms: u64,
    pub ratio: u8,
    pub break_earned_ms: u64,
    pub break_remaining_ms: u64,
    pub history: Vec<FocusHistoryEntry>,
    pub persistence_status: PersistenceStatus,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusHistoryEntry {
    pub id: u64,
    pub completed_at_unix_ms: u64,
    pub focus_elapsed_ms: u64,
    pub ratio: u8,
    pub break_earned_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceError {
    Clock(String),
    Timer(TimerError),
    PowerUnavailable,
    PersistenceUnavailable(PersistenceStatus),
    StaleRevision { expected: u32, actual: u32 },
    RevisionOverflow,
    StateUnavailable,
}

impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clock(error) => write!(formatter, "clock unavailable: {error}"),
            Self::Timer(error) => error.fmt(formatter),
            Self::PowerUnavailable => formatter.write_str("power monitoring is unavailable"),
            Self::PersistenceUnavailable(status) => {
                write!(formatter, "session could not be saved: {status:?}")
            }
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
    power_available: bool,
    history: Vec<FocusHistoryEntry>,
    next_history_id: u64,
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
                power_available: true,
                history: Vec::new(),
                next_history_id: 1,
            }),
            storage: None,
        }
    }

    pub fn from_storage(clock: C, storage: JsonStorage) -> Self {
        let (timer, revision, history, next_history_id, persistence_status) =
            match storage.load::<AppDocument>() {
                Ok(LoadResult::Missing) => (
                    FocusTimer::default(),
                    0,
                    Vec::new(),
                    1,
                    PersistenceStatus::Available,
                ),
                Ok(LoadResult::Loaded(document))
                    if document.revision < u32::MAX && document.is_valid() =>
                {
                    match clock
                        .now_100ns()
                        .ok()
                        .and_then(|now| FocusTimer::restore(document.timer, now).ok())
                    {
                        Some(timer) => (
                            timer,
                            document.revision,
                            document.history,
                            document.next_history_id,
                            PersistenceStatus::Available,
                        ),
                        None => (
                            FocusTimer::default(),
                            0,
                            Vec::new(),
                            1,
                            PersistenceStatus::Blocked,
                        ),
                    }
                }
                Ok(LoadResult::Loaded(_))
                | Ok(LoadResult::Invalid(_))
                | Ok(LoadResult::UnsupportedVersion(_)) => (
                    FocusTimer::default(),
                    0,
                    Vec::new(),
                    1,
                    PersistenceStatus::Blocked,
                ),
                Err(_) => (
                    FocusTimer::default(),
                    0,
                    Vec::new(),
                    1,
                    PersistenceStatus::Unavailable,
                ),
            };

        Self {
            clock,
            state: Mutex::new(ServiceState {
                timer,
                revision,
                persistence_status,
                power_available: true,
                history,
                next_history_id,
            }),
            storage: Some(storage),
        }
    }

    pub fn snapshot(&self) -> Result<TimerSnapshot, ServiceError> {
        let mut state = self.lock_state()?;
        let now_100ns = self.now_100ns()?;
        self.normalize_at(&mut state, now_100ns)?;
        snapshot_at(&state, now_100ns)
    }

    pub fn start_focus(&self, expected_revision: u32) -> Result<TimerSnapshot, ServiceError> {
        self.transition(expected_revision, true, FocusTimer::start)
    }

    pub fn pause_focus(&self, expected_revision: u32) -> Result<TimerSnapshot, ServiceError> {
        self.transition(expected_revision, false, FocusTimer::pause)
    }

    pub fn resume_focus(&self, expected_revision: u32) -> Result<TimerSnapshot, ServiceError> {
        self.transition(expected_revision, true, FocusTimer::resume)
    }

    pub fn finish_focus(&self, expected_revision: u32) -> Result<TimerSnapshot, ServiceError> {
        let mut state = self.lock_state()?;
        validate_revision(&state, expected_revision)?;
        let next_revision = next_revision(&state)?;
        let now_100ns = self.now_100ns()?;
        let focus_elapsed_ms = state.timer.effective_at(now_100ns)? / 10_000;
        let ratio = state.timer.ratio().value();
        let break_earned_ms = state.timer.earned_break_at(now_100ns)? / 10_000;
        state.timer.finish(now_100ns)?;
        state.revision = next_revision;
        if focus_elapsed_ms > 0 {
            let id = state.next_history_id;
            state.next_history_id = id.checked_add(1).ok_or(ServiceError::RevisionOverflow)?;
            state.history.insert(
                0,
                FocusHistoryEntry {
                    id,
                    completed_at_unix_ms: current_unix_ms(),
                    focus_elapsed_ms,
                    ratio,
                    break_earned_ms,
                },
            );
        }
        self.persist_at(&mut state, now_100ns);
        snapshot_at(&state, now_100ns)
    }

    pub fn reset_focus(&self, expected_revision: u32) -> Result<TimerSnapshot, ServiceError> {
        let mut state = self.lock_state()?;
        validate_revision(&state, expected_revision)?;
        let next_revision = next_revision(&state)?;
        let now_100ns = self.now_100ns()?;
        state.timer.reset(now_100ns)?;
        state.revision = next_revision;
        self.persist_at(&mut state, now_100ns);
        snapshot_at(&state, now_100ns)
    }

    pub fn start_break(&self, expected_revision: u32) -> Result<TimerSnapshot, ServiceError> {
        self.transition(expected_revision, false, FocusTimer::start_break)
    }

    pub fn pause_break(&self, expected_revision: u32) -> Result<TimerSnapshot, ServiceError> {
        self.transition(expected_revision, false, FocusTimer::pause_break)
    }

    pub fn resume_break(&self, expected_revision: u32) -> Result<TimerSnapshot, ServiceError> {
        self.transition(expected_revision, false, FocusTimer::resume_break)
    }

    pub fn end_break(&self, expected_revision: u32) -> Result<TimerSnapshot, ServiceError> {
        self.transition(expected_revision, false, FocusTimer::end_break)
    }

    pub fn set_ratio(
        &self,
        expected_revision: u32,
        ratio: FocusRatio,
    ) -> Result<TimerSnapshot, ServiceError> {
        let mut state = self.lock_state()?;
        let now_100ns = self.now_100ns()?;
        self.normalize_at(&mut state, now_100ns)?;
        validate_revision(&state, expected_revision)?;
        let next_revision = next_revision(&state)?;

        state.timer.set_ratio(ratio);
        state.revision = next_revision;
        self.persist_at(&mut state, now_100ns);
        snapshot_at(&state, now_100ns)
    }

    pub fn checkpoint(&self) -> Result<(), ServiceError> {
        let mut state = self.lock_state()?;
        let now_100ns = self.now_100ns()?;
        self.normalize_at(&mut state, now_100ns)?;
        if state.persistence_status == PersistenceStatus::Blocked
            || !matches!(
                state.timer.state(),
                TimerState::FocusRunning { .. } | TimerState::BreakRunning { .. }
            )
        {
            return Ok(());
        }
        self.persist_at(&mut state, now_100ns);
        Ok(())
    }

    pub fn pause_for_suspend(&self, at_100ns: u64) -> Result<(), ServiceError> {
        let mut state = self.lock_state()?;
        match state.timer.state() {
            TimerState::FocusRunning {
                started_at_100ns, ..
            } if at_100ns >= started_at_100ns => {
                state.timer.pause(at_100ns)?;
            }
            TimerState::BreakRunning {
                started_at_100ns, ..
            } if at_100ns >= started_at_100ns => {
                state.timer.pause_break(at_100ns)?;
            }
            _ => return Ok(()),
        }
        state.revision = next_revision(&state)?;
        self.persist_at(&mut state, at_100ns);
        Ok(())
    }

    pub fn set_power_available(&self, available: bool) -> Result<(), ServiceError> {
        self.lock_state()?.power_available = available;
        Ok(())
    }

    pub fn prepare_close(&self) -> Result<TimerSnapshot, ServiceError> {
        let mut state = self.lock_state()?;
        let now_100ns = self.now_100ns()?;
        self.normalize_at(&mut state, now_100ns)?;
        if matches!(
            state.timer.state(),
            TimerState::FocusRunning { .. } | TimerState::BreakRunning { .. }
        ) {
            let next_revision = next_revision(&state)?;
            match state.timer.state() {
                TimerState::FocusRunning { .. } => state.timer.pause(now_100ns)?,
                TimerState::BreakRunning { .. } => state.timer.pause_break(now_100ns)?,
                _ => unreachable!(),
            }
            state.revision = next_revision;
        }
        self.persist_at(&mut state, now_100ns);
        let snapshot = snapshot_at(&state, now_100ns)?;

        if state.persistence_status == PersistenceStatus::Available {
            Ok(snapshot)
        } else {
            Err(ServiceError::PersistenceUnavailable(
                state.persistence_status,
            ))
        }
    }

    fn transition(
        &self,
        expected_revision: u32,
        requires_power: bool,
        action: fn(&mut FocusTimer, u64) -> Result<(), TimerError>,
    ) -> Result<TimerSnapshot, ServiceError> {
        let mut state = self.lock_state()?;
        let now_100ns = self.now_100ns()?;
        self.normalize_at(&mut state, now_100ns)?;
        validate_revision(&state, expected_revision)?;
        if requires_power && !state.power_available {
            return Err(ServiceError::PowerUnavailable);
        }
        let next_revision = next_revision(&state)?;
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
            history: state.history.clone(),
            next_history_id: state.next_history_id,
        };
        state.persistence_status = if storage.write(&document).is_ok() {
            PersistenceStatus::Available
        } else {
            PersistenceStatus::Unavailable
        };
    }

    fn normalize_at(&self, state: &mut ServiceState, now_100ns: u64) -> Result<(), ServiceError> {
        if state.timer.normalize_break(now_100ns)? {
            state.revision = next_revision(state)?;
            self.persist_at(state, now_100ns);
        }
        Ok(())
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
        TimerState::BreakReady { .. } => TimerPhase::BreakReady,
        TimerState::BreakRunning { .. } => TimerPhase::BreakRunning,
        TimerState::BreakPaused { .. } => TimerPhase::BreakPaused,
        TimerState::BreakCompleted { .. } => TimerPhase::BreakCompleted,
    };
    Ok(TimerSnapshot {
        revision: state.revision,
        phase,
        focus_elapsed_ms: state.timer.effective_at(now_100ns)? / 10_000,
        ratio: state.timer.ratio().value(),
        break_earned_ms: state.timer.earned_break_at(now_100ns)? / 10_000,
        break_remaining_ms: state.timer.break_remaining_at(now_100ns)? / 10_000,
        history: state.history.clone(),
        persistence_status: state.persistence_status,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppDocument {
    revision: u32,
    timer: PersistedTimer,
    #[serde(default)]
    history: Vec<FocusHistoryEntry>,
    #[serde(default = "first_history_id")]
    next_history_id: u64,
}

impl AppDocument {
    fn is_valid(&self) -> bool {
        self.next_history_id != 0
            && self
                .history
                .iter()
                .all(|entry| entry.id != 0 && entry.id < self.next_history_id)
            && self.history.iter().enumerate().all(|(index, entry)| {
                self.history[..index]
                    .iter()
                    .all(|other| other.id != entry.id)
            })
    }
}

fn first_history_id() -> u64 {
    1
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
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
            Arc, Barrier,
        },
        thread,
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

    #[test]
    fn checkpoint_persists_running_elapsed_without_incrementing_revision() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(7 * 60 * 10_000_000);

        service.checkpoint().unwrap();
        assert_eq!(service.snapshot().unwrap().revision, 1);
        drop(service);

        let recovered = TimerService::from_storage(clock, directory.storage());
        let snapshot = recovered.snapshot().unwrap();
        assert_eq!(snapshot.phase, TimerPhase::FocusPaused);
        assert_eq!(snapshot.focus_elapsed_ms, 7 * 60 * 1_000);
        assert_eq!(snapshot.revision, 1);
    }

    #[test]
    fn checkpoint_cannot_overwrite_a_concurrent_pause() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = Arc::new(TimerService::from_storage(
            clock.clone(),
            directory.storage(),
        ));
        service.start_focus(0).unwrap();
        clock.set(5 * 60 * 10_000_000);
        let barrier = Arc::new(Barrier::new(3));

        let checkpoint_service = Arc::clone(&service);
        let checkpoint_barrier = Arc::clone(&barrier);
        let checkpoint = thread::spawn(move || {
            checkpoint_barrier.wait();
            checkpoint_service.checkpoint().unwrap();
        });
        let pause_service = Arc::clone(&service);
        let pause_barrier = Arc::clone(&barrier);
        let pause = thread::spawn(move || {
            pause_barrier.wait();
            pause_service.pause_focus(1).unwrap();
        });
        barrier.wait();
        checkpoint.join().unwrap();
        pause.join().unwrap();
        drop(service);

        let recovered = TimerService::from_storage(clock, directory.storage());
        let snapshot = recovered.snapshot().unwrap();
        assert_eq!(snapshot.phase, TimerPhase::FocusPaused);
        assert_eq!(snapshot.revision, 2);
        assert_eq!(snapshot.focus_elapsed_ms, 5 * 60 * 1_000);
    }

    #[test]
    fn later_checkpoint_retries_an_unavailable_write() {
        let directory = TestDirectory::new();
        let state_path = directory.0.join("state.json");
        fs::create_dir(&state_path).unwrap();
        let service = TimerService::from_storage(FakeClock::default(), directory.storage());
        assert_eq!(
            service.start_focus(0).unwrap().persistence_status,
            PersistenceStatus::Unavailable
        );
        fs::remove_dir(state_path).unwrap();

        service.checkpoint().unwrap();

        assert_eq!(
            service.snapshot().unwrap().persistence_status,
            PersistenceStatus::Available
        );
    }

    #[test]
    fn suspension_pauses_at_the_callback_boundary() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());
        service.start_focus(0).unwrap();
        clock.set(20 * 60 * 10_000_000);

        service.pause_for_suspend(12 * 60 * 10_000_000).unwrap();

        let snapshot = service.snapshot().unwrap();
        assert_eq!(snapshot.phase, TimerPhase::FocusPaused);
        assert_eq!(snapshot.focus_elapsed_ms, 12 * 60 * 1_000);
        assert_eq!(snapshot.revision, 2);
    }

    #[test]
    fn stale_suspend_event_does_not_pause_a_newer_focus_segment() {
        let clock = FakeClock::default();
        clock.set(100);
        let service = TimerService::new(clock);
        service.start_focus(0).unwrap();

        service.pause_for_suspend(99).unwrap();

        assert_eq!(service.snapshot().unwrap().phase, TimerPhase::FocusRunning);
        assert_eq!(service.snapshot().unwrap().revision, 1);
    }

    #[test]
    fn unavailable_power_monitor_blocks_start_and_resume() {
        let service = TimerService::new(FakeClock::default());
        service.set_power_available(false).unwrap();
        assert_eq!(service.start_focus(0), Err(ServiceError::PowerUnavailable));

        service.set_power_available(true).unwrap();
        service.start_focus(0).unwrap();
        service.pause_focus(1).unwrap();
        service.set_power_available(false).unwrap();
        assert_eq!(service.resume_focus(2), Err(ServiceError::PowerUnavailable));
    }

    #[test]
    fn preparing_close_pauses_and_persists_running_focus() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(8 * 60 * 10_000_000);

        let snapshot = service.prepare_close().unwrap();

        assert_eq!(snapshot.phase, TimerPhase::FocusPaused);
        assert_eq!(snapshot.focus_elapsed_ms, 8 * 60 * 1_000);
        drop(service);
        let recovered = TimerService::from_storage(clock, directory.storage());
        assert_eq!(recovered.snapshot().unwrap().phase, TimerPhase::FocusPaused);
    }

    #[test]
    fn failed_close_save_keeps_focus_paused_and_can_be_retried() {
        let directory = TestDirectory::new();
        let state_path = directory.0.join("state.json");
        fs::create_dir(&state_path).unwrap();
        let service = TimerService::from_storage(FakeClock::default(), directory.storage());
        service.start_focus(0).unwrap();

        assert_eq!(
            service.prepare_close(),
            Err(ServiceError::PersistenceUnavailable(
                PersistenceStatus::Unavailable
            ))
        );
        assert_eq!(service.snapshot().unwrap().phase, TimerPhase::FocusPaused);

        fs::remove_dir(state_path).unwrap();
        assert!(service.prepare_close().is_ok());
    }

    #[test]
    fn repeated_close_after_suspension_does_not_resume_or_advance_revision() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(5 * 60 * 10_000_000);
        service.pause_for_suspend(5 * 60 * 10_000_000).unwrap();

        let first = service.prepare_close().unwrap();
        let repeated = service.prepare_close().unwrap();

        assert_eq!(first.phase, TimerPhase::FocusPaused);
        assert_eq!(repeated.phase, TimerPhase::FocusPaused);
        assert_eq!(repeated.revision, 2);
        assert_eq!(repeated.focus_elapsed_ms, 5 * 60 * 1_000);
    }

    #[test]
    fn close_and_checkpoint_are_serialized_without_losing_the_pause() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = Arc::new(TimerService::from_storage(
            clock.clone(),
            directory.storage(),
        ));
        service.start_focus(0).unwrap();
        clock.set(3 * 60 * 10_000_000);
        let barrier = Arc::new(Barrier::new(3));
        let checkpoint_service = Arc::clone(&service);
        let checkpoint_barrier = Arc::clone(&barrier);
        let checkpoint = thread::spawn(move || {
            checkpoint_barrier.wait();
            checkpoint_service.checkpoint().unwrap();
        });
        let close_service = Arc::clone(&service);
        let close_barrier = Arc::clone(&barrier);
        let close = thread::spawn(move || {
            close_barrier.wait();
            close_service.prepare_close().unwrap();
        });

        barrier.wait();
        checkpoint.join().unwrap();
        close.join().unwrap();
        drop(service);

        let recovered = TimerService::from_storage(clock, directory.storage());
        let snapshot = recovered.snapshot().unwrap();
        assert_eq!(snapshot.phase, TimerPhase::FocusPaused);
        assert_eq!(snapshot.revision, 2);
        assert_eq!(snapshot.focus_elapsed_ms, 3 * 60 * 1_000);
    }

    #[test]
    fn finishing_focus_persists_one_frozen_history_entry_and_break_ready() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(30 * 60 * 10_000_000);

        let snapshot = service.finish_focus(1).unwrap();

        assert_eq!(snapshot.phase, TimerPhase::BreakReady);
        assert_eq!(snapshot.focus_elapsed_ms, 30 * 60 * 1_000);
        assert_eq!(snapshot.break_earned_ms, 10 * 60 * 1_000);
        assert_eq!(snapshot.history.len(), 1);
        assert_eq!(snapshot.history[0].id, 1);
        assert_eq!(snapshot.history[0].ratio, 3);
        assert_eq!(snapshot.history[0].focus_elapsed_ms, 30 * 60 * 1_000);
        drop(service);
        let recovered = TimerService::from_storage(clock, directory.storage());
        assert_eq!(recovered.snapshot().unwrap().history.len(), 1);
    }

    #[test]
    fn reset_discards_active_focus_without_creating_history() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(30 * 60 * 10_000_000);

        let snapshot = service.reset_focus(1).unwrap();

        assert_eq!(snapshot.phase, TimerPhase::Idle);
        assert_eq!(snapshot.focus_elapsed_ms, 0);
        assert!(snapshot.history.is_empty());
        drop(service);
        let recovered = TimerService::from_storage(clock, directory.storage());
        let recovered = recovered.snapshot().unwrap();
        assert_eq!(recovered.phase, TimerPhase::Idle);
        assert!(recovered.history.is_empty());
    }

    #[test]
    fn finishing_zero_focus_returns_to_idle_without_history() {
        let service = TimerService::new(FakeClock::default());
        service.start_focus(0).unwrap();

        let snapshot = service.finish_focus(1).unwrap();

        assert_eq!(snapshot.phase, TimerPhase::Idle);
        assert!(snapshot.history.is_empty());
        assert!(service.finish_focus(1).is_err());
    }

    #[test]
    fn duplicate_history_ids_block_recovery_without_overwriting_data() {
        let directory = TestDirectory::new();
        let storage = directory.storage();
        storage
            .write(&serde_json::json!({
                "revision": 1,
                "timer": { "phase": "idle", "elapsed100ns": 0, "ratio": 3, "breakEarned100ns": 0 },
                "nextHistoryId": 3,
                "history": [
                    { "id": 1, "completedAtUnixMs": 1, "focusElapsedMs": 1, "ratio": 3, "breakEarnedMs": 0 },
                    { "id": 1, "completedAtUnixMs": 2, "focusElapsedMs": 2, "ratio": 3, "breakEarnedMs": 0 }
                ]
            }))
            .unwrap();

        let service = TimerService::from_storage(FakeClock::default(), storage);

        assert_eq!(
            service.snapshot().unwrap().persistence_status,
            PersistenceStatus::Blocked
        );
    }

    #[test]
    fn changing_ratio_after_finish_preserves_the_granted_break_and_history() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(30 * 60 * 10_000_000);
        service.finish_focus(1).unwrap();

        let snapshot = service.set_ratio(2, FocusRatio::Five).unwrap();

        assert_eq!(snapshot.phase, TimerPhase::BreakReady);
        assert_eq!(snapshot.ratio, 5);
        assert_eq!(snapshot.break_earned_ms, 10 * 60 * 1_000);
        assert_eq!(snapshot.history[0].ratio, 3);
        drop(service);
        let recovered = TimerService::from_storage(clock, directory.storage());
        let recovered = recovered.snapshot().unwrap();
        assert_eq!(recovered.phase, TimerPhase::BreakReady);
        assert_eq!(recovered.ratio, 5);
        assert_eq!(recovered.break_earned_ms, 10 * 60 * 1_000);
    }

    #[test]
    fn break_completion_is_normalized_before_snapshot_and_new_focus_is_manual() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());
        service.start_focus(0).unwrap();
        clock.set(3 * 60 * 10_000_000);
        service.finish_focus(1).unwrap();
        service.start_break(2).unwrap();
        clock.set(4 * 60 * 10_000_000);

        let completed = service.snapshot().unwrap();

        assert_eq!(completed.phase, TimerPhase::BreakCompleted);
        assert_eq!(completed.break_remaining_ms, 0);
        assert_eq!(completed.revision, 4);
        assert_eq!(
            service.start_focus(4).unwrap().phase,
            TimerPhase::FocusRunning
        );
    }

    #[test]
    fn running_break_recovers_as_paused_with_its_remaining_duration() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(30 * 60 * 10_000_000);
        service.finish_focus(1).unwrap();
        service.start_break(2).unwrap();
        clock.set(35 * 60 * 10_000_000);
        service.checkpoint().unwrap();
        drop(service);

        let recovered = TimerService::from_storage(clock, directory.storage());
        let snapshot = recovered.snapshot().unwrap();
        assert_eq!(snapshot.phase, TimerPhase::BreakPaused);
        assert_eq!(snapshot.break_remaining_ms, 5 * 60 * 1_000);
        assert_eq!(snapshot.history.len(), 1);
    }

    #[test]
    fn preparing_close_pauses_a_running_break() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(30 * 60 * 10_000_000);
        service.finish_focus(1).unwrap();
        service.start_break(2).unwrap();
        clock.set(35 * 60 * 10_000_000);

        let snapshot = service.prepare_close().unwrap();

        assert_eq!(snapshot.phase, TimerPhase::BreakPaused);
        assert_eq!(snapshot.break_remaining_ms, 5 * 60 * 1_000);
    }
}
