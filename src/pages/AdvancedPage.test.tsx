import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS } from "../types";
import { AdvancedPage } from "./AdvancedPage";

const listAdvancedTools = vi.fn();
const listRuntimes = vi.fn();
const runAdvanced = vi.fn();

vi.mock("../api", () => ({
  listAdvancedTools: (...args: unknown[]) => listAdvancedTools(...args),
  listRuntimes: (...args: unknown[]) => listRuntimes(...args),
  runAdvanced: (...args: unknown[]) => runAdvanced(...args),
}));

const target = {
  kind: "wsl",
  distribution: "Ubuntu-24.04",
  user: "developer",
} as const;

const toast = {
  ok: vi.fn(),
  err: vi.fn(),
  info: vi.fn(),
  toasts: [],
  dismiss: vi.fn(),
};

describe("AdvancedPage runtime target", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listAdvancedTools.mockResolvedValue({
      enabled: true,
      tools: [
        { kind: "scenario", name: "Scenario evaluation", description: "" },
        { kind: "grokRun", name: "Grok Run", description: "" },
        { kind: "grokBreaktest", name: "Grok Breaktest", description: "" },
      ],
    });
    listRuntimes.mockResolvedValue({
      runtimes: [
        {
          target: { kind: "native" },
          label: "Windows local",
          isDefault: true,
          state: "running",
          wslVersion: null,
          available: true,
          unavailableReason: null,
          home: "C:\\Users\\test",
        },
        {
          target,
          label: "WSL2 · Ubuntu-24.04 · developer",
          isDefault: false,
          state: "running",
          wslVersion: 2,
          available: true,
          unavailableReason: null,
          home: "/home/developer",
        },
      ],
    });
    runAdvanced.mockResolvedValue({
      ok: true,
      kind: "scenario",
      output: "done",
      error: null,
    });
  });

  it("runs the Codex advanced tool in its selected WSL distribution and user", async () => {
    render(
      <AdvancedPage
        enabled
        toast={toast as never}
        settings={{
          ...DEFAULT_SETTINGS,
          agentRuntimeTargets: { codex: target },
        }}
      />,
    );

    expect(await screen.findByTestId("advanced-runtime")).toHaveTextContent(
      "Ubuntu-24.04 · developer",
    );
    fireEvent.change(screen.getByTestId("advanced-args"), {
      target: { value: "fixture" },
    });
    fireEvent.click(screen.getByTestId("advanced-run"));

    await waitFor(() =>
      expect(runAdvanced).toHaveBeenCalledWith(
        "scenario",
        { input: "fixture" },
        target,
      ),
    );
  });
});
