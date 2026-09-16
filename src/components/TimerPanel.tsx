import type {
  FocusRatio,
  TimerActionType,
  TimerSnapshot,
} from "../flowmodoro";
import { formatElapsedTime, formatRemainingTime } from "../formatTime";
import { useState } from "react";

interface TimerPanelProps {
  snapshot: TimerSnapshot | null;
  isLoading: boolean;
  isActionPending: boolean;
  onAction(action: TimerActionType): void;
  onRatioChange(ratio: FocusRatio): void;
}

function TimerPanel({
  snapshot,
  isLoading,
  isActionPending,
  onAction,
  onRatioChange,
}: TimerPanelProps) {
  const [resetPending, setResetPending] = useState(false);
  const disabled = isLoading || isActionPending || !snapshot;
  const isBreakPhase = snapshot?.phase.startsWith("break") ?? false;
  const breakBalanceMs = isBreakPhase
    ? (snapshot?.breakRemainingMs ?? 0)
    : (snapshot?.breakEarnedMs ?? 0);

  return (
    <section className="timer-panel" aria-labelledby="focus-title">
      <div className="timer-dashboard">
        <fieldset>
          <legend id="focus-title">Foco</legend>
          <p className="timer-readout" aria-label="Tempo de foco decorrido">
            {formatElapsedTime(snapshot?.focusElapsedMs ?? 0)}
          </p>
        </fieldset>
        <fieldset className="break-balance">
          <legend>Descanso conquistado</legend>
          <p className="timer-readout" aria-label="Saldo de descanso restante">
            {formatRemainingTime(breakBalanceMs)}
          </p>
        </fieldset>
        <fieldset>
          <legend>Descanso decorrido</legend>
          <p className="timer-readout" aria-label="Tempo de descanso decorrido">
            {formatElapsedTime(snapshot?.breakElapsedMs ?? 0)}
          </p>
        </fieldset>
      </div>
      <p className="timer-meta">
        <span>Modo: tempo livre</span>
        <strong>{phaseLabel(snapshot?.phase)}</strong>
      </p>

      <div className="field-row">
        <label htmlFor="focus-ratio">Proporção foco/descanso:</label>
        <select
          id="focus-ratio"
          value={snapshot?.ratio ?? 3}
          disabled={disabled}
          onChange={(event) =>
            onRatioChange(Number(event.currentTarget.value) as FocusRatio)
          }
        >
          <option value={3}>3:1 (padrão)</option>
          <option value={4}>4:1</option>
          <option value={5}>5:1</option>
        </select>
      </div>

      <p className="timer-meta">Proporção atual: {snapshot?.ratio ?? 3}:1</p>

      <div className="timer-actions" aria-label="Controles do foco">
        {(!snapshot || snapshot.phase === "idle") && (
          <button
            className="default"
            type="button"
            disabled={disabled}
            onClick={() => onAction("startFocus")}
          >
            Iniciar foco
          </button>
        )}
        {snapshot?.phase === "focusRunning" && (
          <>
            <button
              type="button"
              disabled={disabled}
              onClick={() => onAction("pauseFocus")}
            >
              Pausar
            </button>
            <button
              type="button"
              disabled={disabled}
              onClick={() => onAction("finishFocus")}
            >
              Finalizar foco
            </button>
          </>
        )}
        {snapshot?.phase === "focusPaused" && (
          <>
            <button
              className="default"
              type="button"
              disabled={disabled}
              onClick={() => onAction("resumeFocus")}
            >
              Retomar
            </button>
            <button
              type="button"
              disabled={disabled}
              onClick={() => onAction("finishFocus")}
            >
              Finalizar foco
            </button>
          </>
        )}
        {(snapshot?.phase === "breakRunning" ||
          snapshot?.phase === "breakPaused") && (
          <>
            {snapshot.phase === "breakRunning" && (
              <button
                type="button"
                disabled={disabled}
                onClick={() => onAction("pauseBreak")}
              >
                Pausar descanso
              </button>
            )}
            {snapshot.phase === "breakPaused" && (
              <button
                className="default"
                type="button"
                disabled={disabled}
                onClick={() => onAction("resumeBreak")}
              >
                Retomar descanso
              </button>
            )}
            <button
              type="button"
              disabled={disabled}
              onClick={() => onAction("endBreak")}
            >
              Finalizar descanso
            </button>
          </>
        )}
        {snapshot?.phase === "breakCompleted" && (
          <>
            <button
              className="default"
              type="button"
              disabled={disabled}
              onClick={() => onAction("startFocus")}
            >
              Iniciar novo foco
            </button>
          </>
        )}
        {snapshot?.phase !== "idle" && (
          <button
            type="button"
            disabled={disabled}
            onClick={() => {
              if (resetPending) {
                setResetPending(false);
                onAction("resetFocus");
              } else {
                setResetPending(true);
              }
            }}
          >
            {resetPending ? "Confirmar Reset?" : "Reset"}
          </button>
        )}
      </div>
    </section>
  );
}

function phaseLabel(phase: TimerSnapshot["phase"] | undefined) {
  switch (phase) {
    case "focusRunning":
      return "Em foco";
    case "focusPaused":
      return "Pausado";
    case "breakReady":
      return "Descanso disponível";
    case "breakRunning":
      return "Descansando";
    case "breakPaused":
      return "Descanso pausado";
    case "breakCompleted":
      return "Descanso concluído";
    default:
      return "Aguardando";
  }
}

export default TimerPanel;
