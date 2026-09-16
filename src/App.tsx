import "98.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import "./App.css";
import HistoryPanel from "./components/HistoryPanel";
import TimerPanel from "./components/TimerPanel";
import {
  type CloseDecision,
  onCloseSaveFailed,
  resolveClose,
} from "./flowmodoro";
import { useFlowmodoro } from "./useFlowmodoro";

function App() {
  const [closeSaveFailed, setCloseSaveFailed] = useState(false);
  const { snapshot, error, isLoading, isActionPending, act, setRatio } =
    useFlowmodoro();

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onCloseSaveFailed(() => setCloseSaveFailed(true)).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  function runAction(
    action:
      | "startFocus"
      | "pauseFocus"
      | "resumeFocus"
      | "finishFocus"
      | "resetFocus"
      | "startBreak"
      | "pauseBreak"
      | "resumeBreak"
      | "endBreak",
  ) {
    if (
      action === "resetFocus" &&
      !window.confirm(
        "Resetar o foco atual? Ele será descartado e não aparecerá no histórico.",
      )
    ) {
      return;
    }
    void act(action).catch(() => {
      // The controller exposes command failures in its state.
    });
  }

  function changeRatio(ratio: 3 | 4 | 5) {
    void setRatio(ratio).catch(() => {
      // The controller exposes command failures in its state.
    });
  }

  function decideClose(decision: CloseDecision) {
    if (decision === "cancel") setCloseSaveFailed(false);
    void resolveClose(decision).catch(() => setCloseSaveFailed(true));
  }

  return (
    <main className="desktop-shell">
      <section className="window app-window" aria-labelledby="app-title">
        <div className="title-bar" data-tauri-drag-region>
          <div className="title-bar-text" id="app-title" data-tauri-drag-region>
            WorkBreak 98
          </div>
          <div className="title-bar-controls">
            <button
              type="button"
              aria-label="Minimize"
              onClick={() => void getCurrentWindow().minimize()}
            />
            <button
              type="button"
              aria-label="Close"
              onClick={() => void getCurrentWindow().close()}
            />
          </div>
        </div>

        <div className="window-body app-body">
          <TimerPanel
            snapshot={snapshot}
            isLoading={isLoading}
            isActionPending={isActionPending}
            onAction={runAction}
            onRatioChange={changeRatio}
          />
          <HistoryPanel history={snapshot?.history ?? []} />

          {error && (
            <p className="error-message" role="alert">
              Não foi possível acessar o cronômetro: {error}
            </p>
          )}
          {snapshot?.persistenceStatus === "unavailable" && (
            <p className="error-message" role="alert">
              Não foi possível salvar a sessão. O cronômetro continua funcionando,
              mas a recuperação não está garantida.
            </p>
          )}
          {snapshot?.persistenceStatus === "blocked" && (
            <p className="error-message" role="alert">
              Os dados locais são inválidos ou incompatíveis. O arquivo original
              não será sobrescrito.
            </p>
          )}
          {closeSaveFailed && (
            <fieldset role="alertdialog" aria-labelledby="close-error-title">
              <legend id="close-error-title">Falha ao salvar</legend>
              <p>
                O fechamento seguro não pôde ser concluído. Tente novamente,
                cancele o fechamento ou saia sem salvar as alterações recentes.
              </p>
              <div className="timer-actions">
                <button type="button" onClick={() => decideClose("retry")}>
                  Tentar novamente
                </button>
                <button type="button" onClick={() => decideClose("cancel")}>
                  Cancelar
                </button>
                <button
                  type="button"
                  onClick={() => decideClose("exitWithoutSaving")}
                >
                  Sair sem salvar
                </button>
              </div>
            </fieldset>
          )}
        </div>

        <div className="status-bar" aria-live="polite">
          <p className="status-bar-field">
            {snapshot
              ? statusLabel(snapshot.phase)
              : isLoading
                ? "Conectando ao serviço local..."
                : "Serviço local indisponível"}
          </p>
          <p className="status-bar-field">Sessão local</p>
        </div>
      </section>
    </main>
  );
}

function statusLabel(
  phase:
    | "idle"
    | "focusRunning"
    | "focusPaused"
    | "breakReady"
    | "breakRunning"
    | "breakPaused"
    | "breakCompleted",
) {
  switch (phase) {
    case "focusRunning":
      return "Foco em andamento";
    case "focusPaused":
      return "Foco pausado";
    case "breakReady":
      return "Descanso disponível";
    case "breakRunning":
      return "Descanso em andamento";
    case "breakPaused":
      return "Descanso pausado";
    case "breakCompleted":
      return "Descanso concluído";
    default:
      return "Pronto para focar";
  }
}

export default App;
