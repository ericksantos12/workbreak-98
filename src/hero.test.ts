import { describe, expect, it } from "vitest";
import type { ProgressionSnapshot, Reward, TimerPhase } from "./flowmodoro";
import {
  createRewardTracker,
  foeTierFor,
  heroCaption,
  heroSceneFor,
  heroStateFor,
  levelProgressPercent,
  rewardLines,
  rewardTitle,
} from "./hero";

function progression(overrides: Partial<ProgressionSnapshot> = {}): ProgressionSnapshot {
  return { level: 1, xpIntoLevel: 0, xpForNextLevel: 60, totalXp: 0, lastReward: null, ...overrides };
}

function reward(id: number, overrides: Partial<Reward> = {}): Reward {
  return { id, kind: "focus", xp: 25, levelUpTo: null, ...overrides };
}

describe("hero state", () => {
  it.each([
    ["idle", "idle", "Pronto para a aventura"],
    ["breakCompleted", "idle", "Pronto para a aventura"],
    ["focusRunning", "adventure", "Em aventura"],
    ["focusPaused", "sleep", "Cochilando"],
    ["breakPaused", "sleep", "Cochilando"],
    ["breakReady", "camp", "Descansando no acampamento"],
    ["breakRunning", "camp", "Descansando no acampamento"],
  ] as const)("maps %s to %s", (phase, state, caption) => {
    expect(heroStateFor(phase as TimerPhase)).toBe(state);
    expect(heroCaption(state)).toBe(caption);
  });
});

describe("hero scene", () => {
  it.each([
    ["idle", "trail"],
    ["focusRunning", "trail"],
    ["focusPaused", "trail"],
    ["breakReady", "camp"],
    ["breakRunning", "camp"],
    ["breakPaused", "camp"],
    ["breakCompleted", "trail"],
  ] as const)("places %s on the %s", (phase, scene) => {
    expect(heroSceneFor(phase as TimerPhase)).toBe(scene);
  });
});

describe("foe tier", () => {
  it.each([
    [0, "slime"],
    [9 * 60_000 + 59_999, "slime"],
    [10 * 60_000, "goblin"],
    [25 * 60_000, "ogre"],
    [49 * 60_000, "ogre"],
    [50 * 60_000, "golem"],
    [3 * 60 * 60_000, "golem"],
  ] as const)("after %i ms of focus is a %s", (ms, tier) => {
    expect(foeTierFor(ms)).toBe(tier);
  });
});

describe("level progress", () => {
  it("is the share of the current level", () => {
    expect(levelProgressPercent(progression({ xpIntoLevel: 45, xpForNextLevel: 90 }))).toBe(50);
  });

  it("is full at the maximum level", () => {
    expect(levelProgressPercent(progression({ level: 99, xpForNextLevel: 0 }))).toBe(100);
  });
});

describe("reward text", () => {
  it("describes focus, rest and level up", () => {
    expect(rewardTitle(reward(1))).toBe("Bloco concluído!");
    expect(rewardTitle(reward(1, { kind: "rest" }))).toBe("Descanso respeitado!");
    expect(rewardLines(reward(1, { xp: 30, levelUpTo: 3 }))).toEqual(["+30 XP", "Subiu para o nível 3!"]);
  });
});

describe("reward tracker", () => {
  it("does not announce the reward that existed when the app opened", () => {
    const tracker = createRewardTracker();
    expect(tracker.next(undefined)).toBeNull();
    expect(tracker.next(progression({ lastReward: reward(4) }))).toBeNull();
  });

  it("announces each new reward once", () => {
    const tracker = createRewardTracker();
    tracker.next(progression());

    expect(tracker.next(progression({ lastReward: reward(1) }))?.id).toBe(1);
    expect(tracker.next(progression({ lastReward: reward(1) }))).toBeNull();
    expect(tracker.next(progression({ lastReward: reward(2, { kind: "rest" }) }))?.kind).toBe("rest");
  });
});
