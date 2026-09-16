use serde::{Deserialize, Serialize};
use tauri::{State, WebviewWindow};

use crate::{
    clock::Clock,
    service::{AppTimerService, TimerService, TimerSnapshot},
    timer::FocusRatio,
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TimerAction {
    StartFocus {
        #[serde(rename = "expectedRevision")]
        expected_revision: u32,
    },
    PauseFocus {
        #[serde(rename = "expectedRevision")]
        expected_revision: u32,
    },
    ResumeFocus {
        #[serde(rename = "expectedRevision")]
        expected_revision: u32,
    },
    FinishFocus {
        #[serde(rename = "expectedRevision")]
        expected_revision: u32,
    },
    ResetFocus {
        #[serde(rename = "expectedRevision")]
        expected_revision: u32,
    },
    StartBreak {
        #[serde(rename = "expectedRevision")]
        expected_revision: u32,
    },
    PauseBreak {
        #[serde(rename = "expectedRevision")]
        expected_revision: u32,
    },
    ResumeBreak {
        #[serde(rename = "expectedRevision")]
        expected_revision: u32,
    },
    EndBreak {
        #[serde(rename = "expectedRevision")]
        expected_revision: u32,
    },
    SetRatio {
        #[serde(rename = "expectedRevision")]
        expected_revision: u32,
        ratio: u8,
    },
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloseDecision {
    Retry,
    Cancel,
    ExitWithoutSaving,
}

pub fn dispatch_action<C: Clock>(
    service: &TimerService<C>,
    action: TimerAction,
) -> Result<TimerSnapshot, String> {
    match action {
        TimerAction::StartFocus { expected_revision } => service.start_focus(expected_revision),
        TimerAction::PauseFocus { expected_revision } => service.pause_focus(expected_revision),
        TimerAction::ResumeFocus { expected_revision } => service.resume_focus(expected_revision),
        TimerAction::FinishFocus { expected_revision } => service.finish_focus(expected_revision),
        TimerAction::ResetFocus { expected_revision } => service.reset_focus(expected_revision),
        TimerAction::StartBreak { expected_revision } => service.start_break(expected_revision),
        TimerAction::PauseBreak { expected_revision } => service.pause_break(expected_revision),
        TimerAction::ResumeBreak { expected_revision } => service.resume_break(expected_revision),
        TimerAction::EndBreak { expected_revision } => service.end_break(expected_revision),
        TimerAction::SetRatio {
            expected_revision,
            ratio,
        } => {
            let ratio = FocusRatio::try_from(ratio).map_err(|error| error.to_string())?;
            service.set_ratio(expected_revision, ratio)
        }
    }
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_timer_snapshot(service: State<'_, AppTimerService>) -> Result<TimerSnapshot, String> {
    service.snapshot().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn apply_timer_action(
    service: State<'_, AppTimerService>,
    action: TimerAction,
) -> Result<TimerSnapshot, String> {
    dispatch_action(service.inner(), action)
}

#[tauri::command]
pub fn resolve_close(
    window: WebviewWindow,
    service: State<'_, AppTimerService>,
    decision: CloseDecision,
) -> Result<(), String> {
    let should_destroy = resolve_close_decision(decision, || {
        service
            .prepare_close()
            .map(|_| ())
            .map_err(|error| error.to_string())
    })?;
    if should_destroy {
        window.destroy().map_err(|error| error.to_string())
    } else {
        Ok(())
    }
}

fn resolve_close_decision(
    decision: CloseDecision,
    prepare_close: impl FnOnce() -> Result<(), String>,
) -> Result<bool, String> {
    match decision {
        CloseDecision::Retry => prepare_close().map(|()| true),
        CloseDecision::Cancel => Ok(false),
        CloseDecision::ExitWithoutSaving => Ok(true),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::{
            atomic::{AtomicU64, Ordering},
            Arc,
        },
    };

    use crate::{
        clock::Clock,
        service::{TimerPhase, TimerService},
    };

    use super::{dispatch_action, TimerAction};

    struct FixedClock;

    impl Clock for FixedClock {
        fn now_100ns(&self) -> io::Result<u64> {
            Ok(100)
        }
    }

    struct MutableClock(Arc<AtomicU64>);

    impl Clock for MutableClock {
        fn now_100ns(&self) -> io::Result<u64> {
            Ok(self.0.load(Ordering::Relaxed))
        }
    }

    #[test]
    fn dispatches_action_against_expected_revision() {
        let service = TimerService::new(FixedClock);

        let snapshot = dispatch_action(
            &service,
            TimerAction::StartFocus {
                expected_revision: 0,
            },
        )
        .unwrap();

        assert_eq!(snapshot.phase, TimerPhase::FocusRunning);
        assert!(dispatch_action(
            &service,
            TimerAction::PauseFocus {
                expected_revision: 0,
            },
        )
        .is_err());
    }

    #[test]
    fn rejects_unsupported_ratio_without_changing_revision() {
        let service = TimerService::new(FixedClock);

        assert!(dispatch_action(
            &service,
            TimerAction::SetRatio {
                expected_revision: 0,
                ratio: 2,
            },
        )
        .is_err());
        assert_eq!(service.snapshot().unwrap().revision, 0);
    }

    #[test]
    fn dispatches_finish_and_reset_actions() {
        let service = TimerService::new(FixedClock);
        dispatch_action(
            &service,
            TimerAction::StartFocus {
                expected_revision: 0,
            },
        )
        .unwrap();
        let finished = dispatch_action(
            &service,
            TimerAction::FinishFocus {
                expected_revision: 1,
            },
        )
        .unwrap();
        assert_eq!(finished.phase, TimerPhase::Idle);

        dispatch_action(
            &service,
            TimerAction::StartFocus {
                expected_revision: 2,
            },
        )
        .unwrap();
        let reset = dispatch_action(
            &service,
            TimerAction::ResetFocus {
                expected_revision: 3,
            },
        )
        .unwrap();
        assert_eq!(reset.phase, TimerPhase::Idle);
    }

    #[test]
    fn dispatches_break_actions_against_expected_revision() {
        let now = Arc::new(AtomicU64::new(0));
        let service = TimerService::new(MutableClock(Arc::clone(&now)));
        dispatch_action(
            &service,
            TimerAction::StartFocus {
                expected_revision: 0,
            },
        )
        .unwrap();
        now.store(30 * 60 * 10_000_000, Ordering::Relaxed);
        dispatch_action(
            &service,
            TimerAction::FinishFocus {
                expected_revision: 1,
            },
        )
        .unwrap();

        let running = dispatch_action(
            &service,
            TimerAction::EndBreak {
                expected_revision: 2,
            },
        )
        .unwrap();
        assert_eq!(running.phase, TimerPhase::BreakCompleted);
    }

    #[test]
    fn close_decisions_use_camel_case_values() {
        let decision: super::CloseDecision = serde_json::from_str("\"exitWithoutSaving\"").unwrap();

        assert!(matches!(decision, super::CloseDecision::ExitWithoutSaving));
    }

    #[test]
    fn close_decisions_retry_cancel_or_bypass_saving() {
        let failed_retry = super::resolve_close_decision(super::CloseDecision::Retry, || {
            Err("write failed".to_owned())
        });
        assert_eq!(failed_retry, Err("write failed".to_owned()));

        assert_eq!(
            super::resolve_close_decision(super::CloseDecision::Cancel, || unreachable!()),
            Ok(false)
        );
        assert_eq!(
            super::resolve_close_decision(
                super::CloseDecision::ExitWithoutSaving,
                || unreachable!()
            ),
            Ok(true)
        );
    }
}
