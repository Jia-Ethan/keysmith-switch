import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PromptViewPage } from "./PromptViewPage";
import type { ToastApi } from "../hooks/useToasts";
import type { PromptDetail } from "../types";

const getPrompt = vi.fn();
const promptHistory = vi.fn();
const createPrompt = vi.fn();
const copyPrompt = vi.fn();
const deletePrompt = vi.fn();
const restorePromptVersion = vi.fn();
const promptDiff = vi.fn();

vi.mock("../api", () => ({
  getPrompt: (...args: unknown[]) => getPrompt(...args),
  promptHistory: (...args: unknown[]) => promptHistory(...args),
  createPrompt: (...args: unknown[]) => createPrompt(...args),
  copyPrompt: (...args: unknown[]) => copyPrompt(...args),
  deletePrompt: (...args: unknown[]) => deletePrompt(...args),
  restorePromptVersion: (...args: unknown[]) => restorePromptVersion(...args),
  promptDiff: (...args: unknown[]) => promptDiff(...args),
}));

const mockToast: ToastApi = {
  toasts: [],
  dismiss: vi.fn(),
  info: vi.fn(),
  ok: vi.fn(),
  err: vi.fn(),
};

const mockDetail: PromptDetail = {
  id: "prompt-1",
  tool: "claude",
  title: "Test Prompt",
  content: "Test content",
  tags: ["test", "demo"],
  createdAt: "2026-08-19T00:00:00Z",
  updatedAt: "2026-08-19T00:00:00Z",
  active: false,
  lastUsedAt: null,
  excerpt: "Test content",
};

describe("PromptViewPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getPrompt.mockResolvedValue(mockDetail);
    promptHistory.mockResolvedValue({ versions: [] });
  });

  it("loads and displays prompt details", async () => {
    getPrompt.mockResolvedValue(mockDetail);
    promptHistory.mockResolvedValue({ versions: [] });

    render(
      <PromptViewPage
        promptId="prompt-1"
        tool="claude"
        isActiveHere={false}
        disabled={false}
        busy={false}
        toast={mockToast}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onActivate={vi.fn()}
        onDeactivate={vi.fn()}
        onDeleted={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByRole("dialog", { name: "Test Prompt" })).toBeInTheDocument();
    });
    expect(screen.getByDisplayValue("Test content")).toBeInTheDocument();
    expect(screen.getByText("test, demo")).toBeInTheDocument();
  });

  it("shows activate button when prompt is inactive", async () => {
    getPrompt.mockResolvedValue(mockDetail);
    promptHistory.mockResolvedValue({ versions: [] });

    render(
      <PromptViewPage
        promptId="prompt-1"
        tool="claude"
        isActiveHere={false}
        disabled={false}
        busy={false}
        toast={mockToast}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onActivate={vi.fn()}
        onDeactivate={vi.fn()}
        onDeleted={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("prompt-activate")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("prompt-deactivate")).not.toBeInTheDocument();
  });

  it("shows deactivate button when prompt is active", async () => {
    getPrompt.mockResolvedValue(mockDetail);
    promptHistory.mockResolvedValue({ versions: [] });

    render(
      <PromptViewPage
        promptId="prompt-1"
        tool="claude"
        isActiveHere={true}
        disabled={false}
        busy={false}
        toast={mockToast}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onActivate={vi.fn()}
        onDeactivate={vi.fn()}
        onDeleted={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("prompt-deactivate")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("prompt-activate")).not.toBeInTheDocument();
    expect(screen.getByTestId("prompt-active-here")).toBeInTheDocument();
  });

  it("shows activation unknown badge when state is null", async () => {
    getPrompt.mockResolvedValue(mockDetail);
    promptHistory.mockResolvedValue({ versions: [] });

    render(
      <PromptViewPage
        promptId="prompt-1"
        tool="claude"
        isActiveHere={null}
        disabled={false}
        busy={false}
        toast={mockToast}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onActivate={vi.fn()}
        onDeactivate={vi.fn()}
        onDeleted={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("prompt-activation-unknown")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("prompt-activate")).not.toBeInTheDocument();
    expect(screen.queryByTestId("prompt-deactivate")).not.toBeInTheDocument();
  });

  it("keeps the delete dialog open and unlocks it after deletion fails", async () => {
    deletePrompt.mockRejectedValue(new Error("delete failed"));
    const onDeleted = vi.fn();

    render(
      <PromptViewPage
        promptId="prompt-1"
        tool="claude"
        isActiveHere={false}
        disabled={false}
        busy={false}
        toast={mockToast}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onActivate={vi.fn()}
        onDeactivate={vi.fn()}
        onDeleted={onDeleted}
      />,
    );

    fireEvent.click(await screen.findByTestId("prompt-menu"));
    fireEvent.click(screen.getByTestId("prompt-delete"));
    const confirm = screen.getByTestId("prompt-delete-confirm");
    fireEvent.click(confirm);

    await waitFor(() => expect(mockToast.err).toHaveBeenCalled());
    expect(screen.getByRole("dialog", { name: "删除" })).toBeInTheDocument();
    expect(confirm).not.toBeDisabled();
    expect(onDeleted).not.toHaveBeenCalled();
  });

  it("prevents duplicate destructive actions while one is pending", async () => {
    let resolveDelete!: (value: { ok: boolean }) => void;
    deletePrompt.mockReturnValue(new Promise((resolve) => { resolveDelete = resolve; }));

    render(
      <PromptViewPage
        promptId="prompt-1"
        tool="claude"
        isActiveHere={false}
        disabled={false}
        busy={false}
        toast={mockToast}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onActivate={vi.fn()}
        onDeactivate={vi.fn()}
        onDeleted={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByTestId("prompt-menu"));
    fireEvent.click(screen.getByTestId("prompt-delete"));
    const confirm = screen.getByTestId("prompt-delete-confirm");
    fireEvent.click(confirm);
    fireEvent.click(confirm);

    expect(deletePrompt).toHaveBeenCalledTimes(1);
    expect(confirm).toBeDisabled();
    resolveDelete({ ok: true });
    await waitFor(() => expect(mockToast.ok).toHaveBeenCalled());
  });

  it("reports copy failures without closing the prompt", async () => {
    copyPrompt.mockRejectedValue(new Error("copy failed"));
    const onClose = vi.fn();

    render(
      <PromptViewPage
        promptId="prompt-1"
        tool="claude"
        isActiveHere={false}
        disabled={false}
        busy={false}
        toast={mockToast}
        onClose={onClose}
        onEdit={vi.fn()}
        onActivate={vi.fn()}
        onDeactivate={vi.fn()}
        onDeleted={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByTestId("prompt-menu"));
    fireEvent.click(screen.getByTestId("prompt-copy-codex"));

    await waitFor(() => expect(mockToast.err).toHaveBeenCalled());
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "Test Prompt" })).toBeInTheDocument();
  });

  it("keeps the current detail and history after restore refresh fails", async () => {
    promptHistory
      .mockResolvedValueOnce({
        versions: [
          { version: 2, createdAt: "2026-08-20T00:00:00Z", title: "Test Prompt", summary: null },
          { version: 1, createdAt: "2026-08-19T00:00:00Z", title: "Old Prompt", summary: null },
        ],
      })
      .mockRejectedValueOnce(new Error("history failed"));
    restorePromptVersion.mockResolvedValue({ ...mockDetail, title: "Restored Prompt", content: "Restored body" });

    render(
      <PromptViewPage
        promptId="prompt-1"
        tool="claude"
        isActiveHere={false}
        disabled={false}
        busy={false}
        toast={mockToast}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onActivate={vi.fn()}
        onDeactivate={vi.fn()}
        onDeleted={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "版本历史 (2)" }));
    const restoreButtons = screen.getAllByRole("button", { name: "恢复此版本" });
    fireEvent.click(restoreButtons[0]!);

    expect(await screen.findByRole("dialog", { name: "Restored Prompt" })).toBeInTheDocument();
    expect(screen.getByDisplayValue("Restored body")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("无法加载版本历史");
    expect(screen.getByRole("button", { name: "版本历史 (2)" })).toBeInTheDocument();
  });

  it("preserves the last diff when a later diff request fails", async () => {
    promptHistory.mockResolvedValue({
      versions: [
        { version: 3, createdAt: "2026-08-21T00:00:00Z", title: "V3", summary: null },
        { version: 2, createdAt: "2026-08-20T00:00:00Z", title: "V2", summary: null },
        { version: 1, createdAt: "2026-08-19T00:00:00Z", title: "V1", summary: null },
      ],
    });
    promptDiff
      .mockResolvedValueOnce({ unified: "known diff", summary: "" })
      .mockRejectedValueOnce(new Error("diff failed"));

    render(
      <PromptViewPage
        promptId="prompt-1"
        tool="claude"
        isActiveHere={false}
        disabled={false}
        busy={false}
        toast={mockToast}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onActivate={vi.fn()}
        onDeactivate={vi.fn()}
        onDeleted={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "版本历史 (3)" }));
    const diffButtons = screen.getAllByRole("button", { name: "差异" });
    fireEvent.click(diffButtons[0]!);
    expect(await screen.findByText("known diff")).toBeInTheDocument();
    fireEvent.click(diffButtons[1]!);

    await waitFor(() => expect(mockToast.err).toHaveBeenCalled());
    expect(screen.getByText("known diff")).toBeInTheDocument();
  });
});
