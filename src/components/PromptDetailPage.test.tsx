import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ToastApi } from "../hooks/useToasts";
import type { Envelope, PromptDetail } from "../types";
import { PromptDetailPage } from "./PromptDetailPage";

const listActivations = vi.fn();
const toolStatus = vi.fn();
const getPrompt = vi.fn();
const promptHistory = vi.fn();
const planActivate = vi.fn();
const activate = vi.fn();
const planDeactivate = vi.fn();
const deactivate = vi.fn();

vi.mock("../api", () => ({
  listActivations: (...args: unknown[]) => listActivations(...args),
  toolStatus: (...args: unknown[]) => toolStatus(...args),
  getPrompt: (...args: unknown[]) => getPrompt(...args),
  promptHistory: (...args: unknown[]) => promptHistory(...args),
  planActivate: (...args: unknown[]) => planActivate(...args),
  activate: (...args: unknown[]) => activate(...args),
  planDeactivate: (...args: unknown[]) => planDeactivate(...args),
  deactivate: (...args: unknown[]) => deactivate(...args),
  createPrompt: vi.fn(),
  copyPrompt: vi.fn(),
  deletePrompt: vi.fn(),
  restorePromptVersion: vi.fn(),
  promptDiff: vi.fn(),
}));

const toast: ToastApi = {
  toasts: [],
  dismiss: vi.fn(),
  info: vi.fn(),
  ok: vi.fn(),
  err: vi.fn(),
};

const detail: PromptDetail = {
  id: "prompt-1",
  tool: "claude",
  title: "Test Prompt",
  content: "Body",
  tags: [],
  active: false,
  lastUsedAt: null,
  updatedAt: "2026-08-23T00:00:00Z",
  createdAt: "2026-08-23T00:00:00Z",
  excerpt: "Body",
};

const envelope = {
  schema: "keysmith-switch/adapter-v1",
  tool: "claude",
  command: "activate",
  ok: true,
  preview: true,
  available: true,
  unavailableReason: null,
  adapterVersion: "7.1",
  cliPath: "/tmp/keysmith-claude",
  argv: ["activate"],
  exitCode: 0,
  status: "inactive",
  recoveryRequired: false,
  scopes: [],
  targetPaths: [],
  plannedFiles: [],
  backups: [],
  conflicts: [],
  warnings: [],
  blockers: [],
  currentFingerprint: null,
  targetFingerprint: null,
  doctor: { ok: true, checks: [] },
  reloadRequired: false,
  reloadHint: null,
  error: null,
  redactedStderr: "",
} satisfies Envelope;

describe("PromptDetailPage activation gate", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listActivations.mockResolvedValue({ activations: [] });
    toolStatus.mockResolvedValue(envelope);
    getPrompt.mockResolvedValue(detail);
    promptHistory.mockResolvedValue({ versions: [] });
  });

  it("keeps activation disabled until status and activation context both load", async () => {
    toolStatus.mockRejectedValue(new Error("status unavailable"));
    render(
      <PromptDetailPage
        promptId="prompt-1"
        tool="claude"
        scope="user"
        projectDir=""
        toast={toast}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onChanged={vi.fn()}
      />,
    );

    expect(await screen.findByTestId("prompt-activate")).toBeDisabled();
    expect(screen.getByRole("alert")).toHaveTextContent("加载失败");
  });

  it("requires plan confirmation before executing activation", async () => {
    const onChanged = vi.fn();
    planActivate.mockResolvedValue({ operationId: "op-1", envelope });
    activate.mockResolvedValue({ operationId: "op-1", envelope: { ...envelope, preview: false, status: "active" } });
    render(
      <PromptDetailPage
        promptId="prompt-1"
        tool="claude"
        scope="user"
        projectDir=""
        toast={toast}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onChanged={onChanged}
      />,
    );

    fireEvent.click(await screen.findByTestId("prompt-activate"));
    expect(await screen.findByTestId("plan-preview")).toBeInTheDocument();
    expect(activate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("plan-confirm"));
    await waitFor(() => expect(activate).toHaveBeenCalledWith("op-1"));
    expect(onChanged).toHaveBeenCalled();
  });

  it("requires the same confirmation gate before deactivation", async () => {
    listActivations.mockResolvedValue({
      activations: [{
        id: "activation-1",
        tool: "claude",
        promptId: "prompt-1",
        promptTitle: "Test Prompt",
        scope: "user",
        projectDir: null,
        active: true,
        createdAt: "2026-08-23T00:00:00Z",
        fingerprint: null,
      }],
    });
    planDeactivate.mockResolvedValue({ operationId: "op-2", envelope: { ...envelope, command: "deactivate" } });
    deactivate.mockResolvedValue({
      operationId: "op-2",
      envelope: { ...envelope, command: "deactivate", preview: false, status: "inactive" },
    });

    render(
      <PromptDetailPage
        promptId="prompt-1"
        tool="claude"
        scope="user"
        projectDir=""
        toast={toast}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onChanged={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByTestId("prompt-deactivate"));
    expect(await screen.findByTestId("plan-preview")).toBeInTheDocument();
    expect(deactivate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("plan-confirm"));
    await waitFor(() => expect(deactivate).toHaveBeenCalledWith("op-2"));
  });

  it("blocks confirmation for blocker and recovery plans", async () => {
    planActivate.mockResolvedValue({
      operationId: "op-blocked",
      envelope: { ...envelope, blockers: ["target conflict"], recoveryRequired: true },
    });

    render(
      <PromptDetailPage
        promptId="prompt-1"
        tool="claude"
        scope="user"
        projectDir=""
        toast={toast}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onChanged={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByTestId("prompt-activate"));
    expect(await screen.findByTestId("plan-confirm")).toBeDisabled();
    fireEvent.click(screen.getByTestId("plan-confirm"));
    expect(activate).not.toHaveBeenCalled();
  });

  it("keeps the plan open and retryable after execution fails", async () => {
    planActivate.mockResolvedValue({ operationId: "op-failed", envelope });
    activate.mockResolvedValue({
      operationId: "op-failed",
      envelope: { ...envelope, preview: false, ok: false, exitCode: 1, error: "write failed" },
    });

    render(
      <PromptDetailPage
        promptId="prompt-1"
        tool="claude"
        scope="user"
        projectDir=""
        toast={toast}
        onClose={vi.fn()}
        onEdit={vi.fn()}
        onChanged={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByTestId("prompt-activate"));
    fireEvent.click(await screen.findByTestId("plan-confirm"));

    expect(await screen.findByText("write failed")).toBeInTheDocument();
    expect(screen.getByTestId("plan-confirm")).not.toBeDisabled();
    expect(screen.getByTestId("plan-preview")).toBeInTheDocument();
  });
});
