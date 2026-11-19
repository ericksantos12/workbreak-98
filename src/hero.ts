import type { ProgressionSnapshot, Reward, TimerPhase } from "./flowmodoro";

export type HeroState = "idle" | "adventure" | "sleep" | "camp";

const captions: Record<HeroState, string> = {
  idle: "Pronto para a aventura",
  adventure: "Em aventura",
  sleep: "Cochilando",
  camp: "Descansando no acampamento",
};

/** Sprite state shown for each timer phase. Derived only from the snapshot. */
export function heroStateFor(phase: TimerPhase): HeroState {
  switch (phase) {
    case "focusRunning":
      return "adventure";
    case "focusPaused":
    case "breakPaused":
      return "sleep";
    case "breakReady":
    case "breakRunning":
      return "camp";
    default:
      return "idle";
  }
}

export type HeroScene = "trail" | "camp";

/** The trail scrolls while focusing; breaks happen at the campsite. */
export function heroSceneFor(phase: TimerPhase): HeroScene {
  return phase === "breakReady" || phase === "breakRunning" || phase === "breakPaused" ? "camp" : "trail";
}

export type FoeTier = "slime" | "goblin" | "ogre" | "golem";

const MINUTE_MS = 60_000;

/** Foes grow with the current focus block: slime < 10 min, goblin < 25, ogre < 50, golem after. */
export function foeTierFor(focusBlockMs: number): FoeTier {
  if (focusBlockMs < 10 * MINUTE_MS) return "slime";
  if (focusBlockMs < 25 * MINUTE_MS) return "goblin";
  if (focusBlockMs < 50 * MINUTE_MS) return "ogre";
  return "golem";
}

export function heroCaption(state: HeroState): string {
  return captions[state];
}

/** Share of the current level already earned, from 0 to 100. Full at the maximum level. */
export function levelProgressPercent(progression: ProgressionSnapshot): number {
  if (progression.xpForNextLevel <= 0) return 100;
  return Math.min(100, Math.floor((progression.xpIntoLevel / progression.xpForNextLevel) * 100));
}

export function rewardTitle(reward: Reward): string {
  return reward.kind === "focus" ? "Bloco concluído!" : "Descanso respeitado!";
}

export function rewardLines(reward: Reward): string[] {
  const lines = [`+${reward.xp} XP`];
  if (reward.levelUpTo !== null) lines.push(`Subiu para o nível ${reward.levelUpTo}!`);
  return lines;
}

/**
 * Tracks which reward ids were already shown in this run. The first snapshot only sets the
 * baseline, so a reward earned before the app was reopened is not announced again.
 */
export function createRewardTracker() {
  let seenId: number | null = null;
  return {
    /** Returns the reward to announce for this snapshot, if it is new. */
    next(progression: ProgressionSnapshot | undefined): Reward | null {
      if (!progression) return null;
      const reward = progression.lastReward;
      const id = reward?.id ?? 0;
      if (seenId === null) {
        seenId = id;
        return null;
      }
      if (reward && id > seenId) {
        seenId = id;
        return reward;
      }
      return null;
    },
  };
}
