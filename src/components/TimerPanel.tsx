import { useEffect, useState } from "react";
import type { FocusRatio, TimerActionType, TimerSnapshot } from "../flowmodoro";
import { formatElapsedTime, formatRemainingTime } from "../formatTime";

interface TimerPanelProps {
  snapshot: TimerSnapshot | null;
  isLoading: boolean;
  isActionPending: boolean;
  onAction(action: TimerActionType): void;
  onRatioChange(ratio: FocusRatio): void;
}

function TimerPanel({ snapshot, isLoading, isActionPending, onAction, onRatioChange }: TimerPanelProps) {
  const [resetPending, setResetPending] = useState(false);
  const phase = snapshot?.phase ?? "idle";
  const disabled = isLoading || isActionPending || !snapshot;
  const onBreak = phase.startsWith("break");
  const showFocus = !onBreak || phase === "breakCompleted";
  const balance = onBreak ? snapshot?.breakRemainingMs ?? 0 : snapshot?.breakEarnedMs ?? 0;

  useEffect(() => setResetPending(false), [phase]);

  const primary = (() => {
    switch (phase) {
      case "focusRunning": return { label: "Pausar foco", action: "pauseFocus" as const };
      case "focusPaused": return { label: "Retomar foco", action: "resumeFocus" as const };
      case "breakReady": return { label: "Iniciar descanso", action: "startBreak" as const };
      case "breakRunning": return { label: "Pausar descanso", action: "pauseBreak" as const };
      case "breakPaused": return { label: "Retomar descanso", action: "resumeBreak" as const };
      case "breakCompleted": return { label: "Iniciar novo foco", action: "startFocus" as const };
      default: return { label: "Iniciar foco", action: "startFocus" as const };
    }
  })();
  const secondary = phase === "focusRunning" || phase === "focusPaused"
    ? { label: "Finalizar foco", action: "finishFocus" as const }
    : phase === "breakRunning" || phase === "breakPaused"
      ? { label: "Finalizar descanso", action: "endBreak" as const }
      : null;

  return (
    <section className="timer-panel" aria-labelledby="timer-title">
      <div className="section-heading">
        <h1 id="timer-title">Cronômetro</h1>
      </div>
      <div className="timer-workspace">
        <fieldset className="main-clock">
          <legend>{showFocus ? "Foco" : "Descanso"}</legend>
          <div className="clock-face" role="timer" aria-label={`${showFocus ? "Foco decorrido" : "Descanso decorrido"}: ${formatElapsedTime(showFocus ? snapshot?.focusElapsedMs ?? 0 : snapshot?.breakElapsedMs ?? 0)}`}>
            {formatElapsedTime(showFocus ? snapshot?.focusElapsedMs ?? 0 : snapshot?.breakElapsedMs ?? 0)}
          </div>
          <p className="clock-caption">{showFocus ? "Tempo de foco nesta sessão" : "Tempo descansado nesta sessão"}</p>
          {onBreak && phase !== "breakCompleted" && <p className="focus-summary">Foco concluído: <strong>{formatElapsedTime(snapshot?.focusElapsedMs ?? 0)}</strong></p>}
          <div className="action-row">
            <button className="default primary-action" type="button" disabled={disabled} onClick={() => onAction(primary.action)}>{primary.label}</button>
            {secondary && <button type="button" disabled={disabled} onClick={() => onAction(secondary.action)}>{secondary.label}</button>}
          </div>
        </fieldset>
        <div className="side-panel">
          <fieldset className="balance-panel">
            <legend>{onBreak ? "Descanso restante" : "Descanso conquistado"}</legend>
            <div className="balance-face" aria-label={`${onBreak ? "Descanso restante" : "Descanso conquistado"}: ${formatRemainingTime(balance)}`}>
              {formatRemainingTime(balance)}
            </div>
            <p>{onBreak ? "Saldo disponível para esta pausa" : "Acumulado durante o foco"}</p>
          </fieldset>
          <fieldset className="settings-panel">
            <legend>Configuração</legend>
            <label htmlFor="focus-ratio">Proporção foco / descanso</label>
            <select id="focus-ratio" value={snapshot?.ratio ?? 3} disabled={disabled} onChange={(event) => onRatioChange(Number(event.currentTarget.value) as FocusRatio)}>
              <option value={3}>3:1 — 3 min de foco = 1 min de descanso</option>
              <option value={4}>4:1 — 4 min de foco = 1 min de descanso</option>
              <option value={5}>5:1 — 5 min de foco = 1 min de descanso</option>
            </select>
            <p>O tempo de descanso é calculado a partir do foco.</p>
          </fieldset>
        </div>
      </div>
      <div className="session-footer">
        <span>Modo livre · sessão local</span>
        {phase !== "idle" && (
          <div className="reset-controls">
            {resetPending && <button type="button" disabled={disabled} onClick={() => setResetPending(false)}>Cancelar</button>}
            <button type="button" disabled={disabled} onClick={() => {
              if (resetPending) { setResetPending(false); onAction("resetFocus"); }
              else setResetPending(true);
            }}>{resetPending ? "Confirmar reinício" : "Reiniciar sessão"}</button>
          </div>
        )}
      </div>
    </section>
  );
}

export default TimerPanel;
