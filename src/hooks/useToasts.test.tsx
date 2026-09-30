import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TOAST_DURATION_MS, useToasts } from "./useToasts";

afterEach(() => {
  vi.useRealTimers();
});

describe("useToasts", () => {
  it("replaces an identical message instead of stacking it", () => {
    const { result } = renderHook(() => useToasts());
    act(() => {
      result.current.ok("Saved");
      result.current.ok("Saved");
      result.current.ok("Saved");
    });
    expect(result.current.toasts).toHaveLength(1);

    act(() => {
      result.current.err("Saved");
      result.current.ok("Other");
    });
    expect(result.current.toasts.map((item) => `${item.kind}:${item.message}`)).toEqual(["ok:Saved", "err:Saved", "ok:Other"]);
  });

  it("dismisses a toast on its own after the display time", () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useToasts());
    act(() => result.current.info("Heads up"));
    expect(result.current.toasts).toHaveLength(1);
    act(() => {
      vi.advanceTimersByTime(TOAST_DURATION_MS + 1);
    });
    expect(result.current.toasts).toHaveLength(0);
  });
});
