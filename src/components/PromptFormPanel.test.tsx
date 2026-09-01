import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PromptFormPanel } from "./PromptFormPanel";

describe("PromptFormPanel dirty guard", () => {
  it("checks an unsaved draft only once when Escape closes the panel", () => {
    const dirtyGuard = vi.fn(() => true);
    const onClose = vi.fn();
    render(
      <PromptFormPanel
        title="Edit"
        draft={{ title: "Prompt", content: "Body", tags: "" }}
        saving={false}
        onChange={vi.fn()}
        onSave={vi.fn()}
        onClose={onClose}
        dirtyGuard={dirtyGuard}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "返回" }));
    expect(dirtyGuard).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("fullscreen-panel")).toBeInTheDocument();
  });
});
