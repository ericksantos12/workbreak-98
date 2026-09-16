import "98.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./App.css";
import TimerPanel from "./components/TimerPanel";
import { useFlowmodoro } from "./useFlowmodoro";

function App() {
  const { snapshot, error, isLoading, isActionPending, act, setRatio } =
    useFlowmodoro();

  function runAction(action: "startFocus" | "pauseFocus" | "resumeFocus") {
    void act(action).catch(() => {
      // The controller exposes command failures in its state.
    });
  }

  function changeRatio(ratio: 3 | 4 | 5) {
    void setRatio(ratio).catch(() => {
      // The controller exposes command failures in its state.
    });
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

function statusLabel(phase: "idle" | "focusRunning" | "focusPaused") {
  switch (phase) {
    case "focusRunning":
      return "Foco em andamento";
    case "focusPaused":
      return "Foco pausado";
    default:
      return "Pronto para focar";
  }
}

export default App;
