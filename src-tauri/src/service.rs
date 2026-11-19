use std::{
    error::Error,
    fmt,
    sync::{Mutex, MutexGuard},
};

use serde::{Deserialize, Serialize};

use crate::{
    clock::{Clock, SystemClock},
    progression::{Progression, ProgressionSnapshot},
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
    pub break_earned_ms: i64,
    pub break_remaining_ms: i64,
    pub break_elapsed_ms: u64,
    pub persistence_status: PersistenceStatus,
    /// Effective focus in the current block; drives the size of the hero's foes.
    pub focus_block_ms: u64,
    pub progression: ProgressionSnapshot,
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
    progression: Progression,
    revision: u32,
    persistence_status: PersistenceStatus,
    power_available: bool,
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
                progression: Progression::default(),
                revision: 0,
                persistence_status: PersistenceStatus::Unavailable,
                power_available: true,
            }),
            storage: None,
        }
    }

    pub fn from_storage(clock: C, storage: JsonStorage) -> Self {
        let fresh = || (FocusTimer::default(), Progression::default(), 0);
        let ((timer, progression, revision), persistence_status) =
            match storage.load::<AppDocument>() {
                Ok(LoadResult::Missing) => (fresh(), PersistenceStatus::Available),
                Ok(LoadResult::Loaded(document)) if document.revision < u32::MAX => {
                    match clock.now_100ns().ok().and_then(|now| {
                        let timer = FocusTimer::restore(document.timer, now).ok()?;
                        let focus_elapsed_100ns = timer.effective_at(now).ok()?;
                        // Documents saved before progression existed start the hero at the
                        // current focus, so earlier cycles of the session are not granted.
                        let progression = document
                            .progression
                            .unwrap_or_else(|| Progression::starting_at(focus_elapsed_100ns));
                        progression
                            .is_consistent_with(focus_elapsed_100ns)
                            .then_some((timer, progression))
                    }) {
                        Some((timer, progression)) => (
                            (timer, progression, document.revision),
                            PersistenceStatus::Available,
                        ),
                        None => (fresh(), PersistenceStatus::Blocked),
                    }
                }
                Ok(LoadResult::Loaded(_))
                | Ok(LoadResult::Invalid(_))
                | Ok(LoadResult::UnsupportedVersion(_)) => (fresh(), PersistenceStatus::Blocked),
                Err(_) => (fresh(), PersistenceStatus::Unavailable),
            };

        Self {
            clock,
            state: Mutex::new(ServiceState {
                timer,
                progression,
                revision,
                persistence_status,
                power_available: true,
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
        mutate(&mut state, |timer| timer.finish(now_100ns))?;
        if matches!(state.timer.state(), TimerState::BreakReady { .. }) {
            mutate(&mut state, |timer| timer.start_break(now_100ns))?;
        }
        state.revision = next_revision;
        self.persist_at(&mut state, now_100ns);
        snapshot_at(&state, now_100ns)
    }

    pub fn reset_focus(&self, expected_revision: u32) -> Result<TimerSnapshot, ServiceError> {
        let mut state = self.lock_state()?;
        validate_revision(&state, expected_revision)?;
        let next_revision = next_revision(&state)?;
        let now_100ns = self.now_100ns()?;
        mutate(&mut state, |timer| timer.reset(now_100ns))?;
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
        let mut state = self.lock_state()?;
        let now_100ns = self.now_100ns()?;
        self.normalize_at(&mut state, now_100ns)?;
        validate_revision(&state, expected_revision)?;
        if !matches!(state.timer.state(), TimerState::BreakCompleted { .. }) {
            mutate(&mut state, |timer| timer.end_break(now_100ns))?;
        }
        state.revision = next_revision(&state)?;
        self.persist_at(&mut state, now_100ns);
        snapshot_at(&state, now_100ns)
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

        state.timer.set_ratio_at(ratio, now_100ns)?;
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
                mutate(&mut state, |timer| timer.pause(at_100ns))?;
            }
            TimerState::BreakRunning {
                started_at_100ns, ..
            } if at_100ns >= started_at_100ns => {
                mutate(&mut state, |timer| timer.pause_break(at_100ns))?;
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
                TimerState::FocusRunning { .. } => {
                    mutate(&mut state, |timer| timer.pause(now_100ns))?
                }
                TimerState::BreakRunning { .. } => {
                    mutate(&mut state, |timer| timer.pause_break(now_100ns))?
                }
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
        mutate(&mut state, |timer| action(timer, now_100ns))?;
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
            progression: Some(state.progression),
        };
        state.persistence_status = if storage.write(&document).is_ok() {
            PersistenceStatus::Available
        } else {
            PersistenceStatus::Unavailable
        };
    }

    fn normalize_at(&self, state: &mut ServiceState, now_100ns: u64) -> Result<(), ServiceError> {
        let mut changed = false;
        mutate(state, |timer| {
            changed = timer.normalize_break(now_100ns)?;
            Ok(())
        })?;
        if changed {
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
    let focus_elapsed_100ns = state.timer.effective_at(now_100ns)?;
    Ok(TimerSnapshot {
        revision: state.revision,
        phase,
        focus_elapsed_ms: focus_elapsed_100ns / 10_000,
        ratio: state.timer.ratio().value(),
        break_earned_ms: signed_ms(state.timer.earned_break_at(now_100ns)?),
        break_remaining_ms: signed_ms(state.timer.break_remaining_at(now_100ns)?),
        break_elapsed_ms: state.timer.break_elapsed_at(now_100ns)? / 10_000,
        persistence_status: state.persistence_status,
        focus_block_ms: state.progression.focus_block_100ns(focus_elapsed_100ns) / 10_000,
        progression: state.progression.snapshot(),
    })
}

/// Applies one timer mutation and lets the hero progression observe the transition,
/// so both change under the same lock and are saved by the same write.
fn mutate(
    state: &mut ServiceState,
    action: impl FnOnce(&mut FocusTimer) -> Result<(), TimerError>,
) -> Result<(), TimerError> {
    let before = state.timer.state();
    action(&mut state.timer)?;
    state.progression.observe(before, state.timer.state());
    Ok(())
}

fn signed_ms(value_100ns: i128) -> i64 {
    let milliseconds = value_100ns / 10_000;
    i64::try_from(milliseconds).unwrap_or_else(|_| {
        if milliseconds.is_negative() {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppDocument {
    revision: u32,
    timer: PersistedTimer,
    #[serde(default)]
    progression: Option<Progression>,
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
    fn finishing_focus_starts_a_persisted_break() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(30 * 60 * 10_000_000);

        let snapshot = service.finish_focus(1).unwrap();

        assert_eq!(snapshot.phase, TimerPhase::BreakRunning);
        assert_eq!(snapshot.focus_elapsed_ms, 30 * 60 * 1_000);
        assert_eq!(snapshot.break_earned_ms, 10 * 60 * 1_000);
        drop(service);
        let recovered = TimerService::from_storage(clock, directory.storage());
        assert_eq!(recovered.snapshot().unwrap().phase, TimerPhase::BreakPaused);
    }

    #[test]
    fn reset_discards_active_focus() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(30 * 60 * 10_000_000);

        let snapshot = service.reset_focus(1).unwrap();

        assert_eq!(snapshot.phase, TimerPhase::Idle);
        assert_eq!(snapshot.focus_elapsed_ms, 0);
        drop(service);
        let recovered = TimerService::from_storage(clock, directory.storage());
        let recovered = recovered.snapshot().unwrap();
        assert_eq!(recovered.phase, TimerPhase::Idle);
    }

    #[test]
    fn finishing_zero_focus_returns_to_idle() {
        let service = TimerService::new(FakeClock::default());
        service.start_focus(0).unwrap();

        let snapshot = service.finish_focus(1).unwrap();

        assert_eq!(snapshot.phase, TimerPhase::Idle);
        assert!(service.finish_focus(1).is_err());
    }

    #[test]
    fn old_history_fields_are_ignored_on_recovery() {
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
            PersistenceStatus::Available
        );
    }

    #[test]
    fn changing_ratio_after_finish_preserves_the_granted_break() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(30 * 60 * 10_000_000);
        service.finish_focus(1).unwrap();

        let snapshot = service.set_ratio(2, FocusRatio::Five).unwrap();

        assert_eq!(snapshot.phase, TimerPhase::BreakRunning);
        assert_eq!(snapshot.ratio, 5);
        assert_eq!(snapshot.break_earned_ms, 10 * 60 * 1_000);
        drop(service);
        let recovered = TimerService::from_storage(clock, directory.storage());
        let recovered = recovered.snapshot().unwrap();
        assert_eq!(recovered.phase, TimerPhase::BreakPaused);
        assert_eq!(recovered.ratio, 5);
        assert_eq!(recovered.break_earned_ms, 10 * 60 * 1_000);
    }

    #[test]
    fn break_completion_waits_for_manual_focus_start() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());
        service.start_focus(0).unwrap();
        clock.set(3 * 60 * 10_000_000);
        service.finish_focus(1).unwrap();
        clock.set(4 * 60 * 10_000_000);

        let completed = service.snapshot().unwrap();

        assert_eq!(completed.phase, TimerPhase::BreakCompleted);
        assert_eq!(completed.break_remaining_ms, 0);
        assert_eq!(completed.revision, 3);
        assert_eq!(completed.break_elapsed_ms, 60 * 1_000);
        assert_eq!(completed.focus_elapsed_ms, 3 * 60 * 1_000);
        assert_eq!(completed.break_earned_ms, 60 * 1_000);
        clock.set(5 * 60 * 10_000_000);
        assert_eq!(
            service.snapshot().unwrap().phase,
            TimerPhase::BreakCompleted
        );
        assert_eq!(
            service.start_focus(3).unwrap().phase,
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
        clock.set(35 * 60 * 10_000_000);
        service.checkpoint().unwrap();
        drop(service);

        let recovered = TimerService::from_storage(clock, directory.storage());
        let snapshot = recovered.snapshot().unwrap();
        assert_eq!(snapshot.phase, TimerPhase::BreakPaused);
        assert_eq!(snapshot.break_remaining_ms, 5 * 60 * 1_000);
    }

    #[test]
    fn preparing_close_pauses_a_running_break() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(30 * 60 * 10_000_000);
        service.finish_focus(1).unwrap();
        clock.set(35 * 60 * 10_000_000);

        let snapshot = service.prepare_close().unwrap();

        assert_eq!(snapshot.phase, TimerPhase::BreakPaused);
        assert_eq!(snapshot.break_remaining_ms, 5 * 60 * 1_000);
    }

    #[test]
    fn ending_break_waits_for_manual_focus_start() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());
        service.start_focus(0).unwrap();
        clock.set(30 * 60 * 10_000_000);
        service.finish_focus(1).unwrap();
        clock.set(35 * 60 * 10_000_000);

        let snapshot = service.end_break(2).unwrap();

        assert_eq!(snapshot.phase, TimerPhase::BreakCompleted);
        assert_eq!(snapshot.focus_elapsed_ms, 30 * 60 * 1_000);
        assert_eq!(snapshot.break_earned_ms, 10 * 60 * 1_000);
        assert_eq!(snapshot.break_remaining_ms, 5 * 60 * 1_000);
        assert_eq!(snapshot.break_elapsed_ms, 5 * 60 * 1_000);
        assert_eq!(
            service.start_focus(3).unwrap().phase,
            TimerPhase::FocusRunning
        );
        assert_eq!(service.reset_focus(4).unwrap().focus_elapsed_ms, 0);
    }

    #[test]
    fn ending_break_after_its_balance_waits_with_negative_balance() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());
        service.start_focus(0).unwrap();
        clock.set(30 * 60 * 10_000_000);
        service.finish_focus(1).unwrap();
        clock.set(41 * 60 * 10_000_000);

        let snapshot = service.snapshot().unwrap();

        assert_eq!(snapshot.phase, TimerPhase::BreakCompleted);
        assert_eq!(snapshot.break_earned_ms, 10 * 60 * 1_000);
        assert_eq!(snapshot.break_remaining_ms, -(60 * 1_000));
        assert_eq!(snapshot.break_elapsed_ms, 11 * 60 * 1_000);
        assert_eq!(snapshot.focus_elapsed_ms, 30 * 60 * 1_000);
    }

    #[test]
    fn zero_new_break_after_negative_balance_waits_for_focus() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());
        service.start_focus(0).unwrap();
        clock.set(30 * 60 * 10_000_000);
        service.finish_focus(1).unwrap();
        clock.set(41 * 60 * 10_000_000);
        let snapshot = service.snapshot().unwrap();
        service.start_focus(snapshot.revision).unwrap();
        clock.set(44 * 60 * 10_000_000);

        let next_focus = service.finish_focus(snapshot.revision + 1).unwrap();

        assert_eq!(next_focus.phase, TimerPhase::BreakCompleted);
        assert_eq!(next_focus.break_earned_ms, 0);
        assert_eq!(next_focus.break_elapsed_ms, 11 * 60 * 1_000);
    }

    const MINUTE: u64 = 60 * 10_000_000;

    #[test]
    fn finishing_focus_grants_xp_for_active_time_only() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());
        service.start_focus(0).unwrap();
        clock.set(10 * MINUTE);
        service.pause_focus(1).unwrap();
        clock.set(15 * MINUTE);
        service.resume_focus(2).unwrap();
        clock.set(35 * MINUTE);

        let snapshot = service.finish_focus(3).unwrap();

        assert_eq!(snapshot.progression.total_xp, 30);
        let reward = snapshot.progression.last_reward.unwrap();
        assert_eq!(reward.kind, crate::progression::RewardKind::Focus);
        assert_eq!(reward.xp, 30);
    }

    #[test]
    fn completed_break_grants_rest_bonus_and_next_block_counts_only_new_focus() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());
        service.start_focus(0).unwrap();
        clock.set(30 * MINUTE);
        service.finish_focus(1).unwrap();
        clock.set(40 * MINUTE);

        let rested = service.snapshot().unwrap();
        assert_eq!(rested.phase, TimerPhase::BreakCompleted);
        assert_eq!(rested.progression.total_xp, 36);
        assert_eq!(
            rested.progression.last_reward.unwrap().kind,
            crate::progression::RewardKind::Rest
        );

        service.start_focus(rested.revision).unwrap();
        clock.set(60 * MINUTE);
        let second = service.finish_focus(rested.revision + 1).unwrap();
        assert_eq!(second.progression.last_reward.unwrap().xp, 20);
        assert_eq!(second.progression.total_xp, 56);
    }

    #[test]
    fn ending_break_early_discards_the_rest_bonus() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());
        service.start_focus(0).unwrap();
        clock.set(30 * MINUTE);
        service.finish_focus(1).unwrap();
        clock.set(35 * MINUTE);

        let ended = service.end_break(2).unwrap();

        assert_eq!(ended.phase, TimerPhase::BreakCompleted);
        assert_eq!(ended.progression.total_xp, 30);
    }

    #[test]
    fn reset_never_grants_or_removes_xp() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());
        service.start_focus(0).unwrap();
        clock.set(30 * MINUTE);
        service.finish_focus(1).unwrap();
        let reset = service.reset_focus(2).unwrap();
        assert_eq!(reset.progression.total_xp, 30);

        service.start_focus(3).unwrap();
        clock.set(60 * MINUTE);
        let discarded = service.reset_focus(4).unwrap();
        assert_eq!(discarded.progression.total_xp, 30);
    }

    #[test]
    fn stale_or_repeated_finish_grants_xp_once() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());
        service.start_focus(0).unwrap();
        clock.set(30 * MINUTE);
        service.finish_focus(1).unwrap();

        assert!(service.finish_focus(1).is_err());
        assert!(service.finish_focus(2).is_err());
        assert_eq!(service.snapshot().unwrap().progression.total_xp, 30);
    }

    #[test]
    fn progression_survives_recovery_and_pending_bonus_is_granted_once() {
        let directory = TestDirectory::new();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), directory.storage());
        service.start_focus(0).unwrap();
        clock.set(30 * MINUTE);
        service.finish_focus(1).unwrap();
        clock.set(32 * MINUTE);
        service.checkpoint().unwrap();
        drop(service);

        clock.set(100 * MINUTE);
        let recovered = TimerService::from_storage(clock.clone(), directory.storage());
        let paused = recovered.snapshot().unwrap();
        assert_eq!(paused.phase, TimerPhase::BreakPaused);
        assert_eq!(paused.progression.total_xp, 30);

        recovered.resume_break(paused.revision).unwrap();
        clock.set(108 * MINUTE);
        let completed = recovered.snapshot().unwrap();
        assert_eq!(completed.phase, TimerPhase::BreakCompleted);
        assert_eq!(completed.progression.total_xp, 36);
        drop(recovered);

        let reopened = TimerService::from_storage(clock, directory.storage());
        assert_eq!(reopened.snapshot().unwrap().progression.total_xp, 36);
    }

    #[test]
    fn document_without_progression_loads_with_zero_xp() {
        let directory = TestDirectory::new();
        let storage = directory.storage();
        storage
            .write(&serde_json::json!({
                "revision": 1,
                "timer": { "phase": "idle", "elapsed100ns": 0, "ratio": 3 }
            }))
            .unwrap();

        let service = TimerService::from_storage(FakeClock::default(), storage);
        let snapshot = service.snapshot().unwrap();

        assert_eq!(snapshot.persistence_status, PersistenceStatus::Available);
        assert_eq!(snapshot.progression.total_xp, 0);
        assert_eq!(snapshot.progression.level, 1);
    }

    #[test]
    fn impossible_progression_blocks_automatic_overwrite() {
        let directory = TestDirectory::new();
        let path = directory.0.join("state.json");
        let invalid = br#"{"version":1,"data":{"revision":0,"timer":{"phase":"idle","elapsed100ns":0,"ratio":3},"progression":{"totalXp":5,"pendingRestBonusXp":999}}}"#;
        fs::write(&path, invalid).unwrap();
        let service = TimerService::from_storage(FakeClock::default(), directory.storage());

        let started = service.start_focus(0).unwrap();

        assert_eq!(started.persistence_status, PersistenceStatus::Blocked);
        assert_eq!(started.progression.total_xp, 0);
        assert_eq!(fs::read(path).unwrap(), invalid);
    }

    #[test]
    fn legacy_document_mid_focus_does_not_grant_earlier_cycles() {
        let directory = TestDirectory::new();
        let storage = directory.storage();
        storage
            .write(&serde_json::json!({
                "revision": 7,
                "timer": { "phase": "paused", "elapsed100ns": 90 * MINUTE, "ratio": 3 }
            }))
            .unwrap();
        let clock = FakeClock::default();
        let service = TimerService::from_storage(clock.clone(), storage);
        service.resume_focus(7).unwrap();
        clock.set(30 * MINUTE);

        let finished = service.finish_focus(8).unwrap();

        assert_eq!(finished.progression.total_xp, 30);
    }

    #[test]
    fn snapshot_reports_only_the_current_focus_block() {
        let clock = FakeClock::default();
        let service = TimerService::new(clock.clone());
        service.start_focus(0).unwrap();
        clock.set(30 * MINUTE);
        assert_eq!(service.snapshot().unwrap().focus_block_ms, 30 * 60 * 1_000);
        service.finish_focus(1).unwrap();
        clock.set(40 * MINUTE);
        let rested = service.snapshot().unwrap();
        assert_eq!(rested.focus_block_ms, 0);

        service.start_focus(rested.revision).unwrap();
        clock.set(52 * MINUTE);
        let second = service.snapshot().unwrap();
        assert_eq!(second.focus_elapsed_ms, 42 * 60 * 1_000);
        assert_eq!(second.focus_block_ms, 12 * 60 * 1_000);
    }
}
