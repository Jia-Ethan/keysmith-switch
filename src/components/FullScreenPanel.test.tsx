import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FullScreenPanel } from "./FullScreenPanel";

describe("FullScreenPanel focus management", () => {
  it("traps focus and restores the opener", () => {
    const onClose = vi.fn();
    const closed = <button type="button">opener</button>;
    const { rerender } = render(closed);
    screen.getByRole("button", { name: "opener" }).focus();
    rerender(
      <>
        {closed}
        <FullScreenPanel isOpen title="Editor" onClose={onClose} footer={<button type="button">Save</button>}>
          <input aria-label="content" />
        </FullScreenPanel>
      </>,
    );

    const first = screen.getByTestId("fullscreen-back");
    const last = screen.getByRole("button", { name: "Save" });
    expect(first).toHaveFocus();
    last.focus();
    fireEvent.keyDown(window, { key: "Tab" });
    expect(first).toHaveFocus();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);

    rerender(closed);
    expect(screen.getByRole("button", { name: "opener" })).toHaveFocus();
  });
});
