import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ToastApi } from "../hooks/useToasts";
import type { RollbackPlan, SnapshotMeta } from "../types";
import { VersionsTab } from "./VersionsTab";

const listSnapshots = vi.fn();
const planRollback = vi.fn();
const confirmRollback = vi.fn();
const deleteSnapshot = vi.fn();
const loadHarnessStatus = vi.fn();

vi.mock("../api", () => ({
  listSnapshots: (...args: unknown[]) => listSnapshots(...args),
  planRollback: (...args: unknown[]) => planRollback(...args),
  confirmRollback: (...args: unknown[]) => confirmRollback(...args),
  deleteSnapshot: (...args: unknown[]) => deleteSnapshot(...args),
}));
vi.mock("../lib/harnessState", () => ({
  loadHarnessStatus: (...args: unknown[]) => loadHarnessStatus(...args),
}));

const toast: ToastApi = { toasts: [], dismiss: vi.fn(), info: vi.fn(), ok: vi.fn(), err: vi.fn() };

const snapshot: SnapshotMeta = {
  id: "20261001-141600-ab12",
  createdAt: "2026-10-01T14:16:00Z",
  tool: "claude",
  kind: "cleanup",
  deployment: { present: true, title: "Reviewer", restorable: true },
  memory: { path: "/home/u/.claude/CLAUDE.md", bytes: 1300, lines: 31, sha256: "abc", mode: 420 },
};

function rollbackPlan(overrides: Partial<RollbackPlan> = {}): RollbackPlan {
  return {
    operationId: "op-9",
    snapshot,
    currentTitle: null,
    replacesDeployment: false,
    memory: { restoreBytes: 1300, currentBytes: 0, currentDiffers: true },
    savesCurrent: false,
    blockers: [],
    ...overrides,
  };
}

describe("VersionsTab", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listSnapshots.mockResolvedValue([snapshot]);
  });

  it("says so when nothing has been saved yet", async () => {
    listSnapshots.mockResolvedValue([]);
    render(<VersionsTab toast={toast} />);
    await screen.findByTestId("versions-empty");
  });

  it("lists each saved version with its own rollback and delete", async () => {
    render(<VersionsTab toast={toast} />);
    const row = await screen.findByTestId(`version-${snapshot.id}`);
    expect(row).toHaveTextContent("Reviewer");
    expect(screen.getByTestId(`version-rollback-${snapshot.id}`)).toBeInTheDocument();
    expect(screen.getByTestId(`version-delete-${snapshot.id}`)).toBeInTheDocument();
  });

  it("previews a rollback and applies exactly that plan", async () => {
    planRollback.mockResolvedValue(rollbackPlan());
    confirmRollback.mockResolvedValue({ savedSnapshotId: null, redeployed: true, memoryRestored: true });
    const onChanged = vi.fn();
    render(<VersionsTab toast={toast} onChanged={onChanged} />);
    fireEvent.click(await screen.findByTestId(`version-rollback-${snapshot.id}`));
    await screen.findByTestId("rollback-plan");
    expect(planRollback).toHaveBeenCalledWith(snapshot.id);
    expect(screen.getByTestId("rollback-deploy")).toBeInTheDocument();
    expect(screen.getByTestId("rollback-memory")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("rollback-confirm"));
    await waitFor(() => expect(confirmRollback).toHaveBeenCalledWith("op-9"));
    await waitFor(() => expect(toast.ok).toHaveBeenCalled());
    expect(loadHarnessStatus).toHaveBeenCalledWith("claude", true);
    expect(onChanged).toHaveBeenCalled();
  });

  it("cannot roll back while the setup needs repair", async () => {
    planRollback.mockResolvedValue(rollbackPlan({ blockers: ["drift"] }));
    render(<VersionsTab toast={toast} />);
    fireEvent.click(await screen.findByTestId(`version-rollback-${snapshot.id}`));
    await screen.findByTestId("rollback-plan");
    expect(screen.getByTestId("rollback-confirm")).toBeDisabled();
  });

  it("cannot roll back to a version that holds nothing to restore", async () => {
    const empty = { ...snapshot, deployment: { present: false, title: null, restorable: false }, memory: null };
    planRollback.mockResolvedValue(rollbackPlan({ snapshot: empty, memory: null }));
    render(<VersionsTab toast={toast} />);
    fireEvent.click(await screen.findByTestId(`version-rollback-${snapshot.id}`));
    await screen.findByTestId("rollback-plan");
    expect(screen.getByTestId("rollback-confirm")).toBeDisabled();
  });

  it("does not report success when the rollback fails", async () => {
    planRollback.mockResolvedValue(rollbackPlan());
    confirmRollback.mockRejectedValue("plan already used");
    render(<VersionsTab toast={toast} />);
    fireEvent.click(await screen.findByTestId(`version-rollback-${snapshot.id}`));
    await screen.findByTestId("rollback-plan");
    fireEvent.click(screen.getByTestId("rollback-confirm"));
    await waitFor(() => expect(confirmRollback).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.getByTestId("rollback-confirm")).toBeDisabled());
    expect(toast.ok).not.toHaveBeenCalled();
    expect(loadHarnessStatus).not.toHaveBeenCalled();
  });

  it("asks before deleting a version", async () => {
    deleteSnapshot.mockResolvedValue({ ok: true });
    render(<VersionsTab toast={toast} />);
    fireEvent.click(await screen.findByTestId(`version-delete-${snapshot.id}`));
    expect(deleteSnapshot).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByTestId("version-confirm-delete"));
    await waitFor(() => expect(deleteSnapshot).toHaveBeenCalledWith(snapshot.id));
  });
});
