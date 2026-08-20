import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ConfirmDialog } from "./ConfirmDialog";

describe("ConfirmDialog focus management", () => {
  it("traps tab focus and restores the opener", () => {
    const onClose = vi.fn();
    const closed = <button type="button">opener</button>;
    const open = (
      <>
        {closed}
        <ConfirmDialog
          open
          title="Confirm"
          confirmLabel="Confirm"
          cancelLabel="Cancel"
          onConfirm={vi.fn()}
          onClose={onClose}
        >
          <input aria-label="value" />
        </ConfirmDialog>
      </>
    );
    const { rerender } = render(closed);
    screen.getByRole("button", { name: "opener" }).focus();
    rerender(open);

    const first = screen.getByRole("button", { name: "Close" });
    const last = screen.getByRole("button", { name: "Confirm" });
    expect(first).toHaveFocus();
    last.focus();
    fireEvent.keyDown(document, { key: "Tab" });
    expect(first).toHaveFocus();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);

    rerender(closed);
    expect(screen.getByRole("button", { name: "opener" })).toHaveFocus();
  });

  it("locks close actions while busy", () => {
    const onClose = vi.fn();
    render(
      <ConfirmDialog
        open
        busy
        title="Working"
        confirmLabel="Working"
        cancelLabel="Cancel"
        onConfirm={vi.fn()}
        onClose={onClose}
      >
        <p>Body</p>
      </ConfirmDialog>,
    );

    expect(screen.getByRole("dialog")).toHaveAttribute("aria-busy", "true");
    expect(screen.getByRole("button", { name: "Close" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
  });
});
