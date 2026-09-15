import { describe, expect, it } from "vitest";
import { formatElapsedTime, formatRemainingTime } from "./formatTime";

describe("duration formatting", () => {
  it("truncates elapsed time to complete seconds", () => {
    expect(formatElapsedTime(1_999)).toBe("00:00:01");
  });

  it("rounds positive remaining time up and keeps zero at zero", () => {
    expect(formatRemainingTime(1)).toBe("00:00:01");
    expect(formatRemainingTime(0)).toBe("00:00:00");
  });

  it("does not wrap after 24 hours", () => {
    expect(formatElapsedTime(25 * 60 * 60 * 1_000)).toBe("25:00:00");
  });
});
