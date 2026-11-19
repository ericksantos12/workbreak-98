//! Hero progression: experience earned from healthy focus and respected breaks.
//!
//! Pure domain logic with no I/O or clock. The service feeds it timer transitions
//! (`observe`) under the same lock and persists it in the same write as the timer,
//! so a recovered checkpoint can never grant the same reward twice.

use serde::{Deserialize, Serialize};

use crate::timer::TimerState;

pub const MAX_LEVEL: u32 = 99;
/// XP cap for a single focus block (two hours): longer blocks earn nothing extra.
pub const FOCUS_XP_CAP: u64 = 120;
const MINUTE_100NS: u64 = 60 * 10_000_000;
const REST_BONUS_PERCENT: u64 = 20;
/// The rest bonus requires at least 90% of the available break to be rested.
const REST_THRESHOLD_NUMERATOR: i128 = 9;
const REST_THRESHOLD_DENOMINATOR: i128 = 10;
const MAX_PENDING_REST_BONUS_XP: u64 = FOCUS_XP_CAP * REST_BONUS_PERCENT / 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RewardKind {
    Focus,
    Rest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reward {
    pub id: u64,
    pub kind: RewardKind,
    pub xp: u64,
    pub level_up_to: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Progression {
    total_xp: u64,
    focus_block_start_100ns: u64,
    pending_rest_bonus_xp: u64,
    last_reward: Option<Reward>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressionSnapshot {
    pub level: u32,
    pub xp_into_level: u64,
    /// XP needed to go from the current level to the next; 0 at the maximum level.
    pub xp_for_next_level: u64,
    pub total_xp: u64,
    pub last_reward: Option<Reward>,
}

/// XP needed to go from `level` to `level + 1`.
pub fn xp_for_level_up(level: u32) -> u64 {
    60 + 30 * u64::from(level.saturating_sub(1))
}

/// Level, XP into that level and XP needed for the next one, derived from total XP.
pub fn level_for(total_xp: u64) -> (u32, u64, u64) {
    let mut level = 1;
    let mut remaining = total_xp;
    while level < MAX_LEVEL {
        let needed = xp_for_level_up(level);
        if remaining < needed {
            return (level, remaining, needed);
        }
        remaining -= needed;
        level += 1;
    }
    (MAX_LEVEL, remaining, 0)
}

/// Focus XP for a block: one per full minute of effective focus, capped per block.
pub fn focus_xp(block_100ns: u64) -> u64 {
    (block_100ns / MINUTE_100NS).min(FOCUS_XP_CAP)
}

impl Progression {
    /// Fresh progression whose current focus block starts at `focus_elapsed_100ns`.
    pub fn starting_at(focus_elapsed_100ns: u64) -> Self {
        Self {
            focus_block_start_100ns: focus_elapsed_100ns,
            ..Self::default()
        }
    }

    /// Effective focus in the current block (0 outside a focus block).
    pub fn focus_block_100ns(&self, focus_elapsed_100ns: u64) -> u64 {
        focus_elapsed_100ns.saturating_sub(self.focus_block_start_100ns)
    }

    pub fn total_xp(&self) -> u64 {
        self.total_xp
    }

    pub fn last_reward(&self) -> Option<Reward> {
        self.last_reward
    }

    pub fn snapshot(&self) -> ProgressionSnapshot {
        let (level, xp_into_level, xp_for_next_level) = level_for(self.total_xp);
        ProgressionSnapshot {
            level,
            xp_into_level,
            xp_for_next_level,
            total_xp: self.total_xp,
            last_reward: self.last_reward,
        }
    }

    /// Whether persisted progression fits the restored timer's effective focus.
    pub fn is_consistent_with(&self, focus_elapsed_100ns: u64) -> bool {
        self.pending_rest_bonus_xp <= MAX_PENDING_REST_BONUS_XP
            && self.focus_block_start_100ns <= focus_elapsed_100ns
            && self.last_reward.is_none_or(|reward| reward.id > 0)
    }

    /// Reacts to one timer transition. Returns the reward granted by it, if any.
    pub fn observe(&mut self, before: TimerState, after: TimerState) -> Option<Reward> {
        match (before, after) {
            (
                TimerState::Idle | TimerState::BreakCompleted { .. },
                TimerState::FocusRunning {
                    accumulated_100ns, ..
                },
            ) => {
                self.focus_block_start_100ns = accumulated_100ns;
                self.pending_rest_bonus_xp = 0;
                None
            }
            (
                TimerState::FocusRunning { .. } | TimerState::FocusPaused { .. },
                TimerState::BreakReady {
                    focus_elapsed_100ns,
                    ..
                },
            ) => self.finish_focus_block(focus_elapsed_100ns),
            (
                TimerState::BreakReady { .. }
                | TimerState::BreakRunning { .. }
                | TimerState::BreakPaused { .. },
                TimerState::BreakCompleted {
                    break_earned_100ns,
                    remaining_100ns,
                    ..
                },
            ) => self.complete_break(break_earned_100ns, remaining_100ns),
            (before, TimerState::Idle) if !matches!(before, TimerState::Idle) => {
                self.focus_block_start_100ns = 0;
                self.pending_rest_bonus_xp = 0;
                None
            }
            _ => None,
        }
    }

    fn finish_focus_block(&mut self, focus_elapsed_100ns: u64) -> Option<Reward> {
        let block_100ns = focus_elapsed_100ns.saturating_sub(self.focus_block_start_100ns);
        let xp = focus_xp(block_100ns);
        self.focus_block_start_100ns = focus_elapsed_100ns;
        self.pending_rest_bonus_xp = xp * REST_BONUS_PERCENT / 100;
        self.grant(RewardKind::Focus, xp)
    }

    fn complete_break(&mut self, earned_100ns: i128, remaining_100ns: i128) -> Option<Reward> {
        let bonus = std::mem::take(&mut self.pending_rest_bonus_xp);
        if earned_100ns <= 0 {
            return None;
        }
        let rested_100ns = earned_100ns - remaining_100ns.max(0);
        if rested_100ns * REST_THRESHOLD_DENOMINATOR < earned_100ns * REST_THRESHOLD_NUMERATOR {
            return None;
        }
        self.grant(RewardKind::Rest, bonus)
    }

    fn grant(&mut self, kind: RewardKind, xp: u64) -> Option<Reward> {
        if xp == 0 {
            return None;
        }
        let (level_before, ..) = level_for(self.total_xp);
        self.total_xp = self.total_xp.saturating_add(xp);
        let (level_after, ..) = level_for(self.total_xp);
        let reward = Reward {
            id: self
                .last_reward
                .map_or(1, |reward| reward.id.saturating_add(1)),
            kind,
            xp,
            level_up_to: (level_after > level_before).then_some(level_after),
        };
        self.last_reward = Some(reward);
        Some(reward)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: u64 = 10_000_000;
    const MINUTE: u64 = 60 * SECOND;

    fn focus_running(accumulated_100ns: u64) -> TimerState {
        TimerState::FocusRunning {
            accumulated_100ns,
            started_at_100ns: 0,
        }
    }

    fn break_ready(focus_elapsed_100ns: u64) -> TimerState {
        TimerState::BreakReady {
            focus_elapsed_100ns,
            break_earned_100ns: 1,
        }
    }

    fn break_running() -> TimerState {
        TimerState::BreakRunning {
            focus_elapsed_100ns: 1,
            break_earned_100ns: 1,
            remaining_at_start_100ns: 1,
            started_at_100ns: 0,
        }
    }

    fn break_completed(earned: u64, remaining: i128) -> TimerState {
        TimerState::BreakCompleted {
            focus_elapsed_100ns: 1,
            break_earned_100ns: i128::from(earned),
            remaining_100ns: remaining,
        }
    }

    /// Runs a focus block of `block_100ns` starting at `start_100ns`.
    fn run_block(
        progression: &mut Progression,
        start_100ns: u64,
        block_100ns: u64,
    ) -> Option<Reward> {
        let before = if start_100ns == 0 {
            TimerState::Idle
        } else {
            break_completed(MINUTE, 0)
        };
        progression.observe(before, focus_running(start_100ns));
        progression.observe(
            focus_running(start_100ns),
            break_ready(start_100ns + block_100ns),
        )
    }

    #[test]
    fn focus_xp_is_one_per_full_minute() {
        let mut progression = Progression::default();
        let reward = run_block(&mut progression, 0, 25 * MINUTE + 40 * SECOND).unwrap();

        assert_eq!(reward.kind, RewardKind::Focus);
        assert_eq!(reward.xp, 25);
        assert_eq!(reward.id, 1);
        assert_eq!(progression.pending_rest_bonus_xp, 5);
        assert_eq!(progression.total_xp(), 25);
    }

    #[test]
    fn focus_xp_is_capped_per_block() {
        let mut progression = Progression::default();
        let reward = run_block(&mut progression, 0, 3 * 60 * MINUTE).unwrap();

        assert_eq!(reward.xp, FOCUS_XP_CAP);
        assert_eq!(progression.pending_rest_bonus_xp, 24);
    }

    #[test]
    fn block_under_a_minute_grants_nothing() {
        let mut progression = Progression::default();

        assert_eq!(run_block(&mut progression, 0, 50 * SECOND), None);
        assert_eq!(progression.total_xp(), 0);
        assert_eq!(progression.last_reward(), None);
        assert_eq!(progression.pending_rest_bonus_xp, 0);
    }

    #[test]
    fn second_block_measures_only_new_focus() {
        let mut progression = Progression::default();
        run_block(&mut progression, 0, 30 * MINUTE);
        progression.observe(break_running(), break_completed(10 * MINUTE, 0));

        let reward = run_block(&mut progression, 30 * MINUTE, 20 * MINUTE).unwrap();

        assert_eq!(reward.xp, 20);
        assert_eq!(progression.total_xp(), 30 + 6 + 20);
    }

    #[test]
    fn rest_bonus_requires_ninety_percent_of_the_break() {
        let earned = 10 * MINUTE;
        let mut respected = Progression::default();
        run_block(&mut respected, 0, 30 * MINUTE);
        let reward = respected
            .observe(break_running(), break_completed(earned, i128::from(MINUTE)))
            .unwrap();
        assert_eq!(reward.kind, RewardKind::Rest);
        assert_eq!(reward.xp, 6);
        assert_eq!(reward.id, 2);

        let mut skipped = Progression::default();
        run_block(&mut skipped, 0, 30 * MINUTE);
        let remaining = i128::from(MINUTE + SECOND);
        assert_eq!(
            skipped.observe(break_running(), break_completed(earned, remaining)),
            None
        );
        assert_eq!(skipped.total_xp(), 30);
        assert_eq!(skipped.pending_rest_bonus_xp, 0);
    }

    #[test]
    fn overrun_break_counts_as_fully_rested() {
        let mut progression = Progression::default();
        run_block(&mut progression, 0, 30 * MINUTE);

        let reward = progression.observe(
            break_running(),
            break_completed(10 * MINUTE, -i128::from(MINUTE)),
        );

        assert_eq!(reward.map(|reward| reward.xp), Some(6));
    }

    #[test]
    fn no_available_break_discards_the_bonus() {
        let mut progression = Progression::default();
        run_block(&mut progression, 0, 30 * MINUTE);

        let completed = TimerState::BreakCompleted {
            focus_elapsed_100ns: 30 * MINUTE,
            break_earned_100ns: -i128::from(MINUTE),
            remaining_100ns: -i128::from(MINUTE),
        };
        assert_eq!(
            progression.observe(break_ready(30 * MINUTE), completed),
            None
        );
        assert_eq!(progression.pending_rest_bonus_xp, 0);
    }

    #[test]
    fn reset_discards_block_and_pending_bonus_but_keeps_xp() {
        let mut progression = Progression::default();
        run_block(&mut progression, 0, 30 * MINUTE);

        progression.observe(break_running(), TimerState::Idle);

        assert_eq!(progression.total_xp(), 30);
        assert_eq!(progression.pending_rest_bonus_xp, 0);
        assert_eq!(progression.focus_block_start_100ns, 0);
    }

    #[test]
    fn level_curve_matches_the_spec() {
        assert_eq!(level_for(0), (1, 0, 60));
        assert_eq!(level_for(59), (1, 59, 60));
        assert_eq!(level_for(60), (2, 0, 90));
        assert_eq!(level_for(1_620), (10, 0, 330));
        assert_eq!(level_for(u64::MAX).0, MAX_LEVEL);
        assert_eq!(level_for(u64::MAX).2, 0);
    }

    #[test]
    fn reward_reports_the_final_level_when_crossing_several() {
        let mut progression = Progression {
            total_xp: 59,
            ..Progression::default()
        };

        let reward = run_block(&mut progression, 0, 120 * MINUTE).unwrap();

        assert_eq!(progression.total_xp(), 179);
        assert_eq!(reward.level_up_to, Some(3));
    }

    #[test]
    fn consistency_check_rejects_impossible_state() {
        let valid = Progression {
            pending_rest_bonus_xp: 24,
            focus_block_start_100ns: 10,
            ..Progression::default()
        };
        assert!(valid.is_consistent_with(10));
        assert!(!valid.is_consistent_with(9));
        let bonus_too_big = Progression {
            pending_rest_bonus_xp: 25,
            ..Progression::default()
        };
        assert!(!bonus_too_big.is_consistent_with(0));
    }
}
