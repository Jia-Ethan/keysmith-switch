import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { resetHarnessStatuses } from "../lib/harnessState";

const deployHarness = vi.fn();
const removeHarness = vi.fn();
const getHarnessState = vi.fn();

vi.mock("../api", () => ({
  deployHarness: (...args: unknown[]) => deployHarness(...args),
  removeHarness: (...args: unknown[]) => removeHarness(...args),
  getHarnessState: (...args: unknown[]) => getHarnessState(...args),
}));

describe("HarnessPage", () => {
  beforeEach(() => {
    // The store remembers for a whole app run, which in a test file means it
    // would leak from one case into the next.
    resetHarnessStatuses();
    deployHarness.mockReset();
    removeHarness.mockReset();
    getHarnessState.mockReset();
    getHarnessState.mockResolvedValue({ tool: "claude", deployed: false, error: null });
  });

  it("reads the machine and shows only deploy when nothing is deployed", async () => {
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="codex" />);
    expect(screen.getByRole("heading", { name: "Codex" })).toBeInTheDocument();
    expect(await screen.findByTestId("harness-deploy")).toHaveTextContent("部署");
    expect(screen.getByTestId("harness-status")).toHaveTextContent("未部署");
    expect(screen.queryByTestId("harness-remove")).not.toBeInTheDocument();
    expect(getHarnessState).toHaveBeenCalledWith("codex");
    expect(screen.queryByTestId("prompt-search")).not.toBeInTheDocument();
    expect(screen.queryByTestId("prompt-new")).not.toBeInTheDocument();
    expect(screen.queryByTestId("prompt-sort")).not.toBeInTheDocument();
    expect(screen.queryByTestId("scope-bar")).not.toBeInTheDocument();
  });

  it("shows only remove when the machine is already deployed", async () => {
    getHarnessState.mockResolvedValue({ tool: "zcode", deployed: true, error: null });
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="zcode" />);
    expect(await screen.findByTestId("harness-remove")).toHaveTextContent("卸载");
    expect(screen.getByTestId("harness-status")).toHaveTextContent("已部署");
    expect(screen.queryByTestId("harness-deploy")).not.toBeInTheDocument();
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

    fireEvent.click(await screen.findByTestId("harness-deploy"));
    const confirm = await screen.findByTestId("harness-confirm");
    fireEvent.click(confirm);

    await waitFor(() => {
      expect(screen.getByTestId("harness-deploy")).toHaveAttribute("data-phase", "busy");
    });
    expect(screen.getByTestId("harness-deploy")).toBeDisabled();
    fireEvent.click(screen.getByTestId("harness-deploy"));
    expect(deployHarness).toHaveBeenCalledTimes(1);
    expect(deployHarness).toHaveBeenCalledWith("claude");
    expect(screen.queryByTestId("harness-remove")).not.toBeInTheDocument();

    release({ ok: true, tool: "claude", action: "deploy", promptId: null, error: null });
    await waitFor(() => {
      expect(screen.getByTestId("harness-remove")).toHaveAttribute("data-phase", "success");
    });
    expect(screen.getByTestId("harness-status")).toHaveTextContent("已部署");
    expect(screen.queryByTestId("harness-deploy")).not.toBeInTheDocument();
  });

  it("holds the failure beside the same button and does not reveal the other action", async () => {
    getHarnessState.mockResolvedValue({ tool: "grok", deployed: true, error: null });
    removeHarness.mockResolvedValue({
      ok: false,
      tool: "grok",
      action: "remove",
      promptId: null,
      error: "no audited latest feed",
    });
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="grok" />);

    fireEvent.click(await screen.findByTestId("harness-remove"));
    const confirm = await screen.findByTestId("harness-confirm");
    fireEvent.click(confirm);

    await waitFor(() => {
      expect(screen.getByTestId("harness-remove")).toHaveAttribute("data-phase", "failure");
    });
    expect(screen.getByTestId("harness-remove")).toHaveTextContent("重试");
    expect(screen.getByTestId("harness-status")).toHaveTextContent("no audited latest feed");
    expect(screen.queryByTestId("harness-deploy")).not.toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("returns to deploy after a successful remove", async () => {
    getHarnessState.mockResolvedValue({ tool: "claude", deployed: true, error: null });
    removeHarness.mockResolvedValue({
      ok: true,
      tool: "claude",
      action: "remove",
      promptId: null,
      error: null,
    });
    const { HarnessPage } = await import("./HarnessPage");
    render(<HarnessPage tool="claude" />);
    fireEvent.click(await screen.findByTestId("harness-remove"));
    fireEvent.click(await screen.findByTestId("harness-confirm"));
    await waitFor(() => {
      expect(screen.getByTestId("harness-deploy")).toHaveTextContent("部署");
    });
    expect(screen.getByTestId("harness-status")).toHaveTextContent("未部署");
    expect(screen.queryByTestId("harness-remove")).not.toBeInTheDocument();
  });
});
