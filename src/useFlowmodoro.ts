import { useEffect, useRef, useState } from "react";
import {
  applyTimerAction,
  createTimerController,
  getTimerSnapshot,
  type Scheduler,
  type FocusRatio,
  type TimerActionType,
  type TimerController,
  type TimerControllerState,
  type TimerTransport,
  type VisibilitySource,
} from "./flowmodoro";

const initialState: TimerControllerState = {
  snapshot: null,
  error: null,
  isLoading: true,
  isActionPending: false,
};

const transport: TimerTransport = {
  getSnapshot: getTimerSnapshot,
  applyAction: applyTimerAction,
};

const scheduler: Scheduler = {
  setInterval: (callback, milliseconds) =>
    window.setInterval(callback, milliseconds),
  clearInterval: (handle) => window.clearInterval(handle as number),
};

const visibility: VisibilitySource = {
  isVisible: () => document.visibilityState === "visible",
  subscribe(listener) {
    document.addEventListener("visibilitychange", listener);
    return () => document.removeEventListener("visibilitychange", listener);
  },
};

export function useFlowmodoro() {
  const [state, setState] = useState<TimerControllerState>(initialState);
  const controllerRef = useRef<TimerController | null>(null);

  useEffect(() => {
    const controller = createTimerController({
      transport,
      scheduler,
      visibility,
    });
    controllerRef.current = controller;
    const unsubscribe = controller.subscribe(setState);
    controller.start();

    return () => {
      controller.stop();
      unsubscribe();
      if (controllerRef.current === controller) {
        controllerRef.current = null;
      }
    };
  }, []);

  function act(type: TimerActionType) {
    const controller = controllerRef.current;
    if (!controller) {
      return Promise.reject(new Error("timer controller is not ready"));
    }
    return controller.act(type);
  }

  function setRatio(ratio: FocusRatio) {
    const controller = controllerRef.current;
    if (!controller) {
      return Promise.reject(new Error("timer controller is not ready"));
    }
    return controller.setRatio(ratio);
  }

  return { ...state, act, setRatio };
}
