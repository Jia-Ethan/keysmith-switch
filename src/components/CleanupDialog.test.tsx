import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CleanupPlan } from "../types";
import { CleanupDialog } from "./CleanupDialog";

const planCleanup = vi.fn();
const confirmCleanup = vi.fn();

vi.mock("../api", () => ({
  planCleanup: (...args: unknown[]) => planCleanup(...args),
  confirmCleanup: (...args: unknown[]) => confirmCleanup(...args),
}));

const memory = { path: "/home/u/.claude/CLAUDE.md", bytes: 1300, lines: 31, sha256: "abc", mode: 420 };

function plan(overrides: Partial<CleanupPlan> = {}): CleanupPlan {
  return {
    operationId: "op-1",
    tool: "claude",
    deployment: { present: true, title: "Reviewer", restorable: true },
    memory,
    memories: null,
    agentRunning: false,
    nothingToDo: false,
    blockers: [],
    ...overrides,
  };
}

function renderDialog(onDone = vi.fn(), onOpenVersions = vi.fn(), onClose = vi.fn()) {
  render(
    <CleanupDialog tool="claude" toolName="Claude Code" open onClose={onClose} onDone={onDone} onOpenVersions={onOpenVersions} />,
  );
  return { onDone, onOpenVersions, onClose };
}

describe("CleanupDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("asks for a tick before it empties what the person wrote in the memory file", async () => {
    planCleanup.mockResolvedValue(plan());
    renderDialog();
    await screen.findByTestId("cleanup-plan");
    expect(screen.getByTestId("cleanup-deployment")).toBeInTheDocument();
    expect(screen.getByTestId("cleanup-memory")).toBeInTheDocument();
    const confirm = screen.getByTestId("cleanup-confirm");
    expect(confirm).toBeDisabled();
    fireEvent.click(screen.getByTestId("cleanup-ack"));
    expect(confirm).toBeEnabled();
    fireEvent.click(screen.getByTestId("cleanup-ack"));
    expect(confirm).toBeDisabled();
    expect(confirmCleanup).not.toHaveBeenCalled();
  });

  it("needs no tick when there is no memory to empty", async () => {
    planCleanup.mockResolvedValue(plan({ memory: { ...memory, bytes: 0, lines: 0 } }));
    renderDialog();
    await screen.findByTestId("cleanup-plan");
    expect(screen.queryByTestId("cleanup-ack")).toBeNull();
    expect(screen.queryByTestId("cleanup-memory")).toBeNull();
    expect(screen.getByTestId("cleanup-confirm")).toBeEnabled();
  });

  it("confirms the previewed plan, reports it and offers the saved versions", async () => {
    planCleanup.mockResolvedValue(plan());
    confirmCleanup.mockResolvedValue({ snapshotId: "20261001-1", deactivated: true, memoryCleared: true, memoriesCleared: false });
    const { onDone, onOpenVersions } = renderDialog();
    await screen.findByTestId("cleanup-plan");
    fireEvent.click(screen.getByTestId("cleanup-ack"));
    fireEvent.click(screen.getByTestId("cleanup-confirm"));
    await screen.findByTestId("cleanup-done");
    expect(confirmCleanup).toHaveBeenCalledWith("op-1", false);
    expect(onDone).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByTestId("cleanup-open-versions"));
    expect(onOpenVersions).toHaveBeenCalledTimes(1);
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

  it("still cleans when only the agent's config drifted", async () => {
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

  describe("an agent's memory folder", () => {
    const folder = { path: "/home/u/.codex/memories", files: 2778, bytes: 41943040 };
    const codex = (overrides: Partial<CleanupPlan> = {}) =>
      plan({ tool: "codex", memory: { ...memory, path: "/home/u/.codex/AGENTS.md" }, memories: folder, ...overrides });

    it("names the memory file after the agent's own file", async () => {
      planCleanup.mockResolvedValue(codex());
      renderDialog();
      await screen.findByTestId("cleanup-plan");
      expect(screen.getByTestId("cleanup-memory")).toHaveTextContent("AGENTS.md");
    });

    it("lists the memories folder and always asks for it to be moved out", async () => {
      planCleanup.mockResolvedValue(codex());
      confirmCleanup.mockResolvedValue({ snapshotId: "s", deactivated: true, memoryCleared: true, memoriesCleared: true });
      renderDialog();
      await screen.findByTestId("cleanup-plan");
      expect(screen.getByTestId("cleanup-memories")).toBeInTheDocument();
      fireEvent.click(screen.getByTestId("cleanup-ack"));
      fireEvent.click(screen.getByTestId("cleanup-confirm"));
      await screen.findByTestId("cleanup-done");
      expect(confirmCleanup).toHaveBeenCalledWith("op-1", true);
    });

    it("cannot run while the agent runs", async () => {
      planCleanup.mockResolvedValue(codex({ agentRunning: true, deployment: { present: false, title: null, restorable: false }, memory: null }));
      renderDialog();
      await screen.findByTestId("cleanup-plan");
      expect(screen.getByTestId("cleanup-running")).toBeInTheDocument();
      expect(screen.getByTestId("cleanup-confirm")).toBeDisabled();
    });
  });

  describe("a full cleanup", () => {
    const none = { present: false, title: null, restorable: false };
    const extra = (name: string, kind: "saved" | "saved-without-login" | "erased") => ({ name, path: `/h/${name}`, files: 3, bytes: 2048, kind });

    it("lists what is saved, what is saved without its login, what is deleted, and the keychain login", async () => {
      planCleanup.mockResolvedValue(
        plan({
          deployment: none,
          memory: null,
          extras: [extra(".claude/rules", "saved"), extra(".claude/settings.json", "saved-without-login"), extra(".claude/projects", "erased")],
          loginInKeychain: true,
        }),
      );
      confirmCleanup.mockResolvedValue({ snapshotId: "s", deactivated: false, memoryCleared: false, memoriesCleared: false, erased: 1, keychainCleared: true });
      renderDialog();
      await screen.findByTestId("cleanup-plan");
      expect(screen.getAllByTestId("cleanup-extra")).toHaveLength(2);
      expect(screen.getByTestId("cleanup-extra-erased")).toHaveTextContent(".claude/projects");
      expect(screen.getByTestId("cleanup-keychain")).toBeInTheDocument();
      expect(screen.getByTestId("cleanup-confirm")).toBeEnabled();
      fireEvent.click(screen.getByTestId("cleanup-confirm"));
      await screen.findByTestId("cleanup-done");
    });

    it("can be run when only a login or history is left", async () => {
      planCleanup.mockResolvedValue(plan({ deployment: none, memory: null, extras: [extra(".claude/projects", "erased")] }));
      renderDialog();
      await screen.findByTestId("cleanup-plan");
      expect(screen.getByTestId("cleanup-confirm")).toBeEnabled();
    });
  });
});
