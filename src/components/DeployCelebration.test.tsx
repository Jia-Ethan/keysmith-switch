import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CELEBRATION_MS, DeployCelebration } from "./DeployCelebration";

afterEach(() => {
  vi.useRealTimers();
});

describe("DeployCelebration", () => {
  it("dismisses once on schedule even when the parent re-renders with a new callback", () => {
    vi.useFakeTimers();
    const first = vi.fn();
    const second = vi.fn();
    const { rerender } = render(<DeployCelebration tool="claude" title="Deployed" subtitle="Rules" onDone={first} />);
    expect(screen.getByTestId("deploy-celebration")).toHaveTextContent("Deployed");
    act(() => {
      vi.advanceTimersByTime(CELEBRATION_MS - 200);
    });
    rerender(<DeployCelebration tool="claude" title="Deployed" subtitle="Rules" onDone={second} />);
    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledTimes(1);
  });
});
