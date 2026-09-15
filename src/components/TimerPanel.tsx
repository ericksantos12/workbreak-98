import type { TimerActionType, TimerSnapshot } from "../flowmodoro";
import { formatElapsedTime } from "../formatTime";

interface TimerPanelProps {
  snapshot: TimerSnapshot | null;
  isLoading: boolean;
  isActionPending: boolean;
  onAction(action: TimerActionType): void;
}

function TimerPanel({
  snapshot,
  isLoading,
  isActionPending,
  onAction,
}: TimerPanelProps) {
  const disabled = isLoading || isActionPending || !snapshot;

  return (
    <section className="timer-panel" aria-labelledby="focus-title">
      <fieldset>
        <legend id="focus-title">Cronômetro de foco</legend>
        <p className="timer-readout" aria-label="Tempo de foco decorrido">
          {formatElapsedTime(snapshot?.focusElapsedMs ?? 0)}
        </p>
        <p className="timer-meta">
          <span>Modo: tempo livre</span>
          <strong>{phaseLabel(snapshot?.phase)}</strong>
        </p>
      </fieldset>

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
          <button
            type="button"
            disabled={disabled}
            onClick={() => onAction("pauseFocus")}
          >
            Pausar
          </button>
        )}
        {snapshot?.phase === "focusPaused" && (
          <button
            className="default"
            type="button"
            disabled={disabled}
            onClick={() => onAction("resumeFocus")}
          >
            Retomar
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
    default:
      return "Aguardando";
  }
}

export default TimerPanel;
