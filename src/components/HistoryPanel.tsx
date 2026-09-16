import type { FocusHistoryEntry } from "../flowmodoro";
import { formatElapsedTime } from "../formatTime";

interface HistoryPanelProps {
  history: FocusHistoryEntry[];
}

function HistoryPanel({ history }: HistoryPanelProps) {
  return (
    <fieldset>
      <legend>Histórico de foco</legend>
      {history.length === 0 ? (
        <p>Nenhum foco finalizado ainda.</p>
      ) : (
        <ul className="history-list">
          {history.map((entry) => (
            <li key={entry.id}>
              <strong>{formatElapsedTime(entry.focusElapsedMs)}</strong>
              {" em "}
              {new Intl.DateTimeFormat("pt-BR", {
                dateStyle: "short",
                timeStyle: "short",
              }).format(entry.completedAtUnixMs)}
              {` | ${entry.ratio}:1 | descanso ${formatElapsedTime(entry.breakEarnedMs)}`}
            </li>
          ))}
        </ul>
      )}
    </fieldset>
  );
}

export default HistoryPanel;
