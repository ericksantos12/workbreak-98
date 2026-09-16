import { describe, expect, it, vi } from "vitest";
import {
  createTimerController,
  type Scheduler,
  type TimerSnapshot,
  type TimerTransport,
  type VisibilitySource,
} from "./flowmodoro";

function snapshot(revision: number, focusElapsedMs = 0): TimerSnapshot {
  return {
    revision,
    phase: "focusRunning",
    focusElapsedMs,
    ratio: 3,
    breakEarnedMs: Math.floor(focusElapsedMs / 3),
    breakRemainingMs: 0,
    breakElapsedMs: 0,
    persistenceStatus: "available",
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function harness(transport: TimerTransport) {
  let interval: (() => void) | null = null;
  let visible = true;
  let visibilityListener: (() => void) | null = null;
  const scheduler: Scheduler = {
    setInterval(callback) {
      interval = callback;
      return 1;
    },
    clearInterval() {
      interval = null;
    },
  };
  const visibility: VisibilitySource = {
    isVisible: () => visible,
    subscribe(listener) {
      visibilityListener = listener;
      return () => {
        visibilityListener = null;
      };
    },
  };

  return {
    controller: createTimerController({ transport, scheduler, visibility }),
    tick: () => interval?.(),
    setVisible(value: boolean) {
      visible = value;
      visibilityListener?.();
    },
  };
}

describe("timer controller", () => {
  it("keeps at most one visual refresh in flight", async () => {
    const request = deferred<TimerSnapshot>();
    const transport: TimerTransport = {
      getSnapshot: vi.fn(() => request.promise),
      applyAction: vi.fn(),
    };
    const { controller, tick } = harness(transport);

    controller.start();
    tick();
    tick();
    expect(transport.getSnapshot).toHaveBeenCalledTimes(1);

    request.resolve(snapshot(0));
    await request.promise;
    await new Promise((resolve) => setTimeout(resolve, 0));
    tick();
    expect(transport.getSnapshot).toHaveBeenCalledTimes(2);
    controller.stop();
  });

  it("sends ratio changes with the current revision", async () => {
    const transport: TimerTransport = {
      getSnapshot: vi.fn().mockResolvedValue(snapshot(4, 60_000)),
      applyAction: vi.fn().mockResolvedValue({
        ...snapshot(5, 60_000),
        ratio: 5,
        breakEarnedMs: 12_000,
      }),
    };
    const { controller } = harness(transport);
    controller.start();
    await new Promise((resolve) => setTimeout(resolve, 0));

    await controller.setRatio(5);

    expect(transport.applyAction).toHaveBeenCalledWith({
      type: "setRatio",
      expectedRevision: 4,
      ratio: 5,
    });
    expect(controller.getState().snapshot?.breakEarnedMs).toBe(12_000);
    controller.stop();
  });

  it("ignores a refresh response older than an action response", async () => {
    const slowRefresh = deferred<TimerSnapshot>();
    const action = deferred<TimerSnapshot>();
    const getSnapshot = vi
      .fn<() => Promise<TimerSnapshot>>()
      .mockResolvedValueOnce(snapshot(1))
      .mockReturnValueOnce(slowRefresh.promise);
    const transport: TimerTransport = {
      getSnapshot,
      applyAction: vi.fn(() => action.promise),
    };
    const { controller, tick } = harness(transport);
    controller.start();
    await Promise.resolve();
    await Promise.resolve();
    tick();

    const actionPromise = controller.act("pauseFocus");
    action.resolve({
      revision: 2,
      phase: "focusPaused",
      focusElapsedMs: 10,
      ratio: 3,
      breakEarnedMs: 3,
      breakRemainingMs: 0,
      breakElapsedMs: 0,
      persistenceStatus: "available",
    });
    await actionPromise;
    slowRefresh.resolve(snapshot(1, 5));
    await slowRefresh.promise;
    await Promise.resolve();

    expect(controller.getState().snapshot?.revision).toBe(2);
    expect(transport.applyAction).toHaveBeenCalledWith({
      type: "pauseFocus",
      expectedRevision: 1,
    });
    controller.stop();
  });

  it("refreshes immediately when becoming visible and stops cleanly", async () => {
    const transport: TimerTransport = {
      getSnapshot: vi.fn().mockResolvedValue(snapshot(0)),
      applyAction: vi.fn(),
    };
    const { controller, setVisible, tick } = harness(transport);
    setVisible(false);
    controller.start();
    expect(transport.getSnapshot).not.toHaveBeenCalled();

    setVisible(true);
    expect(transport.getSnapshot).toHaveBeenCalledTimes(1);
    await Promise.resolve();
    controller.stop();
    tick();
    expect(transport.getSnapshot).toHaveBeenCalledTimes(1);
  });
});
