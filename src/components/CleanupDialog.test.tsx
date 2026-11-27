import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CleanupCategory, CleanupPlan, ExtraMeta } from "../types";
import { CleanupDialog } from "./CleanupDialog";

const planCleanup = vi.fn();
const confirmCleanup = vi.fn();

vi.mock("../api", () => ({
  planCleanup: (...args: unknown[]) => planCleanup(...args),
  confirmCleanup: (...args: unknown[]) => confirmCleanup(...args),
}));

const memory = { path: "/home/u/.claude/CLAUDE.md", bytes: 1300, lines: 31, sha256: "abc", mode: 420 };

function extra(
  name: string,
  kind: "saved" | "saved-without-login" | "erased",
  category: CleanupCategory,
): ExtraMeta {
  return { name, path: `/h/${name}`, files: 3, bytes: 2048, kind, category };
}

function plan(overrides: Partial<CleanupPlan> = {}): CleanupPlan {
  return {
    operationId: "op-1",
    tool: "claude",
    deployment: { present: true, title: "Reviewer", restorable: true },
    memory,
    memories: null,
    extras: [],
    loginInKeychain: false,
    agentRunning: false,
    nothingToDo: false,
    blockers: [],
    ...overrides,
  };
}

function renderDialog(onDone = vi.fn(), onOpenVersions = vi.fn(), onClose = vi.fn(), tool: "claude" | "codex" = "claude") {
  render(
    <CleanupDialog
      tool={tool}
      toolName={tool === "codex" ? "Codex" : "Claude Code"}
      open
      onClose={onClose}
      onDone={onDone}
      onOpenVersions={onOpenVersions}
    />,
  );
  return { onDone, onOpenVersions, onClose };
}

describe("CleanupDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("defaults to Keysmith deployment only and leaves memory, skills, sessions and login unchecked", async () => {
    planCleanup.mockResolvedValue(
      plan({
        extras: [
          extra(".claude/skills", "saved", "setup"),
          extra(".claude/projects", "saved", "sessions"),
          extra(".claude/cache", "erased", "login-and-cache"),
        ],
        loginInKeychain: true,
      }),
    );
    confirmCleanup.mockResolvedValue({
      snapshotId: "20261001-1",
      deactivated: true,
      memoryCleared: false,
      memoriesCleared: false,
      extrasCleared: 0,
      sessionsCleared: 0,
    });

    const { onDone, onOpenVersions } = renderDialog();
    await screen.findByTestId("cleanup-plan");

    expect(screen.getByTestId("cleanup-opt-deployment")).toBeChecked();
    expect(screen.getByTestId("cleanup-opt-memory")).not.toBeChecked();
    expect(screen.getByTestId("cleanup-opt-setup")).not.toBeChecked();
    expect(screen.getByTestId("cleanup-opt-sessions")).not.toBeChecked();
    expect(screen.getByTestId("cleanup-opt-login-and-cache")).not.toBeChecked();

    // Unchecked user assets are explicitly listed as staying untouched.
    const kept = screen.getByTestId("cleanup-kept-summary");
    expect(kept).toHaveTextContent("CLAUDE.md");

    // No destructive ack needed when only Keysmith's deployment is selected.
    expect(screen.queryByTestId("cleanup-ack")).toBeNull();
    expect(screen.queryByTestId("cleanup-ack-erase")).toBeNull();
    expect(screen.getByTestId("cleanup-confirm")).toBeEnabled();

    fireEvent.click(screen.getByTestId("cleanup-confirm"));
    await screen.findByTestId("cleanup-done");
    expect(confirmCleanup).toHaveBeenCalledWith("op-1", {
      deployment: true,
      memory: false,
      memories: false,
      setup: false,
      sessions: false,
      loginAndCache: false,
    });
    expect(onDone).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByTestId("cleanup-open-versions"));
    expect(onOpenVersions).toHaveBeenCalledTimes(1);
  });

  it("asks for explicit confirmation only when the user opts into emptying their memory file", async () => {
    planCleanup.mockResolvedValue(plan());
    renderDialog();
    await screen.findByTestId("cleanup-plan");

    expect(screen.getByTestId("cleanup-confirm")).toBeEnabled();
    fireEvent.click(screen.getByTestId("cleanup-opt-memory"));
    expect(screen.getByTestId("cleanup-ack")).toBeInTheDocument();
    expect(screen.getByTestId("cleanup-confirm")).toBeDisabled();

    fireEvent.click(screen.getByTestId("cleanup-ack"));
    expect(screen.getByTestId("cleanup-confirm")).toBeEnabled();

    // Unchecking memory clears the ack gate again.
    fireEvent.click(screen.getByTestId("cleanup-opt-memory"));
    expect(screen.queryByTestId("cleanup-ack")).toBeNull();
    expect(screen.getByTestId("cleanup-confirm")).toBeEnabled();
    expect(confirmCleanup).not.toHaveBeenCalled();
  });

  it("moves opted-in sessions into a recoverable snapshot and reports it on completion", async () => {
    planCleanup.mockResolvedValue(
      plan({
        deployment: { present: false, title: null, restorable: false },
        memory: null,
        extras: [extra(".claude/projects", "saved", "sessions")],
      }),
    );
    confirmCleanup.mockResolvedValue({
      snapshotId: "s-sessions",
      deactivated: false,
      memoryCleared: false,
      memoriesCleared: false,
      extrasCleared: 1,
      sessionsCleared: 1,
    });

    renderDialog();
    await screen.findByTestId("cleanup-plan");
    // Nothing is pre-selected when there is no active deployment.
    expect(screen.getByTestId("cleanup-confirm")).toBeDisabled();
    expect(screen.getByTestId("cleanup-select-hint")).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("cleanup-details-sessions"));
    expect(screen.getByTestId("cleanup-extra")).toHaveTextContent(".claude/projects");

    fireEvent.click(screen.getByTestId("cleanup-opt-sessions"));
    expect(screen.getByTestId("cleanup-confirm")).toBeEnabled();
    fireEvent.click(screen.getByTestId("cleanup-confirm"));
    await screen.findByTestId("cleanup-done");
    expect(screen.getByTestId("cleanup-done-sessions")).toBeInTheDocument();
    expect(confirmCleanup).toHaveBeenCalledWith("op-1", {
      deployment: false,
      memory: false,
      memories: false,
      setup: false,
      sessions: true,
      loginAndCache: false,
    });
  });

  it("isolates irreversible login/cache deletion and gates it behind an explicit confirmation checkbox", async () => {
    planCleanup.mockResolvedValue(
      plan({
        deployment: { present: false, title: null, restorable: false },
        memory: null,
        extras: [
          extra(".claude/skills", "saved", "setup"),
          extra(".claude/settings.json", "saved-without-login", "login-and-cache"),
          extra(".claude/cache", "erased", "login-and-cache"),
        ],
        loginInKeychain: true,
      }),
    );
    confirmCleanup.mockResolvedValue({
      snapshotId: null,
      deactivated: false,
      memoryCleared: false,
      memoriesCleared: false,
      erased: 1,
      keychainCleared: true,
    });

    renderDialog();
    await screen.findByTestId("cleanup-plan");
    expect(screen.getByTestId("cleanup-irreversible-section")).toBeInTheDocument();
    expect(screen.getByTestId("cleanup-confirm")).toBeDisabled();

    // Ticking setup alone never triggers the irreversible erase gate (keyed configs live in login-and-cache).
    fireEvent.click(screen.getByTestId("cleanup-opt-setup"));
    expect(screen.queryByTestId("cleanup-ack-erase")).toBeNull();
    expect(screen.getByTestId("cleanup-confirm")).toBeEnabled();
    fireEvent.click(screen.getByTestId("cleanup-opt-setup"));

    fireEvent.click(screen.getByTestId("cleanup-details-loginAndCache"));
    expect(screen.getByTestId("cleanup-extra")).toHaveTextContent(".claude/settings.json");
    expect(screen.getByTestId("cleanup-extra-erased")).toHaveTextContent(".claude/cache");
    expect(screen.getByTestId("cleanup-keychain")).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("cleanup-opt-login-and-cache"));
    expect(screen.getByTestId("cleanup-ack-erase")).toBeInTheDocument();
    expect(screen.getByTestId("cleanup-confirm")).toBeDisabled();

    fireEvent.click(screen.getByTestId("cleanup-ack-erase"));
    expect(screen.getByTestId("cleanup-confirm")).toBeEnabled();
    fireEvent.click(screen.getByTestId("cleanup-confirm"));
    await screen.findByTestId("cleanup-done");
    expect(screen.queryByTestId("cleanup-open-versions")).toBeNull();
    expect(confirmCleanup).toHaveBeenCalledWith("op-1", {
      deployment: false,
      memory: false,
      memories: false,
      setup: false,
      sessions: false,
      loginAndCache: true,
    });
  });

  it("does not offer a button when there is nothing to clean", async () => {
    planCleanup.mockResolvedValue(
      plan({ nothingToDo: true, deployment: { present: false, title: null, restorable: false }, memory: null }),
    );
    renderDialog();
    await screen.findByTestId("cleanup-nothing");
    expect(screen.getByTestId("cleanup-confirm")).toBeDisabled();
  });

  it("refuses while the agent's setup needs repair", async () => {
    planCleanup.mockResolvedValue(plan({ blockers: ["drift"], memory: null }));
    renderDialog();
    await screen.findByTestId("cleanup-blocked");
    expect(screen.getByTestId("cleanup-confirm")).toBeDisabled();
  });

  it("still allows cleanup when only the agent's config drifted", async () => {
    planCleanup.mockResolvedValue(
      plan({ configDrifted: true, deployment: { present: false, title: null, restorable: false } }),
    );
    renderDialog();
    await screen.findByTestId("cleanup-drifted");
    expect(screen.queryByTestId("cleanup-blocked")).toBeNull();
  });

  it("keeps a failed confirmation closed and does not call it again", async () => {
    planCleanup.mockResolvedValue(plan({ memory: null }));
    confirmCleanup.mockRejectedValue("plan already used: things changed since the preview");
    const { onDone } = renderDialog();
    await screen.findByTestId("cleanup-plan");
    fireEvent.click(screen.getByTestId("cleanup-confirm"));
    await waitFor(() => expect(confirmCleanup).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.getByTestId("cleanup-confirm")).toBeDisabled());
    expect(onDone).not.toHaveBeenCalled();
    expect(screen.queryByTestId("cleanup-done")).toBeNull();
  });

  it("shows a readable failure when the preview itself fails", async () => {
    planCleanup.mockRejectedValue(new Error("boom"));
    renderDialog();
    await waitFor(() => expect(screen.getByTestId("cleanup-confirm")).toBeDisabled());
    expect(screen.queryByTestId("cleanup-plan")).toBeNull();
    expect(planCleanup).toHaveBeenCalledWith("claude");
  });

  it("ignores a late preview response after the dialog closes and reopens", async () => {
    let resolveFirst!: (value: CleanupPlan) => void;
    planCleanup
      .mockImplementationOnce(
        () =>
          new Promise<CleanupPlan>((resolve) => {
            resolveFirst = resolve;
          }),
      )
      .mockResolvedValueOnce(plan({ operationId: "op-fresh" }));
    confirmCleanup.mockResolvedValue({
      snapshotId: "s-fresh",
      deactivated: true,
      memoryCleared: false,
      memoriesCleared: false,
    });

    const props = {
      tool: "claude" as const,
      toolName: "Claude Code",
      onClose: vi.fn(),
      onDone: vi.fn(),
      onOpenVersions: vi.fn(),
    };
    const { rerender } = render(<CleanupDialog {...props} open />);
    rerender(<CleanupDialog {...props} open={false} />);
    rerender(<CleanupDialog {...props} open />);
    await screen.findByTestId("cleanup-plan");

    // Stale first response arrives after second open already loaded.
    resolveFirst(plan({ operationId: "op-stale" }));
    await Promise.resolve();

    fireEvent.click(screen.getByTestId("cleanup-confirm"));
    await screen.findByTestId("cleanup-done");
    expect(confirmCleanup).toHaveBeenCalledWith("op-fresh", expect.any(Object));
    expect(confirmCleanup).not.toHaveBeenCalledWith("op-stale", expect.any(Object));
  });

  describe("running agent and Codex memories", () => {
    const folder = { path: "/home/u/.codex/memories", files: 2778, bytes: 41943040 };
    const codex = (overrides: Partial<CleanupPlan> = {}) =>
      plan({
        tool: "codex",
        memory: { ...memory, path: "/home/u/.codex/AGENTS.md" },
        memories: folder,
        extras: [extra(".codex/sessions", "saved", "sessions")],
        ...overrides,
      });

    it("names the memory file after the agent's own file and keeps memories unchecked by default", async () => {
      planCleanup.mockResolvedValue(codex());
      renderDialog(vi.fn(), vi.fn(), vi.fn(), "codex");
      await screen.findByTestId("cleanup-plan");
      expect(screen.getByTestId("cleanup-memory")).toHaveTextContent("AGENTS.md");
      expect(screen.getByTestId("cleanup-opt-memories")).not.toBeChecked();
    });

    it("allows cleaning deployment while Codex runs, but blocks once memories or sessions are ticked", async () => {
      planCleanup.mockResolvedValue(codex({ agentRunning: true }));
      renderDialog(vi.fn(), vi.fn(), vi.fn(), "codex");
      await screen.findByTestId("cleanup-plan");

      // Only deployment is selected by default -> allowed while running.
      expect(screen.queryByTestId("cleanup-running")).toBeNull();
      expect(screen.getByTestId("cleanup-confirm")).toBeEnabled();

      // Opting into memories or sessions blocks until Codex quits.
      fireEvent.click(screen.getByTestId("cleanup-opt-sessions"));
      expect(screen.getByTestId("cleanup-running")).toBeInTheDocument();
      expect(screen.getByTestId("cleanup-confirm")).toBeDisabled();

      // Unticking sessions unblocks deployment-only cleanup again.
      fireEvent.click(screen.getByTestId("cleanup-opt-sessions"));
      expect(screen.queryByTestId("cleanup-running")).toBeNull();
      expect(screen.getByTestId("cleanup-confirm")).toBeEnabled();
    });
  });
});
