import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const deployHarness = vi.fn();
const removeHarness = vi.fn();

vi.mock("../api", () => ({
  deployHarness: (...args: unknown[]) => deployHarness(...args),
  removeHarness: (...args: unknown[]) => removeHarness(...args),
}));

describe("HarnessPage", () => {
  beforeEach(() => {
    deployHarness.mockReset();
    removeHarness.mockReset();
  });

  it("shows only deploy and remove for the selected tool", async () => {
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="codex" />);
    expect(screen.getByRole("heading", { name: "Codex" })).toBeInTheDocument();
    expect(screen.getByTestId("harness-deploy")).toHaveTextContent("部署");
    expect(screen.getByTestId("harness-remove")).toHaveTextContent("卸载");
    expect(screen.queryByTestId("prompt-search")).not.toBeInTheDocument();
    expect(screen.queryByTestId("prompt-new")).not.toBeInTheDocument();
    expect(screen.queryByTestId("prompt-sort")).not.toBeInTheDocument();
    expect(screen.queryByTestId("scope-bar")).not.toBeInTheDocument();
  });

  it("confirms once, shows busy on the control, then success, and ignores a second click", async () => {
    let release: (value: { ok: boolean; tool: string; action: string; promptId: null; error: null }) => void =
      () => undefined;
    deployHarness.mockImplementation(
      () =>
        new Promise((resolve) => {
          release = resolve;
        }),
    );
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="claude" />);

    fireEvent.click(screen.getByTestId("harness-deploy"));
    const confirm = await screen.findByTestId("harness-confirm");
    fireEvent.click(confirm);

    await waitFor(() => {
      expect(screen.getByTestId("harness-deploy")).toHaveAttribute("data-phase", "busy");
    });
    expect(screen.getByTestId("harness-deploy")).toBeDisabled();
    fireEvent.click(screen.getByTestId("harness-deploy"));
    expect(deployHarness).toHaveBeenCalledTimes(1);
    expect(deployHarness).toHaveBeenCalledWith("claude");

    release({ ok: true, tool: "claude", action: "deploy", promptId: null, error: null });
    await waitFor(() => {
      expect(screen.getByTestId("harness-deploy")).toHaveAttribute("data-phase", "success");
    });
    expect(screen.getByTestId("harness-deploy")).toHaveTextContent("完成");
  });

  it("holds the failure on the control with the reason beside it", async () => {
    removeHarness.mockResolvedValue({
      ok: false,
      tool: "grok",
      action: "remove",
      promptId: null,
      error: "no audited latest feed",
    });
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="grok" />);

    fireEvent.click(screen.getByTestId("harness-remove"));
    const confirm = await screen.findByTestId("harness-confirm");
    fireEvent.click(confirm);

    await waitFor(() => {
      expect(screen.getByTestId("harness-remove")).toHaveAttribute("data-phase", "failure");
    });
    expect(screen.getByTestId("harness-remove")).toHaveTextContent("重试");
    expect(screen.getByTestId("harness-remove-reason")).toHaveTextContent("no audited latest feed");
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });
});
