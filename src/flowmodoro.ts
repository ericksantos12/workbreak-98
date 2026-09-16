import { invoke } from "@tauri-apps/api/core";

export type TimerPhase = "idle" | "focusRunning" | "focusPaused";
export type FocusRatio = 3 | 4 | 5;
export type PersistenceStatus = "available" | "unavailable" | "blocked";

export interface TimerSnapshot {
  revision: number;
  phase: TimerPhase;
  focusElapsedMs: number;
  ratio: FocusRatio;
  breakEarnedMs: number;
  persistenceStatus: PersistenceStatus;
}

export type TimerAction =
  | { type: "startFocus"; expectedRevision: number }
  | { type: "pauseFocus"; expectedRevision: number }
  | { type: "resumeFocus"; expectedRevision: number }
  | { type: "setRatio"; expectedRevision: number; ratio: FocusRatio };

export function getTimerSnapshot(): Promise<TimerSnapshot> {
  return invoke<TimerSnapshot>("get_timer_snapshot");
}

export function applyTimerAction(
  action: TimerAction,
): Promise<TimerSnapshot> {
  return invoke<TimerSnapshot>("apply_timer_action", { action });
}

export interface TimerTransport {
  getSnapshot(): Promise<TimerSnapshot>;
  applyAction(action: TimerAction): Promise<TimerSnapshot>;
}

export interface Scheduler {
  setInterval(callback: () => void, milliseconds: number): unknown;
  clearInterval(handle: unknown): void;
}

export interface VisibilitySource {
  isVisible(): boolean;
  subscribe(listener: () => void): () => void;
}

export interface TimerControllerState {
  snapshot: TimerSnapshot | null;
  error: string | null;
  isLoading: boolean;
  isActionPending: boolean;
}

export type TimerActionType = TimerAction["type"];

interface ControllerOptions {
  transport: TimerTransport;
  scheduler: Scheduler;
  visibility: VisibilitySource;
  intervalMs?: number;
}

export interface TimerController {
  start(): void;
  stop(): void;
  subscribe(listener: (state: TimerControllerState) => void): () => void;
  getState(): TimerControllerState;
  act(type: TimerActionType): Promise<void>;
  setRatio(ratio: FocusRatio): Promise<void>;
}

export function createTimerController({
  transport,
  scheduler,
  visibility,
  intervalMs = 250,
}: ControllerOptions): TimerController {
  let state: TimerControllerState = {
    snapshot: null,
    error: null,
    isLoading: true,
    isActionPending: false,
  };
  let started = false;
  let generation = 0;
  let nextRequestId = 0;
  let lastAppliedRequestId = 0;
  let refreshRequestId: number | null = null;
  let intervalHandle: unknown;
  let unsubscribeVisibility: (() => void) | null = null;
  const listeners = new Set<(state: TimerControllerState) => void>();

  function update(next: Partial<TimerControllerState>) {
    state = { ...state, ...next };
    listeners.forEach((listener) => listener(state));
  }

  function applyResponse(
    requestId: number,
    requestGeneration: number,
    snapshot: TimerSnapshot,
  ) {
    if (
      !started ||
      requestGeneration !== generation ||
      requestId < lastAppliedRequestId
    ) {
      return;
    }

    lastAppliedRequestId = requestId;
    update({ snapshot, error: null, isLoading: false });
  }

  function refresh() {
    if (!started || !visibility.isVisible() || refreshRequestId !== null) {
      return;
    }

    const requestId = ++nextRequestId;
    const requestGeneration = generation;
    refreshRequestId = requestId;
    void transport
      .getSnapshot()
      .then((snapshot) => applyResponse(requestId, requestGeneration, snapshot))
      .catch((error: unknown) => {
        if (
          started &&
          requestGeneration === generation &&
          requestId >= lastAppliedRequestId
        ) {
          update({ error: errorMessage(error), isLoading: false });
        }
      })
      .finally(() => {
        if (refreshRequestId === requestId) {
          refreshRequestId = null;
        }
      });
  }

  async function performAction(
    action: { type: TimerActionType } | { type: "setRatio"; ratio: FocusRatio },
  ) {
    if (!started || !state.snapshot || state.isActionPending) {
      throw new Error("timer action is not currently available");
    }

    const requestId = ++nextRequestId;
    const requestGeneration = generation;
    const revisionedAction = {
      ...action,
      expectedRevision: state.snapshot.revision,
    } as TimerAction;
    update({ isActionPending: true, error: null });
    try {
      const snapshot = await transport.applyAction(revisionedAction);
      applyResponse(requestId, requestGeneration, snapshot);
    } catch (error) {
      if (started && requestGeneration === generation) {
        update({ error: errorMessage(error) });
      }
      throw error;
    } finally {
      if (started && requestGeneration === generation) {
        update({ isActionPending: false });
      }
    }
  }

  return {
    start() {
      if (started) return;
      started = true;
      generation += 1;
      unsubscribeVisibility = visibility.subscribe(() => {
        if (visibility.isVisible()) refresh();
      });
      intervalHandle = scheduler.setInterval(refresh, intervalMs);
      refresh();
    },
    stop() {
      if (!started) return;
      started = false;
      generation += 1;
      refreshRequestId = null;
      scheduler.clearInterval(intervalHandle);
      unsubscribeVisibility?.();
      unsubscribeVisibility = null;
    },
    subscribe(listener) {
      listeners.add(listener);
      listener(state);
      return () => listeners.delete(listener);
    },
    getState() {
      return state;
    },
    act(type) {
      return performAction({ type });
    },
    setRatio(ratio) {
      return performAction({ type: "setRatio", ratio });
    },
  };
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
