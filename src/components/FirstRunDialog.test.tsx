import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FirstRunDialog } from "./FirstRunDialog";

describe("FirstRunDialog", () => {
  it("shows environment status and completes with no import candidates", () => {
    const onSkip = vi.fn();
    render(
      <FirstRunDialog
        open
        candidates={[]}
        sidecar={{
          pythonRequired: false,
          tools: [
            { tool: "claude", frozen: true, path: "/app/claude", available: true },
            { tool: "zcode", frozen: true, path: "/app/zcode", available: false },
          ],
        }}
        onImport={vi.fn()}
        onSkip={onSkip}
      />,
    );

    expect(screen.getByText("首次启动检查")).toBeInTheDocument();
    expect(screen.getByText("Claude Code")).toBeInTheDocument();
    expect(screen.getByText("ZCode")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "完成设置" }));
    expect(onSkip).toHaveBeenCalledTimes(1);
  });
});
