use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    clock::Clock,
    service::{AppTimerService, TimerService, TimerSnapshot},
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
}

pub fn dispatch_action<C: Clock>(
    service: &TimerService<C>,
    action: TimerAction,
) -> Result<TimerSnapshot, String> {
    match action {
        TimerAction::StartFocus { expected_revision } => service.start_focus(expected_revision),
        TimerAction::PauseFocus { expected_revision } => service.pause_focus(expected_revision),
        TimerAction::ResumeFocus { expected_revision } => service.resume_focus(expected_revision),
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

#[cfg(test)]
mod tests {
    use std::io;

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
}
