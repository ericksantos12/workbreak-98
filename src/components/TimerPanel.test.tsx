import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { TimerPhase, TimerSnapshot } from "../flowmodoro";
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
