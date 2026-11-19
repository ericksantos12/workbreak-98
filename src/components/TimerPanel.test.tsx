import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { TimerPhase, TimerSnapshot } from "../flowmodoro";
import HeroPanel from "./HeroPanel";
import TimerPanel from "./TimerPanel";

const snapshot: TimerSnapshot = {
  revision: 0,
  phase: "idle",
  focusElapsedMs: 0,
  ratio: 3,
  breakEarnedMs: 0,
  breakRemainingMs: 0,
  breakElapsedMs: 0,
  persistenceStatus: "available",
  focusBlockMs: 0,
  progression: { level: 1, xpIntoLevel: 0, xpForNextLevel: 60, totalXp: 0, lastReward: null },
};

describe("timer actions", () => {
  it("keeps the completed focus visible during the break", () => {
    const html = renderToStaticMarkup(
      <TimerPanel
        snapshot={{ ...snapshot, phase: "breakReady", focusElapsedMs: 30 * 60_000 }}
        isLoading={false}
        isActionPending={false}
        onAction={() => {}}
        onRatioChange={() => {}}
      />,
    );
    expect(html).toContain("Foco concluído: <strong>00:30:00</strong>");
  });

  it("returns to the focus display after the break without starting it", () => {
    const html = renderToStaticMarkup(
      <TimerPanel
        snapshot={{ ...snapshot, phase: "breakCompleted", focusElapsedMs: 30 * 60_000 }}
        isLoading={false}
        isActionPending={false}
        onAction={() => {}}
        onRatioChange={() => {}}
      />,
    );
    expect(html).toContain('aria-label="Foco decorrido: 00:30:00"');
    expect(html).toContain("Iniciar novo foco");
  });

  it.each([
    ["idle", ["Iniciar foco"]],
    ["focusRunning", ["Pausar foco", "Finalizar foco"]],
    ["focusPaused", ["Retomar foco", "Finalizar foco"]],
    ["breakReady", ["Iniciar descanso"]],
    ["breakRunning", ["Pausar descanso", "Finalizar descanso"]],
    ["breakPaused", ["Retomar descanso", "Finalizar descanso"]],
    ["breakCompleted", ["Iniciar novo foco"]],
  ] as const)("shows the available actions in %s", (phase, expected) => {
    const html = renderToStaticMarkup(
      <TimerPanel
        snapshot={{ ...snapshot, phase: phase as TimerPhase }}
        isLoading={false}
        isActionPending={false}
        onAction={() => {}}
        onRatioChange={() => {}}
      />,
    );
    const actionRow = html.match(/<div class="action-row">([\s\S]*?)<\/div>/)?.[1] ?? "";
    const labels = [...actionRow.matchAll(/<button[^>]*>(.*?)<\/button>/g)].map((match) => match[1]);
    expect(labels).toEqual(expected);
  });
});

describe("hero panel", () => {
  it("shows level, progress and the sprite state for the phase", () => {
    const html = renderToStaticMarkup(
      <HeroPanel
        phase="focusRunning"
        focusBlockMs={30 * 60_000}
        progression={{ level: 2, xpIntoLevel: 45, xpForNextLevel: 90, totalXp: 105, lastReward: null }}
        reward={null}
        onDismissReward={() => {}}
      />,
    );
    expect(html).toContain("Nível 2");
    expect(html).toContain("45 / 90 XP");
    expect(html).toContain('aria-valuenow="50"');
    expect(html).toContain('aria-label="Progresso para o nível 3"');
    expect(html).toContain("hero-sprite--adventure");
    expect(html).toContain("Nível 2</strong> · Em aventura");
    expect(html).toContain("hero-stage--trail is-traveling");
    expect(html).not.toContain("camp-fire");
    expect(html).toContain("foe foe--ogre");
    expect(html).toContain("spell-bolt");
  });

  it("announces a reward with level up", () => {
    const html = renderToStaticMarkup(
      <HeroPanel
        phase="breakRunning"
        focusBlockMs={0}
        progression={{ level: 3, xpIntoLevel: 5, xpForNextLevel: 120, totalXp: 155, lastReward: null }}
        reward={{ id: 3, kind: "focus", xp: 40, levelUpTo: 3 }}
        onDismissReward={() => {}}
      />,
    );
    expect(html).toContain("Bloco concluído!");
    expect(html).toContain("+40 XP");
    expect(html).toContain("Subiu para o nível 3!");
    expect(html).toContain("hero-sprite--camp");
    expect(html).toContain("hero-stage--camp");
    expect(html).toContain("camp-fire");
    expect(html).not.toContain("foe ");
  });
});
