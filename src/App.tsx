import "98.css";
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
        <div className="title-bar">
          <div className="title-bar-text" id="app-title">
            Flowmodoro
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
