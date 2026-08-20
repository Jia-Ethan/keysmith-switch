import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS } from "../types";

const listPrompts = vi.fn();
const listTools = vi.fn();
const toolStatus = vi.fn();
const doctor = vi.fn();
const listOperations = vi.fn();
const listActivations = vi.fn();
const getPrompt = vi.fn();
const promptHistory = vi.fn();

vi.mock("../api", () => ({
  listPrompts: (...args: unknown[]) => listPrompts(...args),
  listTools: (...args: unknown[]) => listTools(...args),
  toolStatus: (...args: unknown[]) => toolStatus(...args),
  doctor: (...args: unknown[]) => doctor(...args),
  listOperations: (...args: unknown[]) => listOperations(...args),
  listActivations: (...args: unknown[]) => listActivations(...args),
  getPrompt: (...args: unknown[]) => getPrompt(...args),
  promptHistory: (...args: unknown[]) => promptHistory(...args),
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

  it("does not expose activate or deactivate when scoped activation state is unreadable", async () => {
    listPrompts.mockResolvedValue({
      prompts: [
        {
          id: "p1",
          tool: "claude",
          title: "Alpha",
          tags: [],
          active: false,
          lastUsedAt: null,
          updatedAt: "2026-08-21T00:00:00Z",
          createdAt: "2026-08-21T00:00:00Z",
          excerpt: null,
        },
      ],
    });
    listTools.mockResolvedValue({ tools: [] });
    toolStatus.mockResolvedValue({
      available: true,
      status: "inactive",
      scopes: [{ id: "user", supported: true, reason: null }],
      doctor: { ok: true },
    });
    doctor.mockResolvedValue({ doctor: { ok: true } });
    listOperations.mockResolvedValue({ operations: [] });
    listActivations.mockRejectedValue(new Error("activation table unavailable"));
    getPrompt.mockResolvedValue({
      id: "p1",
      tool: "claude",
      title: "Alpha",
      content: "Body",
      tags: [],
      active: false,
      lastUsedAt: null,
      updatedAt: "2026-08-21T00:00:00Z",
      createdAt: "2026-08-21T00:00:00Z",
      excerpt: null,
    });
    promptHistory.mockResolvedValue({ versions: [] });

    const { ToolPage } = await import("./ToolPage");
    render(
      <ToolPage
        tool="claude"
        settings={DEFAULT_SETTINGS}
        toast={{ ok: vi.fn(), err: vi.fn(), info: vi.fn(), toasts: [], dismiss: vi.fn() }}
        onRememberProject={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByTestId("prompt-item-p1"));
    expect(await screen.findByTestId("prompt-activation-unknown")).toBeInTheDocument();
    expect(screen.queryByTestId("prompt-activate")).not.toBeInTheDocument();
    expect(screen.queryByTestId("prompt-deactivate")).not.toBeInTheDocument();
  });

  it("clears selection when prompt detail loading fails", async () => {
    listPrompts.mockResolvedValue({
      prompts: [
        {
          id: "p2",
          tool: "claude",
          title: "Broken",
          tags: [],
          active: false,
          lastUsedAt: null,
          updatedAt: "2026-08-21T00:00:00Z",
          createdAt: "2026-08-21T00:00:00Z",
          excerpt: null,
        },
      ],
    });
    listTools.mockResolvedValue({ tools: [] });
    toolStatus.mockResolvedValue({
      available: true,
      status: "inactive",
      scopes: [{ id: "user", supported: true, reason: null }],
      doctor: { ok: true },
    });
    doctor.mockResolvedValue({ doctor: { ok: true } });
    listOperations.mockResolvedValue({ operations: [] });
    listActivations.mockResolvedValue({ activations: [] });
    getPrompt.mockRejectedValue(new Error("read failed"));

    const toast = { ok: vi.fn(), err: vi.fn(), info: vi.fn(), toasts: [], dismiss: vi.fn() };
    const { ToolPage } = await import("./ToolPage");
    render(
      <ToolPage
        tool="claude"
        settings={DEFAULT_SETTINGS}
        toast={toast}
        onRememberProject={vi.fn()}
      />,
    );

    const item = await screen.findByTestId("prompt-item-p2");
    fireEvent.click(item);
    await waitFor(() => expect(toast.err).toHaveBeenCalled());
    expect(item).not.toHaveAttribute("aria-current");
    expect(screen.getByTestId("prompt-no-selection")).toBeInTheDocument();
  });
});
