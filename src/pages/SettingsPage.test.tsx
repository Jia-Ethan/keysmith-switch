import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { applyLanguage } from "../i18n";
import type { ToastApi } from "../hooks/useToasts";
import { DEFAULT_SETTINGS } from "../types";
import { SettingsPage } from "./SettingsPage";

const listTools = vi.fn();
const getDataDirs = vi.fn();
const listBackups = vi.fn();
const restoreBackup = vi.fn();
const createBackup = vi.fn();
const inspectZipArchive = vi.fn();

vi.mock("../api", () => ({
  listTools: (...args: unknown[]) => listTools(...args),
  getDataDirs: (...args: unknown[]) => getDataDirs(...args),
  listBackups: (...args: unknown[]) => listBackups(...args),
  restoreBackup: (...args: unknown[]) => restoreBackup(...args),
  createBackup: (...args: unknown[]) => createBackup(...args),
  inspectZipArchive: (...args: unknown[]) => inspectZipArchive(...args),
  importMarkdownFiles: vi.fn(),
  importZipArchive: vi.fn(),
  exportZipArchive: vi.fn(),
  planClearAllData: vi.fn(),
  clearAllData: vi.fn(),
}));

vi.mock("../lib/runtime", () => ({
  openExternal: vi.fn(),
  pickFiles: vi.fn().mockResolvedValue([]),
  pickSavePath: vi.fn().mockResolvedValue(null),
}));

describe("SettingsPage data safety", () => {
  const toast = {
    toasts: [],
    dismiss: vi.fn(),
    info: vi.fn(),
    ok: vi.fn(),
    err: vi.fn(),
  } as unknown as ToastApi;

  beforeEach(() => {
    applyLanguage("zh-CN");
    vi.clearAllMocks();
    listTools.mockResolvedValue({ tools: [] });
    getDataDirs.mockResolvedValue({ home: "/data", logs: "/logs", backups: "/backups" });
    listBackups.mockResolvedValue({
      backups: [
        {
          id: "manual-20260819",
          path: "/backups/manual-20260819.zip",
          createdAt: "2026-08-19T00:00:00Z",
          kind: "manual",
          bytes: 1024,
        },
      ],
    });
    restoreBackup.mockResolvedValue({ imported: 4, skipped: 0, errors: [] });
    inspectZipArchive.mockResolvedValue({ mode: "restore" });
  });

  it("requires confirmation before replacing data from a backup", async () => {
    const onDataChanged = vi.fn();
    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        loadError={null}
        onSave={vi.fn()}
        onDataChanged={onDataChanged}
        toast={toast}
        initialTab="data"
      />,
    );

    await screen.findByText("manual-20260819");
    fireEvent.click(screen.getByRole("button", { name: "恢复" }));

    expect(restoreBackup).not.toHaveBeenCalled();
    expect(screen.getByText(/提示词库、版本历史、激活记录和设置可能被替换/)).toBeInTheDocument();

    fireEvent.click(screen.getByTestId("restore-backup-confirm"));
    await waitFor(() =>
      expect(restoreBackup).toHaveBeenCalledWith("/backups/manual-20260819.zip"),
    );
    expect(onDataChanged).toHaveBeenCalledTimes(1);
    expect(toast.ok).toHaveBeenCalledWith("归档已应用（4 个提示词）");
  });

  it("reports backup creation failures instead of leaving an unhandled rejection", async () => {
    const error = new Error("disk full");
    createBackup.mockRejectedValue(error);
    render(
      <SettingsPage
        settings={DEFAULT_SETTINGS}
        loadError={null}
        onSave={vi.fn()}
        toast={toast}
        initialTab="data"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "立即备份" }));
    await waitFor(() => expect(toast.err).toHaveBeenCalledWith(error));
  });
});
