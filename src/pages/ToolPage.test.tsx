import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS } from "../types";

const listPrompts = vi.fn();
const listTools = vi.fn();
const toolStatus = vi.fn();
const doctor = vi.fn();
const listOperations = vi.fn();
const listActivations = vi.fn();

vi.mock("../api", () => ({
  listPrompts: (...args: unknown[]) => listPrompts(...args),
  listTools: (...args: unknown[]) => listTools(...args),
  toolStatus: (...args: unknown[]) => toolStatus(...args),
  doctor: (...args: unknown[]) => doctor(...args),
  listOperations: (...args: unknown[]) => listOperations(...args),
  listActivations: (...args: unknown[]) => listActivations(...args),
}));

describe("ToolPage prompt list loading", () => {
  it("leaves the loading state after listPrompts resolves, even if doctor is slow", async () => {
    listPrompts.mockResolvedValue({ prompts: [] });
    listTools.mockResolvedValue({ tools: [] });
    toolStatus.mockResolvedValue({
      available: true,
      status: "inactive",
      scopes: [{ id: "user", supported: true, reason: null }],
      doctor: { ok: true },
    });
    doctor.mockImplementation(() => new Promise(() => undefined));
    listOperations.mockResolvedValue({ operations: [] });
    listActivations.mockResolvedValue({ activations: [] });

    const { ToolPage } = await import("./ToolPage");
    render(
      <ToolPage
        tool="claude"
        settings={DEFAULT_SETTINGS}
        toast={{ ok: vi.fn(), err: vi.fn(), info: vi.fn(), toasts: [], dismiss: vi.fn() }}
        onRememberProject={vi.fn()}
      />,
    );

    expect(screen.getByTestId("prompt-list-loading")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByTestId("prompt-list-empty")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("prompt-list-loading")).not.toBeInTheDocument();
  });
});
